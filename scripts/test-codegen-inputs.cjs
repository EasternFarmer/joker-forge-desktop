/* Run with `node --test scripts/test-codegen-inputs.cjs`. */
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { test } = require("node:test");
const ts = require("typescript");

function loadTypeScript(relativePath, imports = {}) {
  const filePath = path.join(__dirname, "..", relativePath);
  const compiled = ts.transpileModule(fs.readFileSync(filePath, "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
    fileName: filePath,
  });
  const module = { exports: {} };
  vm.runInNewContext(compiled.outputText, {
    module,
    exports: module.exports,
    require(name) {
      if (Object.hasOwn(imports, name)) return imports[name];
      throw new Error(`Unmocked import in codegen input test: ${name}`);
    },
  }, { filename: filePath });
  return module.exports;
}

const gameVariables = loadTypeScript("src/lib/content/game-vars.ts", {
  "@phosphor-icons/react": {},
});
const { runPreExportChecks } = loadTypeScript("src/lib/export/pre-export-checks.ts", {
  "@/lib/content/game-vars": gameVariables,
});
const aliases = [
  "hand_size", "remaining_hands", "remaining_discards", "deck_size", "full_deck_size",
  "player_money", "dollars", "ante_level", "blind_chips", "blind_mult", "consumable_count",
  "interest", "hand_level", "times_hand_played", "scored_card_count", "played_card_count", "poker_hand_count",
];

function project(collection = "jokers", item = {}) {
  const data = {
    metadata: { id: "test", prefix: "test", name: "Test" },
    jokers: [], consumables: [], vouchers: [], decks: [], enhancements: [], seals: [],
    editions: [], boosters: [], sounds: [], rarities: [], consumableSets: [],
  };
  data[collection].push({
    id: "item", name: "Test Item", objectKey: "item", rules: [],
    ...item,
  });
  return data;
}

function parameterIssues(value) {
  return runPreExportChecks(project("jokers", {
    rules: [{ effects: [{ params: { value } }] }],
  }));
}

test("all selectable game variables and legacy aliases pass pre-export validation", () => {
  for (const id of [...gameVariables.getAllGameVariables().map((value) => value.id), ...aliases]) {
    for (const value of [
      `GAMEVAR:${id}|2|3`,
      { value: `GAMEVAR:${id}|-1.5|+2e-3`, valueType: "game_var" },
      { value: id, valueType: "gameVariable" },
    ]) {
      assert.equal(parameterIssues(value).length, 0, JSON.stringify(value));
    }
  }
});

test("unknown game variables block export and navigate to the owning rules editor", () => {
  for (const value of [
    "GAMEVAR:not_supported|1|0",
    { value: "not_supported", valueType: "game_var" },
    { value: "GAMEVAR:round_number|1|0", valueType: "number" },
  ]) {
    const issues = parameterIssues(value);
    assert.equal(issues.length, 1);
    assert.match(issues[0].message, /Unknown game variable/);
    assert.match(issues[0].message, /Test Item.*rule 1.*effect 1.*value/);
    assert.deepEqual(JSON.parse(JSON.stringify(issues[0].target)), {
      path: "/jokers", itemId: "item", editor: "rules",
    });
  }
});

test("malformed and nonfinite game variable settings block export", () => {
  for (const reference of [
    "GAMEVAR:cards_in_deck", "GAMEVAR:cards_in_deck|1", "GAMEVAR:cards_in_deck|1|0|extra",
    "GAMEVAR:|1|0", "GAMEVAR:cards_in_deck||0", "GAMEVAR:cards_in_deck|1|",
    "GAMEVAR:cards_in_deck|NaN|0", "GAMEVAR:cards_in_deck|inf|0",
    "GAMEVAR:cards_in_deck|1|Infinity", "GAMEVAR:cards_in_deck|1e999|0",
    "GAMEVAR:cards_in_deck|0x10|0", "GAMEVAR:cards_in_deck| 1|0",
    "GAMEVAR:cards_in_deck|1_000|0", "GAMEVAR:cards_in_deck|1|nope",
  ]) {
    assert.equal(parameterIssues(reference).length, 1, reference);
    assert.equal(parameterIssues({ value: reference, valueType: "game_var" }).length, 1, reference);
  }
  for (const value of [0, null, {}, [], undefined]) {
    assert.equal(parameterIssues({ value, valueType: "gameVariable" }).length, 1);
  }
});

test("finite decimal and exponent settings retain the compiler's accepted format", () => {
  for (const number of ["0", "-0", "+0", "1.", ".1", "-.5", "1e-30", "1E+30", "1e308"]) {
    assert.equal(parameterIssues(`GAMEVAR:cards_in_deck|${number}|${number}`).length, 0, number);
  }
});

test("nested condition, chance and loop parameters are validated independently", () => {
  const bad = { value: "GAMEVAR:missing|1|0", valueType: "game_var" };
  const issues = runPreExportChecks(project("jokers", { rules: [{
    conditionGroups: [{ conditions: [{ params: { value: bad } }] }],
    randomGroups: [{ chance_numerator: bad, chance_denominator: bad, effects: [{ params: { value: bad } }] }],
    loops: [{ repetitions: bad, effects: [{ params: { value: bad } }] }],
  }] }));
  assert.equal(issues.length, 6);
  assert.ok(issues.every((issue) => issue.target.editor === "rules"));
  assert.ok(issues.some((issue) => issue.message.includes("condition group 1 / condition 1")));
  assert.ok(issues.some((issue) => issue.message.includes("chance group 1 / chance")));
  assert.ok(issues.some((issue) => issue.message.includes("loop 1 / repetitions")));
});

test("raw value parameters do not hide invalid sibling references", () => {
  const issues = runPreExportChecks(project("jokers", { rules: [{ effects: [{ params: {
    value: "GAMEVAR:missing|1|0", amount: "GAMEVAR:also_missing|1|0",
  } }] }] }));
  assert.equal(issues.length, 2);
  assert.ok(issues.some((issue) => issue.message.includes('parameter "value"')));
  assert.ok(issues.some((issue) => issue.message.includes('parameter "amount"')));
});

test("imported snake-case rule groups and trigger parameters are validated", () => {
  const issues = runPreExportChecks(project("jokers", { rules: [{
    trigger_params: { amount: "GAMEVAR:missing|1|0" },
    condition_groups: [{ conditions: [{ params: { value: "GAMEVAR:missing|1|0" } }] }],
    random_groups: [{ chance_numerator: "GAMEVAR:missing|1|0" }],
    loop_groups: [{ count: "GAMEVAR:missing|1|0" }],
  }] }));
  assert.equal(issues.length, 4);
});

test("all rule-bearing item collections report invalid variables", () => {
  for (const collection of ["jokers", "consumables", "vouchers", "decks", "enhancements", "seals", "editions", "boosters"]) {
    const issues = runPreExportChecks(project(collection, {
      rules: [{ effects: [{ params: { value: "GAMEVAR:missing|1|0" } }] }],
    }));
    assert.equal(issues.length, 1, collection);
    assert.equal(issues[0].target.path, `/${collection}`);
    assert.equal(issues[0].target.itemId, "item");
  }
});

test("explicit description game bindings and fallback parameters use the allowlist", () => {
  const issues = runPreExportChecks(project("jokers", { descriptionVariables: [
    { kind: "game", id: "cards_in_deck" },
    { kind: "game", id: "missing" },
    { kind: "game", id: "GAMEVAR:cards_in_deck|1|0" },
    { kind: "config", fallback: { value: "missing", valueType: "game_var" } },
    { kind: "literal", value: "GAMEVAR:missing|1|0" },
  ] }));
  assert.equal(issues.length, 3);
  assert.ok(issues.every((issue) => issue.message.includes("description variable")));
});

test("ordinary literals, user variables and manually edited code remain editable", () => {
  for (const value of [12, "cards_in_deck", "missing", { value: "missing", valueType: "variable" }]) {
    assert.equal(parameterIssues(value).length, 0);
  }
  assert.equal(runPreExportChecks(project("jokers", {
    customCode: { fullCode: "GAMEVAR:missing|1|0" },
    locVars: { vars: ["GAMEVAR:missing|1|0"] },
    rules: [{ effects: [{ customMessage: "GAMEVAR:missing|1|0", params: {} }] }],
  })).length, 0);
});
