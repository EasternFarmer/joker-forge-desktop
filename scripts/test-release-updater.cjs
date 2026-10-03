/* Run with `node --test scripts/test-release-updater.cjs`. No downloads or installers run. */
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { test } = require("node:test");
const ts = require("typescript");

const REPO = "https://github.com/Jaydchw/joker-forge-desktop";

function loadTypeScript(relativePath, imports = {}, globals = {}, environment = {}) {
  const filePath = path.join(__dirname, "..", relativePath);
  const source = fs.readFileSync(filePath, "utf8").replaceAll("import.meta.env", "__environment");
  const compiled = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
    fileName: filePath,
  });
  const module = { exports: {} };
  vm.runInNewContext(compiled.outputText, {
    module,
    exports: module.exports,
    URL,
    AbortSignal,
    __environment: { DEV: false, ...environment },
    console: { log() {}, info() {}, warn() {}, error() {} },
    require(name) {
      if (Object.hasOwn(imports, name)) return imports[name];
      throw new Error(`Unmocked import in updater test: ${name}`);
    },
    ...globals,
  }, { filename: filePath });
  return module.exports;
}

const clone = (value) => JSON.parse(JSON.stringify(value));
const asset = (name, tag = "v2.1.0") => ({
  name,
  browser_download_url: `${REPO}/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(name)}`,
});
const release = (tag = "v2.1.0", overrides = {}) => ({
  name: tag,
  tag_name: tag,
  html_url: `${REPO}/releases/tag/${encodeURIComponent(tag)}`,
  prerelease: false,
  draft: false,
  assets: [asset("Joker.Forge_2.1.0_x64-setup.exe", tag)],
  ...overrides,
});
const windows = { os: "windows", arch: "x86_64", automaticInstall: true };

function policy() {
  return loadTypeScript("src/lib/services/release-update-policy.ts", { semver: require("semver") });
}

test("stable excludes flagged prereleases and prerelease versions mislabeled as stable", () => {
  const { selectLatestRelease } = policy();
  const candidates = [
    release("v99.0.0", { prerelease: true }),
    release("v99.0.0-beta.1"),
    release("v99.0.0-rc.1"),
    release("nightly-99.0.0-nightly.20261003.3", { prerelease: true }),
    release("v2.1.0"),
  ];
  assert.equal(selectLatestRelease(candidates, "stable")?.tag_name, "v2.1.0");
  assert.equal(selectLatestRelease(candidates.slice(0, 4), "stable"), null);
});

test("candidate selection uses highest valid version rather than GitHub ordering", () => {
  const { selectLatestRelease } = policy();
  const candidates = [
    release("v2.2.0"),
    release("v20.0.0", { draft: true }),
    release("garbage", { name: "v99.0.0" }),
    release("v2.12.0"),
    release("v2.3.0"),
  ];
  assert.equal(selectLatestRelease(candidates, "stable")?.tag_name, "v2.12.0");
  assert.equal(selectLatestRelease([...candidates].reverse(), "stable")?.tag_name, "v2.12.0");
});

test("nightly requires nightly tag, prerelease flag and a matching nightly version", () => {
  const { selectLatestRelease } = policy();
  const older = release("nightly-2.0.0-beta-nightly.20261002.8", { prerelease: true });
  const newest = release("nightly-2.0.0-beta-nightly.20261003.9", { prerelease: true });
  const candidates = [
    older,
    release("v99.0.0", { name: "Nightly", prerelease: true }),
    release("nightly-99.0.0-beta.1", { prerelease: true }),
    release("nightly-99.0.0-nightly.20261003.1"),
    release("nightly-99.0.0-nightly.20261003.1", { prerelease: true, draft: true }),
    newest,
  ];
  assert.equal(selectLatestRelease(candidates, "nightly")?.tag_name, newest.tag_name);
});

test("normalization keeps local nightlies below timestamp builds and rejects malformed versions", () => {
  const { normalizeVersion } = policy();
  assert.equal(normalizeVersion("v2.1.0"), "2.1.0");
  assert.equal(normalizeVersion("nightly-2.0.0-beta-nightly.20261003.0009"), "2.0.0-beta-nightly.20261003.9");
  assert.equal(normalizeVersion("2.0.0-beta-nightly.local"), "2.0.0-beta-nightly.0");
  assert.equal(require("semver").gt(normalizeVersion("2.0.0-beta-nightly.20261003.9"), normalizeVersion("2.0.0-beta-nightly.local")), true);
  for (const invalid of ["", "garbage", "2.1", "2.1.0.9", "v", "2.1.0-beta..1"]) {
    assert.equal(normalizeVersion(invalid), null, invalid);
  }
});

test("Windows selects only an installer for the runtime architecture", () => {
  const { selectInstallerAsset } = policy();
  const arm = asset("Joker.Forge_2.1.0_arm64-setup.exe");
  const x64 = asset("Joker.Forge_2.1.0_x64-setup.exe");
  const candidates = release("v2.1.0", { assets: [arm, x64] });
  assert.equal(selectInstallerAsset(candidates, windows)?.name, x64.name);
  assert.equal(selectInstallerAsset(candidates, { os: "windows", arch: "aarch64", automaticInstall: true })?.name, arm.name);
  assert.equal(selectInstallerAsset(release("v2.1.0", { assets: [arm] }), windows), null);
  assert.equal(selectInstallerAsset(release("v2.1.0", { assets: [asset("Joker.Forge_2.1.0_setup.exe")] }), windows), null);
  assert.equal(selectInstallerAsset(candidates, { os: "windows", arch: "unknown", automaticInstall: true }), null);
  assert.equal(selectInstallerAsset(candidates, { ...windows, automaticInstall: false }), null);
  assert.equal(selectInstallerAsset(release("v2.1.0", { assets: [asset("Joker.Forge_x64.exe.zip"), asset("Joker.Forge_x64.msi")] }), windows), null);
});

test("non-Windows platforms never get an automatic installer", () => {
  const { selectInstallerAsset } = policy();
  const candidate = release("v2.1.0", { assets: [asset("Joker.Forge_x64.exe"), asset("Joker.Forge_amd64.AppImage"), asset("Joker.Forge_amd64.deb"), asset("Joker.Forge_arm64.dmg")] });
  for (const platform of ["linux", "macos", "unsupported"]) {
    assert.equal(selectInstallerAsset(candidate, { os: platform, arch: "x86_64", automaticInstall: false }), null, platform);
  }
});

test("release links and download links are restricted to this official repository", () => {
  const { isOfficialReleaseUrl, isOfficialAssetUrl, selectLatestRelease, selectInstallerAsset } = policy();
  const good = release();
  assert.equal(isOfficialReleaseUrl(good.html_url), true);
  assert.equal(isOfficialAssetUrl(good.assets[0].browser_download_url, good.assets[0].name), true);
  for (const bad of [
    "http://github.com/Jaydchw/joker-forge-desktop/releases/tag/v2.1.0",
    "https://github.com.evil.example/Jaydchw/joker-forge-desktop/releases/tag/v2.1.0",
    "https://github.com/other/joker-forge-desktop/releases/tag/v2.1.0",
    "https://github.com/Jaydchw/other/releases/tag/v2.1.0",
    "https://github.com/Jaydchw/joker-forge-desktop/issues/1",
    "https://evil.example/releases/tag/v2.1.0",
    "javascript:alert(1)",
  ]) {
    assert.equal(isOfficialReleaseUrl(bad), false, bad);
    assert.equal(selectLatestRelease([release("v2.1.0", { html_url: bad })], "stable"), null, bad);
  }
  for (const bad of [
    "https://evil.example/Joker.Forge_x64.exe",
    `${REPO}/releases/download/v2.1.0/other.exe`,
    `${REPO}/releases/download/v2.1.0/Joker.Forge_x64.exe?source=other`,
    `${REPO}/releases/download/v2.1.0/Joker.Forge_x64.exe#fragment`,
  ]) {
    assert.equal(isOfficialAssetUrl(bad, "Joker.Forge_x64.exe"), false, bad);
    assert.equal(selectInstallerAsset(release("v2.1.0", { assets: [{ name: "Joker.Forge_x64.exe", browser_download_url: bad }] }), windows), null, bad);
  }
});

function createService(options = {}) {
  const events = [];
  const notified = [];
  let flushCalls = 0;
  let templateFlushCalls = 0;
  async function flushPendingProjectSaves() {
    events.push("flush");
    flushCalls++;
    if (options.onFlush) await options.onFlush(flushCalls);
    if (options.flushError || options.flushErrorOnCall === flushCalls) throw new Error("save failed");
  }
  async function flushPendingTemplateSaves() {
    events.push("template-flush");
    templateFlushCalls++;
    if (options.onTemplateFlush) await options.onTemplateFlush(templateFlushCalls, events);
    if (options.templateFlushError || options.templateFlushErrorOnCall === templateFlushCalls) throw new Error("template save failed");
  }
  const hasPendingProjectSaves = () => options.isProjectPending?.() ?? false;
  const hasPendingTemplateSaves = () => options.isTemplatePending?.() ?? false;
  const imports = {
    "@tauri-apps/api/app": {
      async getVersion() { events.push("version"); return options.version ?? "2.0.0"; },
    },
    "@tauri-apps/api/window": {
      getCurrentWindow() { return { async close() { events.push("close"); if (options.closeError) throw new Error("close failed"); } }; },
    },
    "@tauri-apps/api/core": {
      async invoke(command, args) {
        events.push({ command, args: clone(args ?? {}) });
        if (command === "get_update_platform") return options.environment ?? windows;
        if (command === "download_release_asset") {
          if (options.downloadError) throw new Error("download failed");
          if (options.downloadGate) await options.downloadGate;
          return "C:/temporary/jokerforge-update/Joker.Forge_x64.exe";
        }
        if (command === "install_update_and_restart") {
          if (options.installError) throw new Error("install failed");
          if (options.onHandoff) await options.onHandoff(events);
          return;
        }
        if (command === "cancel_update_install") {
          if (options.cancelError) throw new Error("cancel failed");
          return;
        }
        if (command === "discard_update_download") return;
        throw new Error(`Unexpected invoke: ${command}`);
      },
    },
    "@tauri-apps/plugin-opener": {
      async openUrl(url) { events.push({ openUrl: url }); if (options.openError) throw new Error("browser failed"); },
    },
    "@/generated/release-channel": { RELEASE_CHANNEL: options.channel ?? "stable" },
    "@/lib/services/release-update-policy": policy(),
    "./release-update-policy": policy(),
    "@/lib/services/storage": {
      flushPendingSaves: flushPendingProjectSaves,
      flushPendingProjectSaves,
      hasPendingProjectSaves,
    },
    "./storage": {
      flushPendingSaves: flushPendingProjectSaves,
      flushPendingProjectSaves,
      hasPendingProjectSaves,
    },
    "@/lib/content/templates": { flushPendingTemplateSaves, hasPendingTemplateSaves },
    "../content/templates": { flushPendingTemplateSaves, hasPendingTemplateSaves },
    semver: require("semver"),
  };
  const service = loadTypeScript("src/lib/services/release-updater.ts", imports, {
    navigator: { userAgent: "Windows NT 10.0; Win64; x64" },
    window: { alert(message) { events.push({ alert: message }); } },
    async fetch(url, request) {
      events.push({ fetch: url, request: clone(request ?? {}) });
      if (options.fetchError) throw new Error("network failed");
      if (options.fetchGate) await options.fetchGate;
      return { ok: options.httpStatus == null || options.httpStatus === 200, status: options.httpStatus ?? 200, async json() { return Object.hasOwn(options, "releases") ? options.releases : [release()]; } };
    },
  }, options.env);
  service.onUpdateAvailable((info) => notified.push(info));
  return { service, events, notified };
}

function updateInfo(overrides = {}) {
  return {
    currentVersion: "2.0.0",
    latestVersion: "2.1.0",
    channel: "stable",
    asset: asset("Joker.Forge_2.1.0_x64-setup.exe"),
    releaseUrl: `${REPO}/releases/tag/v2.1.0`,
    installation: "automatic",
    ...overrides,
  };
}

test("launch check chooses a newer eligible stable version and runs only once", async () => {
  const { service, events, notified } = createService({ releases: [release("v99.0.0-beta.1"), release("v2.0.1"), release("v2.1.0")] });
  await service.checkForReleaseUpdateOnLaunch();
  await service.checkForReleaseUpdateOnLaunch();
  assert.equal(notified.length, 1);
  assert.equal(notified[0].latestVersion, "2.1.0");
  assert.equal(notified[0].installation, "automatic");
  assert.equal(events.filter((item) => item.fetch).length, 1);
  assert.equal(events.some((item) => item.command === "download_release_asset"), false);
  assert.equal(events.includes("close"), false);
});

test("launch check uses backend platform data and offers Linux and macOS manual updates", async () => {
  for (const platform of ["linux", "macos"]) {
    const { service, events, notified } = createService({ environment: { os: platform, arch: "aarch64", automaticInstall: false } });
    await service.checkForReleaseUpdateOnLaunch();
    assert.equal(notified.length, 1, platform);
    assert.equal(notified[0].installation, "manual", platform);
    assert.equal(notified[0].asset, null, platform);
    assert.equal(notified[0].releaseUrl, `${REPO}/releases/tag/v2.1.0`);
    assert.equal(events.some((item) => item.command === "get_update_platform"), true);
  }
});

test("Windows offers a manual release page when its architecture has no compatible installer", async () => {
  const { service, notified } = createService({
    environment: { os: "windows", arch: "aarch64", automaticInstall: true },
    releases: [release("v2.1.0", { assets: [asset("Joker.Forge_2.1.0_x64-setup.exe")] })],
  });
  await service.checkForReleaseUpdateOnLaunch();
  assert.equal(notified.length, 1);
  assert.equal(notified[0].installation, "manual");
  assert.equal(notified[0].asset, null);
});

test("equal, older, invalid and ineligible releases never notify", async () => {
  for (const releases of [[release("v2.0.0")], [release("v1.9.0")], [release("invalid")], [release("v3.0.0-beta")], [release("v3.0.0", { prerelease: true })], []]) {
    const { service, notified } = createService({ releases });
    await service.checkForReleaseUpdateOnLaunch();
    assert.equal(notified.length, 0, JSON.stringify(releases));
  }
});

test("nightly launch check uses the latest nightly and development overrides are deliberate", async () => {
  const nightly = release("nightly-2.0.0-beta-nightly.20261003.9", { prerelease: true });
  const production = createService({ version: "2.0.0-beta-nightly.local", releases: [release("v99.0.0"), nightly] });
  await production.service.checkForReleaseUpdateOnLaunch();
  assert.equal(production.notified[0]?.latestVersion, "2.0.0-beta-nightly.20261003.9");
  assert.equal(production.notified[0]?.channel, "nightly");
  const dev = createService({ env: { DEV: true } });
  await dev.service.checkForReleaseUpdateOnLaunch();
  assert.equal(dev.events.length, 0);
  const override = createService({ releases: [nightly], env: { DEV: true, VITE_ENABLE_UPDATE_CHECK_IN_DEV: "true", VITE_UPDATE_TEST_CHANNEL: "nightly", VITE_UPDATE_TEST_CURRENT_VERSION: "2.0.0-beta-nightly.local" } });
  await override.service.checkForReleaseUpdateOnLaunch();
  assert.equal(override.notified[0]?.channel, "nightly");
});

test("network, API and malformed payload failures preserve the app and do not notify", async () => {
  for (const options of [{ fetchError: true }, { httpStatus: 403 }, { releases: {} }, { releases: null }]) {
    const { service, events, notified } = createService(options);
    await service.checkForReleaseUpdateOnLaunch();
    assert.equal(notified.length, 0);
    assert.equal(events.includes("close"), false);
    assert.equal(events.some((item) => item.command === "download_release_asset"), false);
  }
});

test("manual updating opens the official release page without saving, downloading or closing", async () => {
  const { service, events } = createService();
  await service.performUpdate(updateInfo({ asset: null, installation: "manual" }));
  assert.deepEqual(events.filter((item) => item.openUrl), [{ openUrl: `${REPO}/releases/tag/v2.1.0` }]);
  assert.equal(events.some((item) => item.command === "download_release_asset" || item.command === "install_update_and_restart"), false);
  assert.equal(events.includes("flush"), false);
  assert.equal(events.includes("template-flush"), false);
  assert.equal(events.includes("close"), false);
});

test("Windows automatic update flushes project saves before installer handoff and closes only afterwards", async () => {
  const { service, events } = createService();
  await service.performUpdate(updateInfo());
  const commands = events.filter((item) => item.command).map((item) => item.command);
  assert.ok(commands.includes("download_release_asset"));
  assert.ok(commands.includes("install_update_and_restart"));
  const installIndex = events.findIndex((item) => item.command === "install_update_and_restart");
  const flushIndices = events.flatMap((item, index) => item === "flush" ? [index] : []);
  assert.equal(flushIndices.length, 2);
  assert.ok(flushIndices[0] < installIndex);
  assert.ok(installIndex < flushIndices[1]);
  assert.ok(flushIndices[1] < events.indexOf("close"));
  const download = events.find((item) => item.command === "download_release_asset");
  assert.equal(download.args.url, updateInfo().asset.browser_download_url);
  assert.equal(download.args.fileName, updateInfo().asset.name);
  assert.equal(download.args.expectedSize, null);
  assert.equal(download.args.expectedDigest, null);
});

test("automatic updating forwards release asset size and digest for backend validation", async () => {
  const { service, events } = createService();
  const verified = { ...asset("Joker.Forge_2.1.0_x64-setup.exe"), size: 123456, digest: `sha256:${"a".repeat(64)}` };
  await service.performUpdate(updateInfo({ asset: verified }));
  const download = events.find((item) => item.command === "download_release_asset");
  assert.equal(download.args.expectedSize, verified.size);
  assert.equal(download.args.expectedDigest, verified.digest);
});

test("save, download and installer failures never close the app and can be retried", async () => {
  for (const failure of ["flushError", "downloadError", "installError"]) {
    const options = { [failure]: true };
    const { service, events } = createService(options);
    await assert.rejects(() => service.performUpdate(updateInfo()));
    assert.equal(events.includes("close"), false, failure);
    if (failure === "flushError") assert.equal(events.some((item) => item.command === "install_update_and_restart"), false);
    if (failure !== "downloadError") assert.equal(events.some((item) => item.command === "discard_update_download"), true, failure);
    options[failure] = false;
    await service.performUpdate(updateInfo());
    assert.equal(events.includes("close"), true, `${failure} retry`);
  }
});

test("manual browser failure is retryable and a failed close cancels the waiting installer", async () => {
  const options = { openError: true };
  const manual = createService(options);
  await assert.rejects(() => manual.service.performUpdate(updateInfo({ installation: "manual", asset: null })));
  options.openError = false;
  await manual.service.performUpdate(updateInfo({ installation: "manual", asset: null }));
  assert.equal(manual.events.includes("close"), false);
  const automatic = createService({ closeError: true });
  await assert.rejects(() => automatic.service.performUpdate(updateInfo()));
  assert.equal(automatic.events.some((item) => item.command === "install_update_and_restart"), true);
  const cancel = automatic.events.find((item) => item.command === "cancel_update_install");
  assert.ok(cancel);
  assert.equal(cancel.args.installerPath, "C:/temporary/jokerforge-update/Joker.Forge_x64.exe");
  assert.ok(automatic.events.indexOf("close") < automatic.events.indexOf(cancel));
});

test("performUpdate rejects untrusted release and installer URLs before any action", async () => {
  for (const info of [
    updateInfo({ installation: "manual", asset: null, releaseUrl: "https://evil.example/release" }),
    updateInfo({ asset: { name: "Joker.Forge_x64.exe", browser_download_url: "https://evil.example/setup.exe" } }),
  ]) {
    const { service, events } = createService();
    await assert.rejects(() => service.performUpdate(info));
    assert.equal(events.some((item) => item.command === "download_release_asset" || item.command === "install_update_and_restart" || item.openUrl), false);
    assert.equal(events.includes("close"), false);
  }
});

test("unsubscribed update listeners are no longer notified", async () => {
  const { service } = createService();
  let calls = 0;
  const unsubscribe = service.onUpdateAvailable(() => { calls++; });
  unsubscribe();
  await service.checkForReleaseUpdateOnLaunch();
  assert.equal(calls, 0);
});

test("failed launch checks can retry after a transient network failure", async () => {
  const options = { fetchError: true };
  const { service, events, notified } = createService(options);
  await service.checkForReleaseUpdateOnLaunch();
  assert.equal(notified.length, 0);
  options.fetchError = false;
  await service.checkForReleaseUpdateOnLaunch();
  assert.equal(notified.length, 1);
  assert.equal(events.filter((item) => item.fetch).length, 2);
});

test("simultaneous launch checks share one request and one notification", async () => {
  let finishFetch;
  const fetchGate = new Promise((resolve) => { finishFetch = resolve; });
  const { service, events, notified } = createService({ fetchGate });
  const first = service.checkForReleaseUpdateOnLaunch();
  const second = service.checkForReleaseUpdateOnLaunch();
  finishFetch();
  await Promise.all([first, second]);
  assert.equal(events.filter((item) => item.fetch).length, 1);
  assert.equal(notified.length, 1);
});

test("a listener mounted after the launch check receives the available update", async () => {
  const { service } = createService();
  await service.checkForReleaseUpdateOnLaunch();
  const updates = [];
  service.onUpdateAvailable((info) => updates.push(info));
  assert.equal(updates.length, 1);
  assert.equal(updates[0].latestVersion, "2.1.0");
});

test("simultaneous update clicks share one download, handoff and close", async () => {
  let finishDownload;
  const downloadGate = new Promise((resolve) => { finishDownload = resolve; });
  const { service, events } = createService({ downloadGate });
  const first = service.performUpdate(updateInfo());
  const second = service.performUpdate(updateInfo());
  finishDownload();
  await Promise.all([first, second]);
  assert.equal(events.filter((item) => item.command === "download_release_asset").length, 1);
  assert.equal(events.filter((item) => item.command === "install_update_and_restart").length, 1);
  assert.equal(events.filter((item) => item === "flush").length, 2);
  assert.equal(events.filter((item) => item === "close").length, 1);
});

test("a release and its installer must match their stated release tag", () => {
  const { selectLatestRelease, selectInstallerAsset } = policy();
  assert.equal(selectLatestRelease([release("v2.1.0", { html_url: `${REPO}/releases/tag/v99.0.0` })], "stable"), null);
  assert.equal(selectInstallerAsset(release("v2.1.0", { assets: [asset("Joker.Forge_2.1.0_x64-setup.exe", "v99.0.0")] }), windows), null);
});

test("malformed release and asset entries are skipped rather than blocking valid updates", () => {
  const { selectLatestRelease, selectInstallerAsset } = policy();
  const candidates = [null, {}, { tag_name: "v99.0.0", assets: null }, release("v2.1.0")];
  assert.equal(selectLatestRelease(candidates, "stable")?.tag_name, "v2.1.0");
  const malformedAssets = [null, {}, { name: 1 }, { name: "Joker.Forge_x64-setup.exe", browser_download_url: null }, asset("Joker.Forge_x64-setup.exe")];
  assert.equal(selectInstallerAsset(release("v2.1.0", { assets: malformedAssets }), windows)?.name, "Joker.Forge_x64-setup.exe");
});

test("32-bit Windows never selects an x86_64 installer", () => {
  const { selectInstallerAsset } = policy();
  const candidate = release("v2.1.0", { assets: [asset("Joker.Forge_2.1.0_x86_64-setup.exe")] });
  assert.equal(selectInstallerAsset(candidate, { os: "windows", arch: "x86", automaticInstall: true }), null);
  assert.equal(selectInstallerAsset(candidate, { os: "windows", arch: "i686", automaticInstall: true }), null);
});

test("edits made during installer handoff finish saving before the app closes", { timeout: 2_000 }, async () => {
  let hasLateEdit = false;
  let lateEditSaved = false;
  let enteredSecondFlush;
  const secondFlushStarted = new Promise((resolve) => { enteredSecondFlush = resolve; });
  let finishLateSave;
  const lateSaveGate = new Promise((resolve) => { finishLateSave = resolve; });
  const { service, events } = createService({
    onHandoff(handoffEvents) { hasLateEdit = true; handoffEvents.push("late-edit"); },
    async onFlush(call) {
      if (call === 2) {
        assert.equal(hasLateEdit, true);
        enteredSecondFlush();
        await lateSaveGate;
        lateEditSaved = true;
        hasLateEdit = false;
      }
    },
  });
  const update = service.performUpdate(updateInfo());
  await secondFlushStarted;
  assert.equal(events.includes("close"), false);
  assert.equal(lateEditSaved, false);
  finishLateSave();
  await update;
  assert.equal(lateEditSaved, true);
  assert.equal(hasLateEdit, false);
  assert.ok(events.indexOf("late-edit") < events.lastIndexOf("flush"));
  assert.ok(events.lastIndexOf("flush") < events.indexOf("close"));
});

test("a failed save after handoff cancels the helper and preserves the open app for retry", async () => {
  const options = { flushErrorOnCall: 2 };
  const { service, events } = createService(options);
  await assert.rejects(() => service.performUpdate(updateInfo()), /save failed/);
  const installIndex = events.findIndex((item) => item.command === "install_update_and_restart");
  const cancelIndex = events.findIndex((item) => item.command === "cancel_update_install");
  assert.equal(events.filter((item) => item === "flush").length, 2);
  assert.ok(installIndex < events.lastIndexOf("flush"));
  assert.ok(events.lastIndexOf("flush") < cancelIndex);
  assert.equal(events[cancelIndex].args.installerPath, "C:/temporary/jokerforge-update/Joker.Forge_x64.exe");
  const discardIndex = events.findIndex((item) => item.command === "discard_update_download");
  assert.ok(cancelIndex < discardIndex);
  assert.equal(events.includes("close"), false);
  options.flushErrorOnCall = null;
  await service.performUpdate(updateInfo());
  assert.equal(events.filter((item) => item === "close").length, 1);
});

test("save coordination includes project edits queued while templates finish saving", async () => {
  let pendingProjectSave = false;
  let completedProjectFlushes = 0;
  const { service, events } = createService({
    onFlush() {
      completedProjectFlushes++;
      pendingProjectSave = false;
    },
    onTemplateFlush(call, templateEvents) {
      if (call === 1) {
        pendingProjectSave = true;
        templateEvents.push("project-edit-during-template-save");
      }
    },
    isProjectPending: () => pendingProjectSave,
    onHandoff() {
      assert.equal(pendingProjectSave, false);
      assert.equal(completedProjectFlushes, 2);
    },
  });
  await service.performUpdate(updateInfo());
  const lateEditIndex = events.indexOf("project-edit-during-template-save");
  const installIndex = events.findIndex((item) => item.command === "install_update_and_restart");
  const repeatFlushIndex = events.findIndex((item, index) => index > lateEditIndex && item === "flush");
  assert.ok(lateEditIndex < repeatFlushIndex);
  assert.ok(repeatFlushIndex < installIndex);
  assert.equal(events.filter((item) => item === "flush").length, 3);
  assert.equal(events.filter((item) => item === "template-flush").length, 3);
});

test("save coordination includes template edits queued after their initial flush", { timeout: 2_000 }, async () => {
  let pendingTemplateSave = false;
  let completedTemplateFlushes = 0;
  let firstTemplateFlushed;
  const templateSnapshotSaved = new Promise((resolve) => { firstTemplateFlushed = resolve; });
  const { service, events } = createService({
    async onFlush(call) {
      if (call === 1) {
        await templateSnapshotSaved;
        pendingTemplateSave = true;
      }
    },
    onTemplateFlush(call) {
      completedTemplateFlushes++;
      pendingTemplateSave = false;
      if (call === 1) firstTemplateFlushed();
    },
    isTemplatePending: () => pendingTemplateSave,
    onHandoff() {
      assert.equal(pendingTemplateSave, false);
      assert.equal(completedTemplateFlushes, 2);
    },
  });
  await service.performUpdate(updateInfo());
  assert.equal(events.filter((item) => item === "flush").length, 3);
  assert.equal(events.filter((item) => item === "template-flush").length, 3);
});

test("a failed template save prevents handoff and closure, removes the download and can retry", async () => {
  const options = { templateFlushError: true };
  const { service, events } = createService(options);
  await assert.rejects(() => service.performUpdate(updateInfo()), /template save failed/);
  assert.equal(events.some((item) => item.command === "install_update_and_restart"), false);
  assert.equal(events.some((item) => item.command === "discard_update_download"), true);
  assert.equal(events.includes("close"), false);
  options.templateFlushError = false;
  await service.performUpdate(updateInfo());
  assert.equal(events.filter((item) => item === "close").length, 1);
});

test("a failed template save after handoff cancels the helper and leaves the app open", async () => {
  const { service, events } = createService({ templateFlushErrorOnCall: 2 });
  await assert.rejects(() => service.performUpdate(updateInfo()), /template save failed/);
  const installIndex = events.findIndex((item) => item.command === "install_update_and_restart");
  const cancelIndex = events.findIndex((item) => item.command === "cancel_update_install");
  assert.ok(installIndex < events.lastIndexOf("template-flush"));
  assert.ok(events.lastIndexOf("template-flush") < cancelIndex);
  assert.equal(events.includes("close"), false);
});

test("cancellation errors preserve the original save failure and still attempt download cleanup", async () => {
  const { service, events } = createService({ flushErrorOnCall: 2, cancelError: true });
  await assert.rejects(() => service.performUpdate(updateInfo()), (error) => {
    assert.match(error.message, /save failed/);
    assert.match(error.message, /cancel failed/);
    return true;
  });
  const cancelIndex = events.findIndex((item) => item.command === "cancel_update_install");
  const discardIndex = events.findIndex((item) => item.command === "discard_update_download");
  assert.ok(cancelIndex < discardIndex);
  assert.equal(events.includes("close"), false);
});
