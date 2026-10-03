/* Run with `node --test scripts/test-template-storage.cjs`. No real app data is used. */
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { webcrypto } = require("node:crypto");
const { test } = require("node:test");
const ts = require("typescript");

const INDEX = "/appdata/joker_forge_storage/templates.json";
const FOLDERS = "/appdata/joker_forge_storage/templates";
const settle = () => new Promise((resolve) => setImmediate(resolve));
const plain = (value) => JSON.parse(JSON.stringify(value));
const deferred = () => {
  let resolve;
  const promise = new Promise((yes) => { resolve = yes; });
  return { promise, resolve };
};
const library = (image) => ({
  version: 1,
  templates: [{
    id: "existing", name: "Saved Joker", kind: "item", itemType: "joker",
    createdAt: 1, updatedAt: 1,
    payload: { name: "Joker payload", ...(image ? { image } : {}) },
  }],
});

function templateModule({ desktop = true } = {}) {
  const entries = new Map();
  const preferences = new Map();
  const writes = [];
  const effects = [];
  const state = [];
  const controls = { beforeWrite: null, beforeExists: null, beforePreference: null };
  let stateIndex = 0;
  const adapter = {
    exists: async (file) => {
      if (controls.beforeExists) await controls.beforeExists(file);
      return entries.has(file);
    },
    mkdir: async (directory) => {
      for (let current = directory; current !== "/"; current = path.posix.dirname(current)) {
        entries.set(current, { directory: true });
      }
    },
    readDir: async (directory) => [...entries].flatMap(([file, entry]) =>
      path.posix.dirname(file) === directory
        ? [{ name: path.posix.basename(file), isDirectory: !!entry.directory }]
        : []),
    readTextFile: async (file) => {
      const entry = entries.get(file);
      if (!entry || typeof entry.text !== "string") throw new Error("Missing file");
      return entry.text;
    },
    readFile: async (file) => {
      const entry = entries.get(file);
      if (!entry?.bytes) throw new Error("Missing artwork");
      return entry.bytes;
    },
    remove: async (target, { recursive } = {}) => {
      for (const file of [...entries.keys()]) {
        if (file === target || (recursive && file.startsWith(`${target}/`))) entries.delete(file);
      }
    },
    writeTextFile: async (file, text) => {
      writes.push({ file, text });
      if (controls.beforeWrite) await controls.beforeWrite(file, text);
      entries.set(file, { text });
    },
    writeFile: async (file, bytes) => {
      writes.push({ file, bytes });
      if (controls.beforeWrite) await controls.beforeWrite(file, bytes);
      entries.set(file, { bytes: Uint8Array.from(bytes) });
    },
  };
  const imports = {
    react: {
      useState: (initial) => {
        const index = stateIndex++;
        if (!(index in state)) state[index] = typeof initial === "function" ? initial() : initial;
        return [state[index], (update) => {
          state[index] = typeof update === "function" ? update(state[index]) : update;
        }];
      },
      useCallback: (callback) => callback,
      useMemo: (factory) => factory(),
      useEffect: (effect) => effects.push(effect),
    },
    "@tauri-apps/api/path": {
      appDataDir: async () => "/appdata", dirname: async (file) => path.posix.dirname(file),
      join: async (...parts) => path.posix.join(...parts),
    },
    "@tauri-apps/plugin-fs": adapter,
    "@/lib/balatro/balatro-utils": {
      getConsumableSetByKey: () => null, getConsumableSetByValue: () => null,
      getRarityByKey: () => null, getRarityByValue: () => null,
    },
  };
  const filename = path.join(__dirname, "../src/lib/content/templates.ts");
  const source = fs.readFileSync(filename, "utf8") +
    "\nexport const testInternals = { persistStore, loadQueuedTemplateStore, getQueue: () => persistQueue };\n";
  const compiled = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
    fileName: filename,
  });
  const module = { exports: {} };
  vm.runInNewContext(compiled.outputText, {
    module, exports: module.exports,
    require: (name) => {
      if (Object.hasOwn(imports, name)) return imports[name];
      throw new Error(`Unmocked template import: ${name}`);
    },
    window: {
      ...(desktop ? { __TAURI_INTERNALS__: {} } : {}),
      localStorage: {
        getItem: (key) => preferences.get(key) ?? null,
        setItem: (key, value) => {
          if (controls.beforePreference) controls.beforePreference(key, value);
          preferences.set(key, value);
        },
      },
      addEventListener: () => {}, removeEventListener: () => {}, dispatchEvent: () => {},
    },
    CustomEvent: class CustomEvent { constructor(type, init) { this.type = type; this.detail = init.detail; } },
    crypto: webcrypto, Uint8Array, atob, btoa, queueMicrotask,
    console: { error: () => {} },
  }, { filename });
  const api = module.exports;
  return {
    api, controls, entries, writes,
    mount: () => {
      stateIndex = 0;
      const hook = api.useTemplateStore();
      for (const effect of effects.splice(0)) effect();
      return hook;
    },
    seed: async (data = library()) => {
      await api.testInternals.persistStore(data);
      writes.length = 0;
    },
    indexWrites: () => writes.filter((write) => write.file === INDEX),
    savedLibrary: () => JSON.parse(entries.get(INDEX).text),
    stateLibrary: () => state[0],
  };
}

test("template flush waits for every library file and artwork write to finish", async () => {
  const storage = templateModule();
  await storage.seed();
  const hook = storage.mount();
  await storage.api.flushPendingTemplateSaves();
  const artwork = deferred();
  storage.controls.beforeWrite = async (file) => {
    if (file.endsWith(".png")) await artwork.promise;
  };
  const image = `data:image/png;base64,${Buffer.from("new artwork").toString("base64")}`;
  hook.updateItemTemplate("existing", { name: "With Artwork", payload: { image } });
  let flushed = false;
  const flush = storage.api.flushPendingTemplateSaves().then(() => { flushed = true; });
  await settle();
  assert.equal(storage.api.hasPendingTemplateSaves(), true);
  assert.equal(flushed, false, "The JSON index alone cannot satisfy the flush");
  assert.equal(storage.indexWrites().length, 1);
  artwork.resolve();
  await flush;
  assert.equal(storage.api.hasPendingTemplateSaves(), false);
  const item = [...storage.entries].find(([file]) => file.endsWith("/With_Artwork/item.json"));
  assert.ok(item);
  assert.match(JSON.parse(item[1].text).payload.image, /^asset:\/\//);
  assert.ok([...storage.entries.keys()].some((file) => file.endsWith(".png")));
});

test("template flush includes edits queued while the earlier write is pending", async () => {
  const storage = templateModule();
  await storage.seed();
  const hook = storage.mount();
  await storage.api.flushPendingTemplateSaves();
  const writes = [deferred(), deferred()];
  let index = 0;
  storage.controls.beforeWrite = async (file) => {
    if (file === INDEX) await writes[index++].promise;
  };
  hook.updateTemplateName("existing", "First edit");
  let flushed = false;
  const flush = storage.api.flushPendingTemplateSaves().then(() => { flushed = true; });
  await settle();
  hook.updateTemplateName("existing", "Late edit");
  writes[0].resolve();
  await settle();
  assert.equal(storage.indexWrites().length, 2);
  assert.equal(flushed, false);
  writes[1].resolve();
  await flush;
  assert.equal(storage.savedLibrary().templates[0].name, "Late edit");
  assert.equal(storage.api.hasPendingTemplateSaves(), false);
});

test("failed template writes block updating and retry succeeds without a new edit", async () => {
  const storage = templateModule();
  await storage.seed();
  const hook = storage.mount();
  await storage.api.flushPendingTemplateSaves();
  storage.controls.beforeWrite = async (file) => {
    if (file === INDEX) throw new Error("Disk full");
  };
  hook.updateTemplateName("existing", "Unsaved edit");
  await storage.api.testInternals.getQueue();
  await assert.rejects(storage.api.flushPendingTemplateSaves(), /latest template changes could not be saved/);
  assert.equal(storage.indexWrites().length, 2, "A failed retry must not loop forever");
  assert.equal(storage.api.hasPendingTemplateSaves(), true);
  storage.controls.beforeWrite = null;
  await storage.api.flushPendingTemplateSaves();
  assert.equal(storage.indexWrites().length, 3);
  assert.equal(storage.savedLibrary().templates[0].name, "Unsaved edit");
  assert.equal(storage.api.hasPendingTemplateSaves(), false);
});

test("later successful template saves clear old failures and clean flushes avoid rewrites", async () => {
  const storage = templateModule();
  await storage.seed();
  const hook = storage.mount();
  await storage.api.flushPendingTemplateSaves();
  storage.controls.beforeWrite = async (file) => {
    if (file === INDEX) throw new Error("Locked");
  };
  hook.updateTemplateName("existing", "Failed edit");
  await storage.api.testInternals.getQueue();
  storage.controls.beforeWrite = null;
  hook.updateTemplateName("existing", "Committed edit");
  await storage.api.flushPendingTemplateSaves();
  await storage.api.flushPendingTemplateSaves();
  assert.equal(storage.indexWrites().length, 2);
  assert.equal(storage.savedLibrary().templates[0].name, "Committed edit");
  assert.equal(storage.api.hasPendingTemplateSaves(), false);
});

test("template flush waits for hydration migration writes", async () => {
  const storage = templateModule();
  storage.entries.set(INDEX, { text: JSON.stringify(library()) });
  const migration = deferred();
  storage.controls.beforeWrite = async (file) => {
    if (file === INDEX) await migration.promise;
  };
  storage.mount();
  let flushed = false;
  const flush = storage.api.flushPendingTemplateSaves().then(() => { flushed = true; });
  await settle();
  assert.equal(storage.api.hasPendingTemplateSaves(), true);
  assert.equal(flushed, false);
  migration.resolve();
  await flush;
  assert.equal(storage.api.hasPendingTemplateSaves(), false);
  assert.equal(storage.indexWrites().length, 1);
  assert.equal(storage.stateLibrary().templates[0].name, "Saved Joker");
  assert.ok(storage.entries.has(`${FOLDERS}/manifest.json`));
});

test("hydration queued during template flushing cannot overwrite a newer edit", async () => {
  const storage = templateModule();
  await storage.seed();
  const hook = storage.mount();
  await storage.api.flushPendingTemplateSaves();
  const write = deferred();
  storage.controls.beforeWrite = async (file) => {
    if (file === INDEX && storage.indexWrites().length === 1) await write.promise;
  };
  hook.updateTemplateName("existing", "First edit");
  const flush = storage.api.flushPendingTemplateSaves();
  await settle();
  const load = storage.api.testInternals.loadQueuedTemplateStore();
  hook.updateTemplateName("existing", "Later edit");
  write.resolve();
  assert.equal((await load).templates[0].name, "Later edit");
  await flush;
  assert.equal(storage.savedLibrary().templates[0].name, "Later edit");
  assert.equal(storage.api.hasPendingTemplateSaves(), false);
});

test("swallowed migration write failures also block flushing and retain the library for retry", async () => {
  const storage = templateModule();
  storage.entries.set(INDEX, { text: JSON.stringify(library()) });
  storage.controls.beforeWrite = async (file) => {
    if (file === INDEX) throw new Error("Disk full during migration");
  };
  storage.mount();
  await assert.rejects(storage.api.flushPendingTemplateSaves(), /latest template changes could not be saved/);
  assert.equal(storage.api.hasPendingTemplateSaves(), true);
  assert.equal(storage.indexWrites().length, 2);
  storage.controls.beforeWrite = null;
  await storage.api.flushPendingTemplateSaves();
  assert.equal(storage.savedLibrary().templates[0].name, "Saved Joker");
  assert.ok(storage.entries.has(`${FOLDERS}/manifest.json`));
  assert.equal(storage.api.hasPendingTemplateSaves(), false);
});

test("browser quota failures also block template flush and can be retried", async () => {
  const storage = templateModule({ desktop: false });
  await storage.seed();
  const hook = storage.mount();
  await storage.api.flushPendingTemplateSaves();
  storage.controls.beforePreference = () => { throw new Error("Quota exceeded"); };
  hook.updateTemplateName("existing", "Browser edit");
  await assert.rejects(storage.api.flushPendingTemplateSaves(), /latest template changes could not be saved/);
  assert.equal(storage.api.hasPendingTemplateSaves(), true);
  storage.controls.beforePreference = null;
  await storage.api.flushPendingTemplateSaves();
  assert.equal(storage.api.hasPendingTemplateSaves(), false);
  assert.equal(plain(storage.stateLibrary()).templates[0].name, "Browser edit");
});
