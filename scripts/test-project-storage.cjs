/* Run with `npm run test:storage`. Uses the installed TypeScript compiler only. */
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const vm = require("node:vm");
const { webcrypto } = require("node:crypto");
const { test } = require("node:test");
const ts = require("typescript");

function loadTypeScript(filePath, imports = {}, appendedSource = "", globals = {}) {
  const compiled = ts.transpileModule(
    fs.readFileSync(filePath, "utf8") + appendedSource,
    {
      compilerOptions: {
        module: ts.ModuleKind.CommonJS,
        target: ts.ScriptTarget.ES2022,
        jsx: ts.JsxEmit.ReactJSX,
      },
      fileName: filePath,
    },
  );
  const module = { exports: {} };
  vm.runInNewContext(
    compiled.outputText,
    {
      module,
      exports: module.exports,
      require: (name) => {
        if (Object.hasOwn(imports, name)) return imports[name];
        throw new Error(`Unmocked import in storage test: ${name}`);
      },
      crypto: webcrypto,
      console,
      Uint8Array,
      TextEncoder,
      TextDecoder,
      atob,
      btoa,
      setTimeout,
      clearTimeout,
      ...globals,
    },
    { filename: filePath },
  );
  return module.exports;
}

const { ProjectFileStore } = loadTypeScript(
  path.join(__dirname, "../src/lib/services/project-file-store.ts"),
);
const SETTINGS = "/store/settings.json";
const PREVIOUS_SETTINGS = "/store/settings.previous.json";
const PROJECTS = "/store/projects";
const plain = (value) => JSON.parse(JSON.stringify(value));

class MemoryFs {
  constructor(entries) {
    this.entries = entries
      ? new Map([...entries].map(([key, value]) => [key, { ...value }]))
      : new Map([["/", { directory: true }]]);
    this.operations = [];
    this.failure = null;
    this.stopped = false;
    this.blockedReads = new Set();
    this.adapter = Object.fromEntries(
      [
        "join", "exists", "mkdir", "readDir", "readTextFile",
        "writeTextFile", "rename", "remove",
      ].map((method) => [method, this[method].bind(this)]),
    );
  }

  normalize(value) {
    return path.posix.normalize(value.replace(/\\/g, "/"));
  }

  checkRunning() {
    if (this.stopped) throw new Error("Simulated process interruption");
  }

  async join(...parts) {
    this.checkRunning();
    return this.normalize(path.posix.join(...parts));
  }

  async exists(value) {
    this.checkRunning();
    return this.entries.has(this.normalize(value));
  }

  inject(index, phase, interruption = false) {
    this.operations = [];
    this.failure = { index, phase, interruption };
  }

  restart() {
    this.stopped = false;
    this.failure = null;
    this.operations = [];
  }

  maybeFail(index, phase) {
    if (this.failure?.index !== index || this.failure.phase !== phase) return;
    const { interruption } = this.failure;
    this.failure = null;
    this.stopped = interruption;
    throw new Error(`Injected ${interruption ? "interruption" : "failure"} ${phase} operation ${index}`);
  }

  async mutate(operation, apply) {
    this.checkRunning();
    const index = this.operations.push(operation) - 1;
    this.maybeFail(index, "before");
    apply();
    this.maybeFail(index, "after");
  }

  createDirectories(value, recursive) {
    const target = this.normalize(value);
    if (this.entries.has(target)) {
      assert.equal(this.entries.get(target).directory, true, "mkdir cannot replace a file");
      return;
    }
    const parent = path.posix.dirname(target);
    if (recursive) this.createDirectories(parent, true);
    assert.equal(this.entries.get(parent)?.directory, true, `Missing directory ${parent}`);
    this.entries.set(target, { directory: true });
  }

  async mkdir(value, options = {}) {
    const target = this.normalize(value);
    await this.mutate({ op: "mkdir", path: target }, () =>
      this.createDirectories(target, options.recursive),
    );
  }

  async readDir(value) {
    this.checkRunning();
    const target = this.normalize(value);
    if (!this.entries.get(target)?.directory) throw new Error(`Missing directory ${target}`);
    return [...this.entries]
      .filter(([key]) => key !== target && path.posix.dirname(key) === target)
      .map(([key, entry]) => ({
        name: path.posix.basename(key),
        isDirectory: entry.directory,
        isFile: !entry.directory,
        isSymlink: false,
      }));
  }

  async readTextFile(value) {
    this.checkRunning();
    const target = this.normalize(value);
    if (this.blockedReads.has(target)) throw new Error(`Unreadable file ${target}`);
    const entry = this.entries.get(target);
    if (!entry || entry.directory) throw new Error(`Missing file ${target}`);
    return entry.text;
  }

  async writeTextFile(value, text) {
    const target = this.normalize(value);
    await this.mutate({ op: "writeTextFile", path: target }, () => {
      assert.equal(this.entries.get(path.posix.dirname(target))?.directory, true);
      assert.notEqual(this.entries.get(target)?.directory, true);
      this.entries.set(target, { directory: false, text });
    });
  }

  async rename(from, to) {
    const source = this.normalize(from);
    const target = this.normalize(to);
    await this.mutate({ op: "rename", path: source, target }, () => {
      const entry = this.entries.get(source);
      if (!entry) throw new Error(`Missing rename source ${source}`);
      assert.equal(this.entries.get(path.posix.dirname(target))?.directory, true);
      const moved = [...this.entries].filter(([key]) =>
        key === source || key.startsWith(`${source}/`),
      );
      for (const [key] of moved) this.entries.delete(key);
      for (const [key, item] of moved) {
        this.entries.set(target + key.slice(source.length), { ...item });
      }
    });
  }

  async remove(value, options = {}) {
    const target = this.normalize(value);
    await this.mutate({ op: "remove", path: target }, () => {
      const entry = this.entries.get(target);
      if (!entry) throw new Error(`Missing remove target ${target}`);
      const descendants = [...this.entries.keys()].filter((key) => key.startsWith(`${target}/`));
      if (entry.directory && descendants.length && !options.recursive) {
        throw new Error(`Directory not empty ${target}`);
      }
      this.entries.delete(target);
      for (const key of descendants) this.entries.delete(key);
    });
  }

  put(value, text) {
    const target = this.normalize(value);
    this.createDirectories(path.posix.dirname(target), true);
    this.entries.set(target, { directory: false, text });
  }

  get(value) {
    return this.entries.get(this.normalize(value))?.text;
  }

  manifest(value = SETTINGS) {
    return JSON.parse(this.get(value));
  }

  subtree(value) {
    return [...this.entries]
      .filter(([key]) => key === value || key.startsWith(`${value}/`))
      .map(([key, entry]) => [key, { ...entry }]);
  }
}

function repository(memory, warnings = []) {
  return new ProjectFileStore({
    fs: memory.adapter,
    getPaths: async () => ({ settingsPath: SETTINGS, projectsDir: PROJECTS }),
    externalize: async (project, assetsRoot) => {
      await memory.adapter.writeTextFile(
        await memory.adapter.join(assetsRoot, "picture.png"), project.art,
      );
      return { ...project, art: "asset://picture.png" };
    },
    hydrate: async (raw, assetsRoot) => {
      if (!raw || typeof raw !== "object" || typeof raw.name !== "string" || typeof raw.art !== "string") {
        throw new Error("Invalid project payload");
      }
      const project = { ...raw };
      if (project.art.startsWith("asset://")) {
        if (!assetsRoot) throw new Error("Missing asset root");
        project.art = await memory.adapter.readTextFile(
          await memory.adapter.join(assetsRoot, project.art.slice("asset://".length)),
        );
      }
      return project;
    },
    warn: (message, error) => warnings.push({ message, error }),
  });
}

const original = {
  currentProjectId: "a",
  projects: {
    a: { name: "Original A", art: "original-art-A" },
    b: { name: "Original B", art: "original-art-B" },
  },
};
const changed = {
  currentProjectId: "b",
  projects: {
    a: { name: "Changed A", art: "changed-art-A" },
    b: { name: "Changed B", art: "changed-art-B" },
  },
};

async function seed(data = original) {
  const memory = new MemoryFs();
  await repository(memory).save(plain(data));
  memory.operations = [];
  return memory;
}

async function assertPrecommitFailures(t, candidate, options = {}, initialMemory) {
  const seeded = initialMemory ?? await seed();
  const originalManifest = seeded.manifest();
  const oldEntries = originalManifest.version === 2
    ? Object.values(originalManifest.projects)
    : (await seeded.adapter.readDir(PROJECTS)).map((entry) => entry.name);
  const oldDirectories = oldEntries.map((directory) =>
    [path.posix.join(PROJECTS, directory), seeded.subtree(path.posix.join(PROJECTS, directory))],
  );
  const successful = new MemoryFs(seeded.entries);
  const successfulStore = repository(successful);
  await successfulStore.load();
  await successfulStore.save(plain(candidate), options);
  const commitIndex = successful.operations.findIndex((operation) =>
    operation.op === "rename" && operation.target === SETTINGS,
  );
  assert.ok(commitIndex >= 0, "Settings must commit with an atomic rename");
  for (let index = 0; index <= commitIndex; index += 1) {
    for (const interruption of [false, true]) {
      for (const phase of ["before", "after"]) {
        if (index === commitIndex && phase === "after") continue;
        const operation = successful.operations[index];
        await t.test(`${interruption ? "interrupt" : "fail"} ${phase} ${index}: ${operation.op}`, async () => {
          const memory = new MemoryFs(seeded.entries);
          const store = repository(memory);
          await store.load();
          memory.inject(index, phase, interruption);
          await assert.rejects(store.save(plain(candidate), options));
          memory.restart();
          assert.deepEqual(plain(await repository(memory).load()), original);
          for (const [directory, contents] of oldDirectories) {
            assert.deepEqual(memory.subtree(directory), contents, "Last committed project and artwork remain untouched");
          }
        });
      }
    }
  }
  await t.test("interruption after the commit loads the complete new save", async () => {
    const memory = new MemoryFs(seeded.entries);
    const store = repository(memory);
    await store.load();
    memory.inject(commitIndex, "after", true);
    await store.save(plain(candidate), options).catch(() => {});
    memory.restart();
    assert.deepEqual(plain(await repository(memory).load()), candidate);
  });
}

test("failed or interrupted autosaves preserve every previous project and artwork", async (t) => {
  await assertPrecommitFailures(t, changed);
});

test("failed or interrupted resets preserve the last committed save", async (t) => {
  await assertPrecommitFailures(t, { currentProjectId: "", projects: {} }, { reset: true });
});

test("corrupt project JSON recovers its previous version and keeps healthy siblings current", async () => {
  const memory = await seed();
  await repository(memory).save(changed);
  const manifest = memory.manifest();
  memory.put(`${PROJECTS}/${manifest.projects.a}/project.json`, "{broken JSON");
  const warnings = [];
  const loaded = await repository(memory, warnings).load();
  assert.deepEqual(plain(loaded), {
    currentProjectId: "b",
    projects: { a: original.projects.a, b: changed.projects.b },
  });
  assert.ok(warnings.length, "A recovered damaged project must produce a warning");
});

test("missing referenced artwork recovers the complete previous project", async () => {
  const memory = await seed();
  await repository(memory).save(changed);
  const manifest = memory.manifest();
  memory.entries.delete(`${PROJECTS}/${manifest.projects.a}/assets/picture.png`);
  const warnings = [];
  assert.deepEqual(plain((await repository(memory, warnings).load()).projects.a), original.projects.a);
  assert.ok(warnings.length);
});

test("an unreadable sibling survives loading, autosaving, and a subsequent restart", async () => {
  const memory = await seed();
  const unreadableDirectory = `${PROJECTS}/${memory.manifest().projects.b}`;
  const unreadableFile = `${unreadableDirectory}/project.json`;
  const before = memory.subtree(unreadableDirectory);
  memory.blockedReads.add(unreadableFile);
  const warnings = [];
  const store = repository(memory, warnings);
  const loaded = await store.load();
  assert.equal(loaded.projects.b, undefined);
  assert.ok(warnings.length, "Unreadable projects must be reported");
  loaded.projects.a = changed.projects.a;
  await store.save(loaded);
  assert.deepEqual(memory.subtree(unreadableDirectory), before);
  assert.equal(memory.manifest().projects.b, path.posix.basename(unreadableDirectory));
  memory.blockedReads.clear();
  assert.deepEqual(plain(await repository(memory).load()), {
    currentProjectId: "a", projects: { a: changed.projects.a, b: original.projects.b },
  });
});

test("a damaged sibling with no backup remains on disk and in the manifest after autosave", async () => {
  const memory = await seed();
  const directory = `${PROJECTS}/${memory.manifest().projects.b}`;
  const projectPath = `${directory}/project.json`;
  const goodJson = memory.get(projectPath);
  memory.put(projectPath, "not JSON");
  const damagedContents = memory.subtree(directory);
  const warnings = [];
  const store = repository(memory, warnings);
  const loaded = await store.load();
  assert.equal(loaded.projects.b, undefined);
  await store.save(loaded);
  assert.deepEqual(memory.subtree(directory), damagedContents);
  assert.equal(memory.manifest().projects.b, path.posix.basename(directory));
  assert.ok(warnings.length);
  memory.put(projectPath, goodJson);
  assert.deepEqual(plain((await repository(memory).load()).projects.b), original.projects.b);
});

test("unchanged in-memory projects repair a damaged disk snapshot instead of reusing it", async (t) => {
  for (const corruption of ["JSON", "artwork"]) {
    await t.test(corruption, async () => {
      const memory = new MemoryFs();
      const store = repository(memory);
      const data = plain(original);
      await store.save(data);
      const ref = memory.manifest().projects.a;
      const directory = `${PROJECTS}/${ref}`;
      if (corruption === "JSON") memory.put(`${directory}/project.json`, "broken project");
      else memory.entries.delete(`${directory}/assets/picture.png`);
      const damagedContents = memory.subtree(directory);
      // Keep the same object identities to exercise the unchanged-project shortcut.
      await store.save(data);
      assert.notEqual(memory.manifest().projects.a, ref);
      assert.deepEqual(plain(await repository(memory).load()), original);
      assert.deepEqual(memory.subtree(directory), damagedContents, "Damaged originals remain available for recovery");
    });
  }
});

test("projects omitted from an autosave are retained unless explicitly deleted", async () => {
  const memory = await seed();
  await repository(memory).save({ currentProjectId: "a", projects: { a: changed.projects.a } });
  assert.deepEqual(plain(await repository(memory).load()), {
    currentProjectId: "a", projects: { a: changed.projects.a, b: original.projects.b },
  });
});

test("explicit deletion persists through later saves and backup recovery", async () => {
  const memory = await seed();
  const remaining = { currentProjectId: "a", projects: { a: original.projects.a } };
  await repository(memory).save(remaining, { deletedProjectIds: new Set(["b"]) });
  assert.equal((await repository(memory).load()).projects.b, undefined);
  await repository(memory).save(remaining);
  assert.equal(memory.manifest().projects.b, undefined);
  memory.put(SETTINGS, "{corrupt");
  assert.deepEqual(plain(await repository(memory).load()), remaining);
});

test("colliding sanitized project IDs never overwrite one another", async () => {
  const data = {
    currentProjectId: "project:a",
    projects: {
      "project:a": { name: "Colon", art: "colon-art" },
      "project/a": { name: "Slash", art: "slash-art" },
      "PROJECT_A": { name: "Uppercase", art: "uppercase-art" },
      "../outside": { name: "Traversal", art: "safe-art" },
    },
  };
  const memory = await seed(data);
  const refs = Object.values(memory.manifest().projects);
  assert.equal(new Set(refs.map((value) => value.toLowerCase())).size, refs.length);
  for (const ref of refs) assert.match(ref, /^save-[a-zA-Z0-9-]+$/);
  assert.deepEqual(plain(await repository(memory).load()), data);
  await repository(memory).save(data);
  assert.deepEqual(plain(await repository(memory).load()), data);
});

test("prototype-like project IDs round-trip as ordinary projects", async () => {
  const projects = JSON.parse('{"__proto__":{"name":"Prototype","art":"prototype-art"},"constructor":{"name":"Constructor","art":"constructor-art"}}');
  const data = { currentProjectId: "__proto__", projects };
  const memory = await seed(data);
  assert.deepEqual(plain(await repository(memory).load()), data);
});

test("missing or corrupt settings recover the previous committed manifest", async (t) => {
  for (const corrupt of [false, true]) {
    await t.test(corrupt ? "corrupt settings" : "missing settings", async () => {
      const memory = await seed();
      await repository(memory).save(changed);
      assert.ok(memory.get(PREVIOUS_SETTINGS));
      if (corrupt) memory.put(SETTINGS, "{broken");
      else memory.entries.delete(SETTINGS);
      const warnings = [];
      assert.deepEqual(plain(await repository(memory, warnings).load()), original);
      assert.ok(warnings.length, "Manifest recovery must be reported");
    });
  }
});

test("missing, corrupt, or v1 settings recover legacy folder and loose-file projects", async (t) => {
  for (const settings of [null, "{broken", JSON.stringify({ version: 1, currentProjectId: "legacy" })]) {
    await t.test(settings === null ? "missing settings" : settings[1] === "b" ? "corrupt settings" : "v1 settings", async () => {
      const memory = new MemoryFs();
      if (settings !== null) memory.put(SETTINGS, settings);
      memory.put(`${PROJECTS}/legacy/project.json`, JSON.stringify({
        version: 1, projectId: "legacy", project: { name: "Legacy", art: "asset://picture.png" },
      }));
      memory.put(`${PROJECTS}/legacy/assets/picture.png`, "legacy-art");
      memory.put(`${PROJECTS}/loose.json`, JSON.stringify({
        version: 1, projectId: "loose", project: { name: "Loose", art: "inline-art" },
      }));
      const warnings = [];
      const loaded = await repository(memory, warnings).load();
      assert.deepEqual(plain(loaded.projects), {
        legacy: { name: "Legacy", art: "legacy-art" },
        loose: { name: "Loose", art: "inline-art" },
      });
      assert.ok(Object.hasOwn(loaded.projects, loaded.currentProjectId));
      await repository(memory, warnings).save(loaded);
      assert.equal(memory.manifest().version, 2);
      assert.deepEqual(plain(await repository(memory).load()), plain(loaded));
      assert.ok(memory.get(`${PROJECTS}/legacy/assets/picture.png`), "Migration preserves original legacy artwork");
      assert.ok(memory.get(`${PROJECTS}/loose.json`), "Migration preserves original loose project files");
    });
  }
});

test("an unreadable legacy folder is preserved through migration", async () => {
  const memory = new MemoryFs();
  memory.put(SETTINGS, JSON.stringify({ version: 1, currentProjectId: "good" }));
  memory.put(`${PROJECTS}/good/project.json`, JSON.stringify({
    version: 1, projectId: "good", project: { name: "Good", art: "good-art" },
  }));
  memory.put(`${PROJECTS}/damaged/project.json`, "damaged project");
  memory.put(`${PROJECTS}/damaged/assets/picture.png`, "precious-art");
  const before = memory.subtree(`${PROJECTS}/damaged`);
  const warnings = [];
  const store = repository(memory, warnings);
  await store.save(await store.load());
  assert.deepEqual(memory.subtree(`${PROJECTS}/damaged`), before);
  assert.ok(warnings.length);
});

function legacySeed() {
  const memory = new MemoryFs();
  memory.put(SETTINGS, JSON.stringify({ version: 1, currentProjectId: "a" }));
  memory.put(`${PROJECTS}/a/project.json`, JSON.stringify({
    version: 1, projectId: "a", project: { ...original.projects.a, art: "asset://picture.png" },
  }));
  memory.put(`${PROJECTS}/a/assets/picture.png`, original.projects.a.art);
  memory.put(`${PROJECTS}/b.json`, JSON.stringify({
    version: 1, projectId: "b", project: original.projects.b,
  }));
  return memory;
}

test("the first migrated snapshots recover from original legacy projects if damaged", async (t) => {
  for (const corruption of ["folder project", "folder artwork", "loose project"]) {
    await t.test(corruption, async () => {
      const memory = legacySeed();
      const store = repository(memory);
      assert.deepEqual(plain(await store.load()), original);
      await store.save(changed);
      const manifest = memory.manifest();
      assert.equal(memory.manifest(PREVIOUS_SETTINGS).projects.a, "a");
      assert.equal(memory.manifest(PREVIOUS_SETTINGS).projects.b, "b.json");
      const id = corruption === "loose project" ? "b" : "a";
      const directory = `${PROJECTS}/${manifest.projects[id]}`;
      if (corruption === "folder artwork") memory.entries.delete(`${directory}/assets/picture.png`);
      else memory.put(`${directory}/project.json`, "damaged migrated snapshot");
      const warnings = [];
      const loaded = await repository(memory, warnings).load();
      assert.deepEqual(plain(loaded.projects[id]), original.projects[id]);
      assert.ok(warnings.length);
      assert.equal(memory.get(`${PROJECTS}/a/assets/picture.png`), original.projects.a.art);
      assert.ok(memory.get(`${PROJECTS}/b.json`));
    });
  }
});

test("failed or interrupted first migration leaves every original legacy project intact", async (t) => {
  await assertPrecommitFailures(t, changed, {}, legacySeed());
});

test("atomic replacement of an existing manifest works on the actual filesystem", async () => {
  const root = await fs.promises.mkdtemp(path.join(os.tmpdir(), "jokerforge-storage-"));
  const target = path.join(root, "settings.json");
  const staging = path.join(root, "settings.tmp.json");
  try {
    await fs.promises.writeFile(target, "old manifest");
    await fs.promises.writeFile(staging, "new manifest");
    await fs.promises.rename(staging, target);
    assert.equal(await fs.promises.readFile(target, "utf8"), "new manifest");
  } finally {
    // `root` is the exact directory returned by mkdtemp, never a computed deletion target.
    assert.equal(path.dirname(path.resolve(root)), path.resolve(os.tmpdir()));
    assert.ok(path.basename(root).startsWith("jokerforge-storage-"));
    await fs.promises.rm(root, { recursive: true, force: true });
  }
});

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

function storageModule(memory = new MemoryFs(), projectStore = ProjectFileStore) {
  const removedPreferences = [];
  const alerts = [];
  const stateUpdates = [];
  const storage = new Map();
  const api = loadTypeScript(
    path.join(__dirname, "../src/lib/services/storage.ts"),
    {
      react: {
        useState: (initializer) => [
          typeof initializer === "function" ? initializer() : initializer,
          (value) => stateUpdates.push(value),
        ],
        useCallback: (callback) => callback,
        useRef: (value) => ({ current: value }),
        useEffect: () => {},
      },
      "@tauri-apps/api/path": {
        appDataDir: async () => "/appdata",
        join: memory.adapter.join,
      },
      "@tauri-apps/plugin-fs": {
        ...memory.adapter,
        writeFile: async (file, bytes) =>
          memory.adapter.writeTextFile(file, Buffer.from(bytes).toString("base64")),
        readFile: async (file) =>
          Uint8Array.from(Buffer.from(await memory.adapter.readTextFile(file), "base64")),
      },
      "@/lib/core/localization": {
        DEFAULT_LOCALIZATION_LANGUAGE: "en-us",
        ensureLocalizableWithLanguage: (item) => item,
        normalizeLanguageValue: (value) => value,
      },
      "@/lib/balatro/balatro-utils": { updateDataRegistry: () => {} },
      "@/lib/app/global-alerts-bus": { pushGlobalAlert: (alert) => alerts.push(alert) },
      "@/lib/app/theme-manager": { clearThemeStorage: () => {} },
      "@/lib/items/item-order": { ensureUniqueItemOrderValues: (items) => items },
      "@/lib/services/project-file-store": { ProjectFileStore: projectStore },
    },
    `\nexport const testStorageInternals = {
      externalizeProjectImages,
      hydrateProjectImages,
      sanitizeProjectData,
      getTauriProjectFiles,
      loadStoredStore,
      getCached: () => cachedProjectStore,
      getQueue: () => persistQueue,
    };\n`,
    {
      // The tests exercise the desktop branch while stubbing React's rendering.
      window: {
        __TAURI_INTERNALS__: {},
        localStorage: {
          getItem: (key) => storage.get(key) ?? null,
          setItem: (key, value) => storage.set(key, value),
          removeItem: (key) => { removedPreferences.push(key); storage.delete(key); },
        },
        addEventListener: () => {},
        removeEventListener: () => {},
        dispatchEvent: () => {},
      },
      Event: class Event { constructor(type) { this.type = type; } },
      CustomEvent: class CustomEvent {
        constructor(type, init = {}) { this.type = type; this.detail = init.detail; }
      },
      DOMException,
      console: { warn: () => {}, log: () => {}, error: () => {} },
      setTimeout: () => 1,
      clearTimeout: () => {},
    },
  );
  return { api, internals: api.testStorageInternals, removedPreferences, alerts, stateUpdates };
}

const imageDataUrl = (label) => `data:image/png;base64,${Buffer.from(label).toString("base64")}`;

test("desktop artwork loading rejects missing and invalid references", async (t) => {
  for (const ref of [
    "asset://missing.png", "asset://../outside.png", "asset://", "asset:///absolute.png",
    "asset://C:/outside.png", "asset://nested\\outside.png",
  ]) {
    await t.test(ref, async () => {
      const { internals } = storageModule();
      const project = internals.sanitizeProjectData({ metadata: { iconImage: ref } });
      await assert.rejects(internals.hydrateProjectImages(project, "/art"));
      assert.equal(project.metadata.iconImage, ref, "Loading must not erase the saved reference");
    });
  }
});

test("desktop artwork with colliding item and layer IDs round-trips without overwriting", async () => {
  const memory = new MemoryFs();
  await memory.adapter.mkdir("/art", { recursive: true });
  const { internals } = storageModule(memory);
  const layerIds = ["same:id", "same/id", "SAME_ID", "same_id", "same:id"];
  const project = internals.sanitizeProjectData({
    metadata: { iconImage: imageDataUrl("icon"), gameImage: imageDataUrl("logo") },
    jokers: ["item:id", "item/id", "ITEM_ID"].map((id, index) => ({
      id,
      image: imageDataUrl(`base-${index}`),
      overlayImage: imageDataUrl(`overlay-${index}`),
      imageLayers: layerIds.map((layerId, layerIndex) => ({
        id: layerId, imageDataUrl: imageDataUrl(`layer-${index}-${layerIndex}`),
      })),
    })),
  });
  const stored = await internals.externalizeProjectImages(project, "/art");
  const refs = stored.jokers.flatMap((item) => [
    item.image, item.overlayImage, ...item.imageLayers.map((layer) => layer.imageDataUrl),
  ]);
  assert.equal(new Set(refs.map((ref) => ref.toLowerCase())).size, refs.length);
  assert.deepEqual(plain(await internals.hydrateProjectImages(stored, "/art")), plain(project));
});

test("desktop saves reject unresolved artwork on metadata, items, overlays, and layers", async (t) => {
  for (const input of [
    { metadata: { iconImage: "asset://saved-icon.png" } },
    { metadata: { gameImage: "asset://saved-logo.png" } },
    { jokers: [{ id: "a", image: "asset://saved-image.png" }] },
    { jokers: [{ id: "a", overlayImage: "asset://saved-overlay.png" }] },
    { jokers: [{ id: "a", imageLayers: [{ id: "layer", imageDataUrl: "asset://saved-layer.png" }] }] },
  ]) {
    await t.test(JSON.stringify(input), async () => {
      const memory = new MemoryFs();
      await memory.adapter.mkdir("/art", { recursive: true });
      const { internals } = storageModule(memory);
      const project = internals.sanitizeProjectData(input);
      await assert.rejects(internals.externalizeProjectImages(project, "/art"), /Unresolved saved artwork/);
    });
  }
});

function desktopFixture(internals) {
  return {
    currentProjectId: "a",
    projects: {
      a: internals.sanitizeProjectData({ metadata: { id: "a", name: "Disk A" }, jokers: [{ id: "original-joker" }] }),
      b: internals.sanitizeProjectData({ metadata: { id: "b", name: "Disk B" } }),
    },
  };
}

const settle = () => new Promise((resolve) => setImmediate(resolve));

function importFixture(internals, label, metadata = {}) {
  return internals.sanitizeProjectData({
    metadata: {
      author: [`${label} author`],
      description: `${label} description`,
      version: "2.3.4",
      prefix: `${label}_prefix`,
      display_name: `${label} display name`,
      dependencies: ["Steamodded (>=1.0.0)"],
      iconImage: imageDataUrl(`${label} icon`),
      gameImage: imageDataUrl(`${label} logo`),
      ...metadata,
    },
    stats: { jokers: 1 },
    jokers: [{
      id: `${label}-joker`,
      name: `${label} joker`,
      image: imageDataUrl(`${label} artwork`),
      overlayImage: imageDataUrl(`${label} overlay`),
      imageLayers: [{ id: `${label}-layer`, imageDataUrl: imageDataUrl(`${label} layer`) }],
      rules: [{
        id: `${label}-rule`,
        trigger: "hand_played",
        conditions: [{ type: "poker_hand", operator: "equals", value: "Flush" }],
        effects: [{ type: "add_mult", value: 10 }],
      }],
    }],
  });
}

async function seedDesktopProjects(memory, projects) {
  const storage = storageModule(memory);
  const data = {
    version: 2,
    currentProjectId: Object.keys(projects)[0],
    projects,
  };
  await storage.internals.getTauriProjectFiles().save(data);
  await storage.internals.loadStoredStore();
  return storage;
}

async function assertImportSurvivesRestart(memory, storage, originals, importIds, imported) {
  await storage.internals.getQueue();
  const restarted = storageModule(memory);
  const loaded = await restarted.internals.loadStoredStore();
  assert.equal(loaded.currentProjectId, importIds.at(-1), "The latest import remains active after restarting");
  assert.deepEqual(Object.keys(loaded.projects).sort(), [...Object.keys(originals), ...importIds].sort());
  for (const [id, project] of Object.entries(originals)) {
    assert.deepEqual(plain(loaded.projects[id]), plain(project), `Import must preserve original ${id}, including artwork and rules`);
  }
  for (const id of importIds) {
    assert.deepEqual(plain(loaded.projects[id]), plain(imported), "The imported mod identity, metadata, artwork, and rules must survive");
  }
  assert.equal(storage.alerts.length, 0, "A successful import should not produce storage warnings");
  assert.equal(restarted.alerts.length, 0);
}

test("import creates an independent workspace project even when names or mod IDs match", async (t) => {
  const cases = [
    { name: "same name, different mod ID", key: "existing_mod", existingId: "existing_mod", importedId: "incoming_mod", importedName: "Shared Name" },
    { name: "case and whitespace matching name, different mod ID", key: "existing_mod", existingId: "existing_mod", importedId: "incoming_mod", importedName: "  sHaReD nAmE  " },
    { name: "same name and mod ID", key: "existing_mod", existingId: "existing_mod", importedId: "existing_mod", importedName: "Shared Name" },
    { name: "same mod ID, different name", key: "existing_mod", existingId: "existing_mod", importedId: "existing_mod", importedName: "New Name" },
    { name: "mod ID edited away from its workspace key", key: "original_workspace", existingId: "claimed_mod", importedId: "claimed_mod", importedName: "Shared Name" },
    { name: "missing imported mod ID uses defaults safely", key: "my_custom_mod", existingId: "my_custom_mod", importedName: "Shared Name" },
  ];
  for (const input of cases) {
    await t.test(input.name, async () => {
      const memory = new MemoryFs();
      const { internals } = storageModule(memory);
      const existing = importFixture(internals, "existing", { id: input.existingId, name: "Shared Name" });
      const metadata = { name: input.importedName };
      if (Object.hasOwn(input, "importedId")) metadata.id = input.importedId;
      const imported = importFixture(internals, "imported", metadata);
      const originals = { [input.key]: existing };
      const storage = await seedDesktopProjects(memory, originals);
      const importedBefore = plain(imported);
      storage.api.useProjectData().importProject(imported);
      const cached = storage.internals.getCached();
      const importId = cached.currentProjectId;
      assert.notEqual(importId, input.key, "Import must not reuse an existing workspace identity");
      assert.equal(Object.keys(cached.projects).length, 2);
      assert.deepEqual(plain(cached.projects[input.key]), plain(existing));
      assert.deepEqual(plain(cached.projects[importId]), importedBefore);
      assert.deepEqual(plain(imported), importedBefore, "Import must not mutate the source project");
      const summary = storage.api.useProjectData().projects.find((project) => project.id === importId);
      assert.equal(summary.modId, imported.metadata.id, "Project summaries distinguish workspace identity from the exported mod ID");
      await assertImportSurvivesRestart(memory, storage, originals, [importId], imported);
    });
  }
});

test("back-to-back imports of the same project retain every workspace copy", async () => {
  const memory = new MemoryFs();
  const { internals } = storageModule(memory);
  const existing = importFixture(internals, "existing", { id: "shared_mod", name: "Shared Name" });
  const imported = importFixture(internals, "imported", { id: "shared_mod", name: "Shared Name" });
  const originals = { shared_mod: existing };
  const storage = await seedDesktopProjects(memory, originals);
  const hook = storage.api.useProjectData();
  const importIds = [];
  for (let index = 0; index < 3; index += 1) {
    hook.importProject(imported);
    importIds.push(storage.internals.getCached().currentProjectId);
  }
  assert.equal(new Set(["shared_mod", ...importIds]).size, 4, "Queued imports require separate workspace identities");
  assert.equal(Object.keys(storage.internals.getCached().projects).length, 4);
  await assertImportSurvivesRestart(memory, storage, originals, importIds, imported);
});

test("switching and deleting imported copies uses workspace identity instead of shared mod ID", async () => {
  const memory = new MemoryFs();
  const { internals } = storageModule(memory);
  const existing = importFixture(internals, "existing", { id: "shared_mod", name: "Shared Name" });
  const imported = importFixture(internals, "imported", { id: "shared_mod", name: "Shared Name" });
  const storage = await seedDesktopProjects(memory, { shared_mod: existing });
  const hook = storage.api.useProjectData();
  hook.importProject(imported);
  const firstId = storage.internals.getCached().currentProjectId;
  hook.importProject(imported);
  const secondId = storage.internals.getCached().currentProjectId;
  const summary = storage.api.useProjectData().projects;
  assert.deepEqual(plain(summary.map((project) => project.id)), ["shared_mod", firstId, secondId]);
  assert.ok(summary.every((project) => project.modId === "shared_mod"));
  hook.switchProject("shared_mod");
  assert.deepEqual(plain(storage.api.useProjectData().data), plain(existing));
  hook.switchProject(firstId);
  assert.equal(storage.internals.getCached().currentProjectId, firstId);
  assert.deepEqual(plain(storage.api.useProjectData().data), plain(imported));
  hook.deleteProject(firstId);
  const cached = storage.internals.getCached();
  assert.equal(Object.hasOwn(cached.projects, firstId), false);
  assert.equal(Object.hasOwn(cached.projects, cached.currentProjectId), true);
  assert.deepEqual(plain(cached.projects.shared_mod), plain(existing));
  assert.deepEqual(plain(cached.projects[secondId]), plain(imported));
  hook.switchProject(secondId);
  await assertImportSurvivesRestart(memory, storage, { shared_mod: existing }, [secondId], imported);
});

test("import preserves unreadable disk projects with the imported mod ID", async () => {
  const memory = new MemoryFs();
  const { internals } = storageModule(memory);
  const healthy = importFixture(internals, "healthy", { id: "healthy_mod", name: "Healthy" });
  const damaged = importFixture(internals, "damaged", { id: "shared_mod", name: "Shared Name" });
  const imported = importFixture(internals, "imported", { id: "shared_mod", name: "Shared Name" });
  await seedDesktopProjects(memory, { healthy_mod: healthy, shared_mod: damaged });
  const settingsPath = "/appdata/joker_forge_storage/settings.json";
  const projectsPath = "/appdata/joker_forge_storage/projects";
  const reference = memory.manifest(settingsPath).projects.shared_mod;
  const damagedRoot = `${projectsPath}/${reference}`;
  const damagedFile = `${damagedRoot}/project.json`;
  const before = memory.subtree(damagedRoot);
  memory.blockedReads.add(damagedFile);
  const storage = storageModule(memory);
  const loaded = await storage.internals.loadStoredStore();
  assert.equal(Object.hasOwn(loaded.projects, "shared_mod"), false, "The unreadable project is absent from the editor");
  storage.api.useProjectData().importProject(imported);
  const importId = storage.internals.getCached().currentProjectId;
  assert.notEqual(importId, "shared_mod");
  await storage.internals.getQueue();
  assert.equal(memory.manifest(settingsPath).projects.shared_mod, reference, "Import must keep the unreadable project's recovery reference");
  assert.deepEqual(memory.subtree(damagedRoot), before, "Import must preserve unreadable project files and artwork");
  memory.blockedReads.delete(damagedFile);
  const restarted = storageModule(memory);
  const recovered = await restarted.internals.loadStoredStore();
  assert.equal(recovered.currentProjectId, importId);
  assert.deepEqual(plain(recovered.projects.healthy_mod), plain(healthy));
  assert.deepEqual(plain(recovered.projects.shared_mod), plain(damaged));
  assert.deepEqual(plain(recovered.projects[importId]), plain(imported));
  assert.equal(Object.keys(recovered.projects).length, 3);
});

test("imports queued during hydration preserve loaded projects and each incoming copy", async () => {
  const hydration = deferred();
  const saved = [];
  class PendingStore {
    async load() { return hydration.promise; }
    async save(data, options) { saved.push({ data: plain(data), options }); }
  }
  const storage = storageModule(new MemoryFs(), PendingStore);
  const existing = importFixture(storage.internals, "existing", { id: "shared_mod", name: "Shared Name" });
  const imported = importFixture(storage.internals, "imported", { id: "shared_mod", name: "Shared Name" });
  const disk = { version: 2, currentProjectId: "shared_mod", projects: { shared_mod: existing } };
  const hook = storage.api.useProjectData();
  hook.importProject(imported);
  hook.importProject(imported);
  assert.equal(saved.length, 0, "Import waits for the initial disk load");
  hydration.resolve(disk);
  await settle();
  await storage.internals.getQueue();
  const cached = storage.internals.getCached();
  const importIds = Object.keys(cached.projects).filter((id) => id !== "shared_mod");
  assert.equal(importIds.length, 2);
  assert.equal(cached.currentProjectId, importIds.at(-1));
  assert.deepEqual(plain(cached.projects.shared_mod), plain(existing));
  for (const id of importIds) assert.deepEqual(plain(cached.projects[id]), plain(imported));
  assert.equal(saved.length, 1, "Autosave coalesces the queued imports into one complete snapshot");
  assert.deepEqual(saved[0].data, plain(cached));
  assert.equal(saved[0].options.deletedProjectIds.size, 0, "Import must not delete loaded projects");
});

test("edits during initial hydration apply to loaded projects and retain coalesced deletions", async () => {
  const hydration = deferred();
  const saved = [];
  class PendingStore {
    async load() { return hydration.promise; }
    async save(data, options) { saved.push({ data: plain(data), options }); }
  }
  const { api, internals } = storageModule(new MemoryFs(), PendingStore);
  const loaded = desktopFixture(internals);
  const hook = api.useProjectData();
  let updaterCalls = 0;
  hook.updateMetadata(() => { updaterCalls += 1; return { name: "Early edit" }; });
  hook.deleteProject("b");
  hook.updateMetadata({ prefix: "changed" });
  assert.equal(saved.length, 0, "Early edits must wait for disk hydration");
  assert.equal(updaterCalls, 0);
  hydration.resolve(loaded);
  await settle();
  await internals.getQueue();
  const cached = internals.getCached();
  assert.equal(cached.currentProjectId, "a");
  assert.equal(cached.projects.a.metadata.name, "Early edit");
  assert.equal(cached.projects.a.metadata.prefix, "changed");
  assert.equal(cached.projects.a.jokers[0].id, "original-joker");
  assert.equal(cached.projects.b, undefined);
  assert.equal(updaterCalls, 1, "React updater replay cannot duplicate an edit");
  assert.equal(saved.length, 1, "Only the final queued snapshot is saved");
  assert.deepEqual([...saved[0].options.deletedProjectIds], ["b"]);
  assert.equal(saved[0].data.projects.a.metadata.name, "Early edit");
});

test("a failed reset waits for earlier saves and preserves preferences and active projects", async () => {
  const earlierSave = deferred();
  const resetSave = deferred();
  const saves = [];
  let disk;
  class PendingStore {
    async load() { return disk; }
    async save(data, options) {
      saves.push({ data: plain(data), options });
      return options.reset ? resetSave.promise : earlierSave.promise;
    }
  }
  const { api, internals, removedPreferences, alerts } = storageModule(new MemoryFs(), PendingStore);
  disk = desktopFixture(internals);
  await internals.loadStoredStore();
  api.useProjectData().updateMetadata({ name: "Unsaved edit" });
  await settle();
  const reset = api.resetProjectData();
  await settle();
  assert.equal(saves.length, 1, "Reset must queue behind the outstanding autosave");
  assert.equal(removedPreferences.length, 0);
  earlierSave.resolve();
  await settle();
  assert.equal(saves.length, 2);
  assert.equal(saves[1].options.reset, true);
  assert.equal(removedPreferences.length, 0, "Preferences must wait until the reset commits");
  resetSave.reject(new Error("Disk full"));
  assert.equal(await reset, false);
  assert.equal(removedPreferences.length, 0);
  assert.equal(internals.getCached().projects.a.metadata.name, "Unsaved edit");
  assert.ok(alerts.some((alert) => alert.title === "Save Failed"));
  // A failed reset must release its lock so users can continue editing.
  api.useProjectData().updateMetadata({ name: "After failed reset" });
  await internals.getQueue();
  assert.equal(internals.getCached().projects.a.metadata.name, "After failed reset");
});

test("successful reset clears preferences only after its file commit", async () => {
  const resetSave = deferred();
  let disk;
  class PendingStore {
    async load() { return disk; }
    async save(_data, options) {
      assert.equal(options.reset, true);
      return resetSave.promise;
    }
  }
  const { api, internals, removedPreferences } = storageModule(new MemoryFs(), PendingStore);
  disk = desktopFixture(internals);
  await internals.loadStoredStore();
  const reset = api.resetProjectData();
  await settle();
  assert.equal(removedPreferences.length, 0);
  resetSave.resolve();
  assert.equal(await reset, true);
  assert.ok(removedPreferences.length > 0);
  assert.equal(Object.hasOwn(internals.getCached().projects, "a"), false);
});

test("reset waits for initial hydration and cancels edits deferred before the reset", async () => {
  const hydration = deferred();
  const saved = [];
  class PendingStore {
    async load() { return hydration.promise; }
    async save(data, options) { saved.push({ data: plain(data), options }); }
  }
  const { api, internals, removedPreferences } = storageModule(new MemoryFs(), PendingStore);
  const disk = desktopFixture(internals);
  const hook = api.useProjectData();
  let updaterCalls = 0;
  hook.updateMetadata(() => { updaterCalls += 1; return { name: "Late edit" }; });
  const reset = api.resetProjectData();
  await settle();
  assert.equal(saved.length, 0, "Reset cannot race with the initial load or migration");
  assert.equal(removedPreferences.length, 0);
  hydration.resolve(disk);
  assert.equal(await reset, true);
  await settle();
  await internals.getQueue();
  assert.equal(updaterCalls, 0, "A pre-reset deferred edit cannot overwrite the reset project");
  assert.equal(saved.length, 1);
  assert.equal(saved[0].options.reset, true);
  assert.equal(Object.hasOwn(internals.getCached().projects, "a"), false);
});

test("a whole-store load failure saves to a separate recovery identity", async () => {
  let disk;
  const saved = [];
  class UnreadableStore {
    constructor(options) { this.delegate = new ProjectFileStore(options); }
    async load() { throw new Error("App data folder temporarily unreadable"); }
    async save(data, options) {
      saved.push({ data: plain(data), options });
      await this.delegate.save(data, options);
    }
  }
  const { api, internals, alerts } = storageModule(new MemoryFs(), UnreadableStore);
  const existing = internals.sanitizeProjectData({
    metadata: { id: "my_custom_mod", name: "Existing disk project" },
    jokers: [{ id: "precious-joker", image: imageDataUrl("precious-art") }],
  });
  disk = { currentProjectId: "my_custom_mod", projects: { my_custom_mod: existing } };
  const originalProject = plain(existing);
  const files = internals.getTauriProjectFiles();
  await files.delegate.save(disk);
  const loaded = await internals.loadStoredStore();
  assert.match(loaded.currentProjectId, /^recovery_/);
  assert.equal(loaded.projects[loaded.currentProjectId].metadata.id, loaded.currentProjectId);
  api.useProjectData().updateMetadata({ name: "Recovery draft" });
  await internals.getQueue();
  assert.equal(saved.length, 1);
  assert.equal(saved[0].data.currentProjectId, loaded.currentProjectId);
  assert.equal(Object.hasOwn(saved[0].data.projects, "my_custom_mod"), false);
  assert.equal(saved[0].options.deletedProjectIds.has("my_custom_mod"), false);
  const diskAfterSave = await files.delegate.load();
  assert.deepEqual(plain(diskAfterSave.projects.my_custom_mod), originalProject);
  assert.equal(diskAfterSave.projects[loaded.currentProjectId].metadata.name, "Recovery draft");
  assert.ok(alerts.some((alert) => alert.title === "Project Recovery"));
});

test("the desktop layout withholds editors and project actions until hydration finishes", () => {
  const jsxRuntime = require("react/jsx-runtime");
  const Sidebar = () => null;
  const Header = () => null;
  const TitleBar = () => null;
  const GlobalAlerts = () => null;
  let hydrating = true;
  const { MainLayout } = loadTypeScript(
    path.join(__dirname, "../src/components/layout/main-layout.tsx"),
    {
      "react/jsx-runtime": jsxRuntime,
      react: {
        useEffect: () => {},
        useRef: (value) => ({ current: value }),
        useState: (value) => [value, () => {}],
      },
      "./sidebar": { Sidebar },
      "./header": { Header },
      "./title-bar": { TitleBar },
      "./global-alerts": { GlobalAlerts },
      "framer-motion": { motion: { main: "main" } },
      "@/hooks/use-alert-queue": {
        useAlertQueue: () => ({ alerts: [], pushAlerts: () => {}, dismissAlert: () => {} }),
      },
      "@/lib/balatro/balatro-autofind": { runBalatroAutofind: async () => [] },
      "@/lib/services/storage": { useProjectData: () => ({ isHydrating: hydrating }) },
      "@/lib/app/global-alerts-bus": { GLOBAL_ALERTS_EVENT: "alerts" },
    },
  );
  const editor = jsxRuntime.jsx("button", { onClick: () => {}, children: "Delete Joker" });
  function nodes(tree) {
    if (Array.isArray(tree)) return tree.flatMap(nodes);
    if (!tree || typeof tree !== "object") return [];
    return [tree, ...nodes(tree.props?.children)];
  }
  const loading = nodes(MainLayout({ children: editor, pageTitle: "Jokers" }));
  assert.ok(loading.some((node) => node.type === TitleBar));
  assert.ok(loading.some((node) => node.type === GlobalAlerts));
  assert.ok(loading.some((node) => node.props?.role === "status"));
  assert.equal(loading.some((node) => node.type === Sidebar || node.type === Header), false);
  assert.equal(loading.includes(editor), false, "Captured empty-array editor actions cannot run while loading");
  hydrating = false;
  const ready = nodes(MainLayout({ children: editor, pageTitle: "Jokers" }));
  assert.ok(ready.some((node) => node.type === Sidebar));
  assert.ok(ready.some((node) => node.type === Header));
  assert.ok(ready.includes(editor));
  assert.equal(ready.some((node) => node.props?.role === "status"), false);
});
