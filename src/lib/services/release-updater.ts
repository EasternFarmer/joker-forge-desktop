import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { gt } from "semver";
import { RELEASE_CHANNEL } from "@/generated/release-channel";
import { flushPendingProjectSaves, hasPendingProjectSaves } from "./storage";
import { flushPendingTemplateSaves, hasPendingTemplateSaves } from "@/lib/content/templates";
import {
  isOfficialReleaseUrl,
  normalizeVersion,
  selectInstallerAsset,
  selectLatestRelease,
  type GitHubRelease,
  type GitHubReleaseAsset,
  type ReleaseChannel,
  type UpdatePlatform,
} from "./release-update-policy";

const API_BASE = "https://api.github.com/repos/Jaydchw/joker-forge-desktop";
const UPDATE_CHECK_DEV_OVERRIDE = import.meta.env.VITE_ENABLE_UPDATE_CHECK_IN_DEV === "true";
const UPDATE_TEST_CHANNEL =
  import.meta.env.VITE_UPDATE_TEST_CHANNEL === "nightly" ||
  import.meta.env.VITE_UPDATE_TEST_CHANNEL === "stable"
    ? import.meta.env.VITE_UPDATE_TEST_CHANNEL as ReleaseChannel : null;
const UPDATE_TEST_CURRENT_VERSION = import.meta.env.VITE_UPDATE_TEST_CURRENT_VERSION?.trim() || null;

export interface UpdateInfo {
  currentVersion: string;
  latestVersion: string;
  channel: ReleaseChannel;
  asset: GitHubReleaseAsset | null;
  releaseUrl: string;
  installation: "automatic" | "manual";
}

let checkInProgress: Promise<void> | null = null;
let hasCheckedForUpdateOnLaunch = false;
let availableUpdate: UpdateInfo | null = null;
let updateInProgress: Promise<void> | null = null;
const listeners = new Set<(info: UpdateInfo) => void>();

export const onUpdateAvailable = (listener: (info: UpdateInfo) => void) => {
  listeners.add(listener);
  // The launch check can finish before the dialog mounts.
  if (availableUpdate) listener(availableUpdate);
  return () => { listeners.delete(listener); };
};

async function flushPendingSaves(): Promise<void> {
  for (;;) {
    await Promise.all([flushPendingProjectSaves(), flushPendingTemplateSaves()]);
    // A write to one library can arrive while the other library is saving.
    if (!hasPendingProjectSaves() && !hasPendingTemplateSaves()) return;
  }
}

async function installUpdate(info: UpdateInfo): Promise<void> {
  if (!isOfficialReleaseUrl(info.releaseUrl)) {
    throw new Error("This update does not point to an official Joker Forge release.");
  }
  if (info.installation === "manual") {
    await openUrl(info.releaseUrl);
    return;
  }
  const platform = await invoke<UpdatePlatform>("get_update_platform");
  const asset = info.asset;
  const releasePath = new URL(info.releaseUrl).pathname.split("/");
  const matchingAsset = asset && selectInstallerAsset({
    name: info.latestVersion,
    tag_name: decodeURIComponent(releasePath[releasePath.length - 1]),
    html_url: info.releaseUrl, prerelease: info.channel === "nightly", draft: false, assets: [asset],
  }, platform);
  if (!matchingAsset) {
    throw new Error("An automatic installer is not available for this computer. Download the update from the release page.");
  }
  const localPath = await invoke<string>("download_release_asset", {
    url: matchingAsset.browser_download_url,
    fileName: matchingAsset.name,
    expectedSize: matchingAsset.size ?? null,
    expectedDigest: matchingAsset.digest ?? null,
  });
  let handedOff = false;
  try {
    await flushPendingSaves();
    await invoke("install_update_and_restart", { installerPath: localPath });
    handedOff = true;
    // Native validation and helper startup can take time; include late edits.
    await flushPendingSaves();
    await getCurrentWindow().close();
  } catch (error) {
    let cancellationError: unknown;
    if (handedOff) {
      try {
        await invoke("cancel_update_install", { installerPath: localPath });
      } catch (cancelError) {
        cancellationError = cancelError;
      }
    }
    try {
      await invoke("discard_update_download", { installerPath: localPath });
    } catch (cleanupError) {
      console.warn("[release-updater] Could not remove the unused installer", cleanupError);
    }
    if (cancellationError) {
      throw new Error(`${String(error)} The update helper could not be cancelled: ${String(cancellationError)}`);
    }
    throw error;
  }
}

export const performUpdate = (info: UpdateInfo): Promise<void> => {
  if (updateInProgress) return updateInProgress;
  const operation = installUpdate(info);
  updateInProgress = operation;
  void operation.finally(() => {
    if (updateInProgress === operation) updateInProgress = null;
  }).catch(() => {});
  return operation;
};

async function checkForUpdate(): Promise<void> {
  const currentVersion = UPDATE_TEST_CURRENT_VERSION ?? await getVersion();
  const currentNormalized = normalizeVersion(currentVersion);
  if (!currentNormalized) return;
  const channel = UPDATE_TEST_CHANNEL ??
    (RELEASE_CHANNEL === "nightly" || currentVersion.includes("-nightly.") ? "nightly" : "stable");
  const response = await fetch(`${API_BASE}/releases?per_page=100`, {
    headers: { Accept: "application/vnd.github+json" },
    signal: AbortSignal.timeout(15_000),
  });
  if (!response.ok) throw new Error(`Update check failed (HTTP ${response.status}).`);
  const releases: unknown = await response.json();
  if (!Array.isArray(releases)) throw new Error("The update service returned an invalid release list.");
  const release = selectLatestRelease(releases as GitHubRelease[], channel);
  const latestVersion = release && normalizeVersion(release.tag_name);
  if (!release || !latestVersion || !gt(latestVersion, currentNormalized)) return;
  const platform = await invoke<UpdatePlatform>("get_update_platform");
  const asset = selectInstallerAsset(release, platform);
  availableUpdate = {
    currentVersion, latestVersion, channel, asset, releaseUrl: release.html_url,
    installation: asset ? "automatic" : "manual",
  };
  listeners.forEach((listener) => listener(availableUpdate!));
}

export const checkForReleaseUpdateOnLaunch = (): Promise<void> => {
  if (checkInProgress) return checkInProgress;
  if (hasCheckedForUpdateOnLaunch || (import.meta.env.DEV && !UPDATE_CHECK_DEV_OVERRIDE)) {
    return Promise.resolve();
  }
  const operation = checkForUpdate().then(() => {
    hasCheckedForUpdateOnLaunch = true;
  }).catch((error) => {
    console.warn("[release-updater] Update check failed", error);
  });
  checkInProgress = operation;
  void operation.finally(() => {
    if (checkInProgress === operation) checkInProgress = null;
  });
  return operation;
};
