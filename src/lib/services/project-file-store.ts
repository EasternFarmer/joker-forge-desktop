export interface ProjectFileStoreFs {
  join(...parts: string[]): Promise<string>;
  exists(path: string): Promise<boolean>;
  mkdir(path: string, options: { recursive: true }): Promise<unknown>;
  readDir(path: string): Promise<Array<{ name: string; isDirectory: boolean }>>;
  readTextFile(path: string): Promise<string>;
  writeTextFile(path: string, contents: string): Promise<unknown>;
  rename(from: string, to: string): Promise<unknown>;
  remove(path: string, options?: { recursive?: boolean }): Promise<unknown>;
}

export interface ProjectFileStoreData<T> {
  currentProjectId: string;
  projects: Record<string, T>;
}

export interface ProjectFileStoreOptions<T> {
  fs: ProjectFileStoreFs;
  getPaths(): Promise<{ settingsPath: string; projectsDir: string }>;
  externalize(project: T, assetsRoot: string): Promise<T>;
  hydrate(raw: unknown, assetsRoot?: string): Promise<T>;
  warn(message: string, error?: unknown): void;
}

export interface ProjectFileStoreSaveOptions {
  deletedProjectIds?: ReadonlySet<string>;
  reset?: boolean;
}

interface Manifest {
  version: 2;
  currentProjectId: string;
  projects: Record<string, string>;
}

interface SettingsRead {
  present: boolean;
  manifest: Manifest | null;
  legacyCurrentProjectId: string;
}

interface DiskState<T> {
  present: boolean;
  currentProjectId: string;
  references: Map<string, string>;
  projects: Map<string, T>;
}

interface RememberedProject<T> {
  project: T;
  reference: string;
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value);

// A manifest reference is one directory/file name, never a relative path.
const isSafeReference = (value: unknown): value is string =>
  typeof value === "string" &&
  value.length > 0 &&
  value !== "." &&
  value !== ".." &&
  !/[<>:"/\\|?*\x00-\x1f]/.test(value) &&
  !/[. ]$/.test(value) &&
  !/^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(value);

const isSnapshotReference = (value: string): boolean =>
  /^save-[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value);

const parentDirectory = (path: string): string => {
  const separator = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  if (separator < 0) return ".";
  return path.slice(0, separator) || path.slice(0, 1);
};

const makeManifest = (
  currentProjectId: string,
  references: ReadonlyMap<string, string>,
): Manifest => ({
  version: 2,
  currentProjectId,
  projects: Object.fromEntries(references),
});

/**
 * Each save writes new immutable project snapshots, then publishes their index
 * with one rename. Readers never see project JSON pointing at half-written art.
 * Omitted and unreadable projects remain on disk until explicitly deleted.
 */
export class ProjectFileStore<T> {
  private readonly options: ProjectFileStoreOptions<T>;
  private readonly remembered = new Map<string, RememberedProject<T>>();
  private readonly readableSnapshots = new Set<string>();
  private readonly protectedSnapshots = new Set<string>();
  private saveQueue: Promise<void> = Promise.resolve();

  constructor(options: ProjectFileStoreOptions<T>) {
    this.options = options;
  }

  load(): Promise<ProjectFileStoreData<T> | null> {
    const pending = this.saveQueue.then(async () => {
      const state = await this.readDiskState();
      if (!state.present) return null;
      return {
        currentProjectId: state.projects.has(state.currentProjectId)
          ? state.currentProjectId
          : state.projects.keys().next().value ?? state.currentProjectId,
        projects: Object.fromEntries(state.projects),
      };
    });
    // Reads also reserve their place in the queue: a concurrent reset cannot
    // collect snapshots while an earlier load is still hydrating their artwork.
    this.saveQueue = pending.then(() => undefined, () => undefined);
    return pending;
  }

  save(
    store: ProjectFileStoreData<T>,
    options: ProjectFileStoreSaveOptions = {},
  ): Promise<void> {
    const pending = this.saveQueue.then(() => this.saveNow(store, options));
    // A failed write must not stop subsequent attempts from retrying.
    this.saveQueue = pending.catch(() => undefined);
    return pending;
  }

  private warn(message: string, error?: unknown): void {
    try {
      this.options.warn(message, error);
    } catch {
      // Reporting recovery/cleanup issues must not change persistence behavior.
    }
  }

  private async readSettings(path: string): Promise<SettingsRead> {
    const empty: SettingsRead = {
      present: false,
      manifest: null,
      legacyCurrentProjectId: "",
    };
    try {
      if (!(await this.options.fs.exists(path))) return empty;
    } catch (error) {
      this.warn("Could not check project settings; preserving the existing store.", error);
      return { ...empty, present: true };
    }

    try {
      const parsed: unknown = JSON.parse(await this.options.fs.readTextFile(path));
      if (!isRecord(parsed)) throw new Error("Invalid project settings");
      if (parsed.version === 1) {
        return {
          present: true,
          manifest: null,
          legacyCurrentProjectId:
            typeof parsed.currentProjectId === "string" ? parsed.currentProjectId : "",
        };
      }
      if (
        parsed.version !== 2 ||
        typeof parsed.currentProjectId !== "string" ||
        !isRecord(parsed.projects)
      ) {
        throw new Error("Invalid project manifest");
      }
      const references = new Map<string, string>();
      for (const [projectId, reference] of Object.entries(parsed.projects)) {
        if (!projectId || !isSafeReference(reference)) {
          throw new Error("Invalid project snapshot reference");
        }
        references.set(projectId, reference);
      }
      return {
        present: true,
        manifest: makeManifest(parsed.currentProjectId, references),
        legacyCurrentProjectId: "",
      };
    } catch (error) {
      this.warn("Project settings are unreadable; attempting recovery without deleting files.", error);
      return { ...empty, present: true };
    }
  }

  private async readProject(
    projectsDir: string,
    reference: string,
    expectedProjectId?: string,
    looseFile = reference.toLowerCase().endsWith(".json"),
    onDescriptor?: (projectId: string) => void,
  ): Promise<{ projectId: string; project: T }> {
    try {
      return await this.readProjectUnchecked(
        projectsDir,
        reference,
        expectedProjectId,
        looseFile,
        onDescriptor,
      );
    } catch (error) {
      // A snapshot may have been readable earlier in this session. Once damage
      // is observed, preserve that original even if a backup can be recovered.
      this.readableSnapshots.delete(reference);
      this.protectedSnapshots.add(reference);
      throw error;
    }
  }

  private async readProjectUnchecked(
    projectsDir: string,
    reference: string,
    expectedProjectId?: string,
    looseFile = reference.toLowerCase().endsWith(".json"),
    onDescriptor?: (projectId: string) => void,
  ): Promise<{ projectId: string; project: T }> {
    if (!isSafeReference(reference)) throw new Error("Unsafe project reference");
    const projectRoot = await this.options.fs.join(projectsDir, reference);
    const projectPath = looseFile
      ? projectRoot
      : await this.options.fs.join(projectRoot, "project.json");
    const parsed: unknown = JSON.parse(await this.options.fs.readTextFile(projectPath));
    if (
      !isRecord(parsed) ||
      parsed.version !== 1 ||
      typeof parsed.projectId !== "string" ||
      !parsed.projectId ||
      !Object.prototype.hasOwnProperty.call(parsed, "project") ||
      (expectedProjectId !== undefined && parsed.projectId !== expectedProjectId)
    ) {
      throw new Error("Invalid project snapshot");
    }
    onDescriptor?.(parsed.projectId);
    const project = await this.options.hydrate(
      parsed.project,
      looseFile ? undefined : await this.options.fs.join(projectRoot, "assets"),
    );
    if (
      !looseFile &&
      isSnapshotReference(reference) &&
      !this.protectedSnapshots.has(reference)
    ) {
      this.readableSnapshots.add(reference);
    }
    return { projectId: parsed.projectId, project };
  }

  private async readDiskState(): Promise<DiskState<T>> {
    const { settingsPath, projectsDir } = await this.options.getPaths();
    const previousPath = await this.options.fs.join(
      parentDirectory(settingsPath),
      "settings.previous.json",
    );
    const [primary, previous] = await Promise.all([
      this.readSettings(settingsPath),
      this.readSettings(previousPath),
    ]);
    const chosenManifest = primary.manifest ?? previous.manifest;
    if (!primary.manifest && previous.manifest) {
      this.warn("Recovered project settings from the previous save.");
    }
    const state: DiskState<T> = {
      present: primary.present || previous.present,
      currentProjectId:
        chosenManifest?.currentProjectId ?? primary.legacyCurrentProjectId,
      references: new Map(),
      projects: new Map(),
    };

    if (chosenManifest) {
      // Only IDs in the current index are eligible for per-project recovery.
      // Otherwise deleting a project would cause it to reappear from the backup.
      for (const [projectId, reference] of Object.entries(chosenManifest.projects)) {
        state.references.set(projectId, reference);
        let recoveredReference = reference;
        let project: T;
        try {
          project = (await this.readProject(projectsDir, reference, projectId)).project;
        } catch (error) {
          this.warn(`Project "${projectId}" is unreadable; its files have been preserved.`, error);
          const fallbackReference =
            primary.manifest && previous.manifest
              ? Object.entries(previous.manifest.projects).find(([id]) => id === projectId)?.[1]
              : undefined;
          if (!fallbackReference || fallbackReference === reference) continue;
          try {
            project = (await this.readProject(projectsDir, fallbackReference, projectId)).project;
            recoveredReference = fallbackReference;
            this.warn(`Recovered project "${projectId}" from its previous saved version.`);
          } catch (fallbackError) {
            this.warn(`The previous saved version for project "${projectId}" is also unreadable.`, fallbackError);
            continue;
          }
        }
        state.references.set(projectId, recoveredReference);
        state.projects.set(projectId, project);
        this.remembered.set(projectId, { project, reference: recoveredReference });
      }
      // Inspect previous-only snapshots once, so a restart does not leak every
      // old backup forever. Unreadable or unrecognized files remain protected.
      if (primary.manifest && previous.manifest) {
        for (const [projectId, reference] of Object.entries(previous.manifest.projects)) {
          if (
            this.readableSnapshots.has(reference) ||
            this.protectedSnapshots.has(reference)
          ) continue;
          try {
            await this.readProject(projectsDir, reference, projectId);
          } catch (error) {
            this.warn(`The previous saved version for project "${projectId}" is unreadable; its files have been preserved.`, error);
          }
        }
      }
      return state;
    }

    // Before snapshot manifests existed, settings contained only the selected
    // ID. Discover those projects even when that small settings file is damaged.
    let entries: Array<{ name: string; isDirectory: boolean }>;
    try {
      if (!(await this.options.fs.exists(projectsDir))) return state;
      entries = await this.options.fs.readDir(projectsDir);
    } catch (error) {
      this.warn("Could not inspect saved projects; preserving the existing store.", error);
      state.present = true;
      return state;
    }
    for (const entry of entries) {
      if (!isSafeReference(entry.name)) continue;
      if (!entry.isDirectory && !entry.name.toLowerCase().endsWith(".json")) continue;
      state.present = true;
      // Unindexed snapshots may belong to an interrupted, unpublished save.
      if (entry.isDirectory && isSnapshotReference(entry.name)) continue;
      try {
        const loaded = await this.readProject(
          projectsDir,
          entry.name,
          undefined,
          !entry.isDirectory,
          (projectId) => {
            // Even when hydration fails, carry the known ID forward so fixing
            // that legacy file can make it visible again after later autosaves.
            if (!state.references.has(projectId)) state.references.set(projectId, entry.name);
          },
        );
        if (state.references.get(loaded.projectId) !== entry.name) {
          this.warn(`Multiple saved files contain project "${loaded.projectId}"; keeping all files.`);
          continue;
        }
        state.references.set(loaded.projectId, entry.name);
        state.projects.set(loaded.projectId, loaded.project);
        this.remembered.set(loaded.projectId, {
          project: loaded.project,
          reference: entry.name,
        });
      } catch (error) {
        this.warn(`Saved project file "${entry.name}" is unreadable; it has been preserved.`, error);
      }
    }
    return state;
  }

  private async writeAtomic(path: string, contents: string): Promise<void> {
    const temporaryPath = `${path}.tmp-${crypto.randomUUID()}`;
    await this.options.fs.writeTextFile(temporaryPath, contents);
    await this.options.fs.rename(temporaryPath, path);
  }

  private async saveNow(
    store: ProjectFileStoreData<T>,
    options: ProjectFileStoreSaveOptions,
  ): Promise<void> {
    // Keep identity matches before reading disk: hydration creates fresh objects.
    const rememberedBeforeRead = new Map(this.remembered);
    const state = await this.readDiskState();
    const { settingsPath, projectsDir } = await this.options.getPaths();
    const settingsDir = parentDirectory(settingsPath);
    const previousPath = await this.options.fs.join(settingsDir, "settings.previous.json");
    await this.options.fs.mkdir(settingsDir, { recursive: true });
    await this.options.fs.mkdir(projectsDir, { recursive: true });

    const references = options.reset
      ? new Map<string, string>()
      : new Map(state.references);
    for (const projectId of options.deletedProjectIds ?? []) references.delete(projectId);
    const saved = new Map<string, RememberedProject<T>>();
    const completedSnapshots = new Set<string>();

    for (const [projectId, project] of Object.entries(store.projects)) {
      if (!projectId) throw new Error("Project IDs must not be empty");
      if (options.deletedProjectIds?.has(projectId)) continue;
      const remembered = rememberedBeforeRead.get(projectId);
      if (
        !options.reset &&
        state.projects.has(projectId) &&
        remembered?.project === project &&
        remembered.reference === references.get(projectId) &&
        isSnapshotReference(remembered.reference)
      ) {
        saved.set(projectId, remembered);
        continue;
      }

      const reference = `save-${crypto.randomUUID()}`;
      const projectRoot = await this.options.fs.join(projectsDir, reference);
      const assetsRoot = await this.options.fs.join(projectRoot, "assets");
      await this.options.fs.mkdir(assetsRoot, { recursive: true });
      const externalized = await this.options.externalize(project, assetsRoot);
      await this.options.fs.writeTextFile(
        await this.options.fs.join(projectRoot, "project.json"),
        JSON.stringify({ version: 1, projectId, project: externalized }),
      );
      references.set(projectId, reference);
      saved.set(projectId, { project, reference });
      completedSnapshots.add(reference);
    }

    const currentProjectId = references.has(store.currentProjectId)
      ? store.currentProjectId
      : references.keys().next().value ?? store.currentProjectId;
    const manifest = makeManifest(currentProjectId, references);
    const primary = await this.readSettings(settingsPath);
    let backupManifest: Manifest | null = null;
    if (primary.manifest) {
      // Use recovered project references, so a damaged current snapshot cannot
      // overwrite the last usable snapshot in the previous settings file.
      backupManifest = makeManifest(state.currentProjectId, state.references);
    } else if (state.references.size > 0) {
      const previous = await this.readSettings(previousPath);
      if (!previous.manifest && !previous.present) {
        // The first migration also needs a recovery index: preserved legacy
        // files should be usable if the newly written snapshot is damaged.
        backupManifest = makeManifest(state.currentProjectId, state.references);
      }
    }

    // Prepare the entire new index before touching either published settings.
    const stagedSettingsPath = `${settingsPath}.tmp-${crypto.randomUUID()}`;
    await this.options.fs.writeTextFile(stagedSettingsPath, JSON.stringify(manifest));
    if (backupManifest) {
      await this.writeAtomic(previousPath, JSON.stringify(backupManifest));
    }
    // If the primary is unreadable, leave the existing recovery index alone.
    await this.options.fs.rename(stagedSettingsPath, settingsPath);

    for (const reference of completedSnapshots) this.readableSnapshots.add(reference);
    this.remembered.clear();
    for (const [projectId, remembered] of saved) this.remembered.set(projectId, remembered);

    if (options.reset) {
      try {
        if (await this.options.fs.exists(previousPath)) {
          await this.options.fs.remove(previousPath);
        }
        backupManifest = null;
      } catch (error) {
        this.warn("Projects were saved, but the previous recovery save could not be cleared.", error);
        const previous = await this.readSettings(previousPath);
        backupManifest = previous.manifest;
      }
    } else if (!backupManifest) {
      const previous = await this.readSettings(previousPath);
      backupManifest = previous.manifest;
    }
    await this.collectGarbage(projectsDir, manifest, backupManifest);
  }

  private async collectGarbage(
    projectsDir: string,
    current: Manifest,
    previous: Manifest | null,
  ): Promise<void> {
    const retained = new Set([
      ...Object.values(current.projects),
      ...Object.values(previous?.projects ?? {}),
    ]);
    // Never sweep arbitrary directory entries: damaged, legacy, and orphaned
    // files may be someone's only surviving copy of their artwork.
    for (const reference of this.readableSnapshots) {
      if (retained.has(reference) || this.protectedSnapshots.has(reference)) continue;
      try {
        await this.options.fs.remove(await this.options.fs.join(projectsDir, reference), {
          recursive: true,
        });
        this.readableSnapshots.delete(reference);
      } catch (error) {
        this.warn("Projects were saved, but an unused previous save could not be cleaned up.", error);
      }
    }
  }
}
