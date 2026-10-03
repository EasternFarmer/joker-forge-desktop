import { gt, prerelease, valid } from "semver";

export type ReleaseChannel = "stable" | "nightly";

export interface UpdatePlatform {
  os: string;
  arch: string;
  automaticInstall: boolean;
}

export interface GitHubReleaseAsset {
  name: string;
  browser_download_url: string;
  size?: number;
  digest?: string | null;
}

export interface GitHubRelease {
  name: string;
  tag_name: string;
  html_url: string;
  prerelease: boolean;
  draft: boolean;
  assets: GitHubReleaseAsset[];
}

const OWNER = "jaydchw";
const REPOSITORY = "joker-forge-desktop";

function officialReleasePath(value: string): string[] | null {
  try {
    const url = new URL(value);
    if (
      url.protocol !== "https:" || url.hostname !== "github.com" ||
      url.username || url.password || url.port || url.search || url.hash
    ) return null;
    const path = url.pathname.split("/").slice(1).map(decodeURIComponent);
    if (
      path[0]?.toLowerCase() !== OWNER ||
      path[1]?.toLowerCase() !== REPOSITORY || path[2] !== "releases"
    ) return null;
    return path;
  } catch {
    return null;
  }
}

export function isOfficialReleaseUrl(value: string): boolean {
  const path = officialReleasePath(value);
  return !!path && path.length === 5 && path[3] === "tag" && !!path[4];
}

export function isOfficialAssetUrl(value: string, fileName: string): boolean {
  const path = officialReleasePath(value);
  const stem = fileName.split(".")[0].trimEnd().toUpperCase();
  const reserved = /^(?:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])$/.test(stem);
  return !!path && path.length === 6 && path[3] === "download" &&
    !!path[4] && path[5] === fileName && fileName.length <= 200 && !reserved &&
    /^[A-Za-z0-9][A-Za-z0-9 ._()-]*\.exe$/i.test(fileName);
}

export function normalizeVersion(value: string): string | null {
  if (typeof value !== "string") return null;
  const version = value.replace(/^nightly-/i, "").replace(/^v/i, "")
    .replace(/\.local$/, ".0")
    .split(".")
    .map((part) => /^\d+$/.test(part) ? String(Number(part)) : part)
    .join(".");
  return valid(version);
}

export function selectLatestRelease(
  releases: readonly GitHubRelease[],
  channel: ReleaseChannel,
): GitHubRelease | null {
  let selected: GitHubRelease | null = null;
  let selectedVersion: string | null = null;
  for (const release of releases) {
    if (
      !release || release.draft !== false ||
      typeof release.tag_name !== "string" ||
      typeof release.html_url !== "string" ||
      !isOfficialReleaseUrl(release.html_url) || !Array.isArray(release.assets)
    ) continue;
    if (officialReleasePath(release.html_url)?.[4] !== release.tag_name) continue;
    const version = normalizeVersion(release.tag_name);
    if (!version) continue;
    const nightly = /^nightly-/i.test(release.tag_name);
    if (channel === "stable") {
      if (release.prerelease !== false || nightly || prerelease(version)) continue;
    } else if (
      release.prerelease !== true || !nightly || !version.includes("-nightly.")
    ) continue;
    if (!selectedVersion || gt(version, selectedVersion)) {
      selected = release;
      selectedVersion = version;
    }
  }
  return selected;
}

export function selectInstallerAsset(
  release: GitHubRelease,
  platform: UpdatePlatform,
): GitHubReleaseAsset | null {
  if (platform.os !== "windows" || !platform.automaticInstall) return null;
  const arch = platform.arch.toLowerCase();
  const aliases = arch === "x86_64" || arch === "x64" ? ["x64", "x86_64", "amd64"]
    : arch === "aarch64" || arch === "arm64" ? ["arm64", "aarch64"]
    : arch === "x86" || arch === "i686" ? ["x86", "i686", "i386"] : [];
  if (!aliases.length) return null;
  return release.assets.find((asset) => {
    if (
      !asset || typeof asset.name !== "string" ||
      typeof asset.browser_download_url !== "string" ||
      !isOfficialAssetUrl(asset.browser_download_url, asset.name)
    ) return false;
    if (officialReleasePath(asset.browser_download_url)?.[4] !== release.tag_name) {
      return false;
    }
    const installerArch = asset.name.match(
      /(?:^|[_. -])(x86_64|aarch64|arm64|amd64|x64|i686|i386|x86)[_. -]setup\.exe$/i,
    )?.[1].toLowerCase();
    return !!installerArch && aliases.includes(installerArch);
  }) ?? null;
}
