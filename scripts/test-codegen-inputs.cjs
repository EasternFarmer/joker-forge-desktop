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
    compilerOptions: {
      module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
      jsx: ts.JsxEmit.ReactJSX,
    },
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
const descriptionRegistry = loadTypeScript("src/lib/rules/description-variable-registry.ts", {
  "@/lib/content/game-vars": gameVariables,
});
const { buildDescriptionVariableTokens } = descriptionRegistry;
const aliases = [
  "hand_size", "remaining_hands", "remaining_discards", "deck_size", "full_deck_size",
  "player_money", "dollars", "ante_level", "blind_chips", "blind_mult", "consumable_count",
  "interest", "hand_level", "times_hand_played", "scored_card_count", "played_card_count", "poker_hand_count",
];

function editionSaveHandlers(shader = "foil") {
  const edition = {
    id: "edition", objectType: "edition", name: "Test Edition",
    objectKey: "test", description: "Custom description", orderValue: 1,
    shader, rules: [],
  };
  const data = { editions: [edition], metadata: { prefix: "test" } };
  const hookStates = [null, edition, null, false];
  let hookIndex = 0;
  const { applyItemUpdatesWithOrderSwap } = loadTypeScript("src/lib/items/item-order.ts");
  const componentImports = {
    "@/components/pages/generic-item-page": { GenericItemPage: "ItemPage" },
    "@/components/pages/generic-item-card": {},
    "@/components/pages/generic-item-card-compact": {},
    "@/components/ui/confirm-dialog": {},
    "@/components/balatro/balatro-card": {},
    "@/components/rule-builder": { RuleBuilder: "RuleBuilder" },
    "@/components/pages/item-showcase-dialog": {},
    "@/components/templates/template-picker-dialog": {},
    "@/components/edit-dialogs": { EditEditionDialog: "InfoDialog" },
  };
  const { default: EditionsPage } = loadTypeScript("src/pages/editions-page.tsx", {
    ...componentImports,
    react: {
      useCallback: (callback) => callback,
      useMemo: (callback) => callback(),
      useEffect() {},
      useRef: (current) => ({ current }),
      useState() {
        const index = hookIndex++;
        return [hookStates[index], (value) => {
          hookStates[index] = typeof value === "function" ? value(hookStates[index]) : value;
        }];
      },
    },
    "react/jsx-runtime": {
      jsx: (type, props) => ({ type, props }),
      jsxs: (type, props) => ({ type, props }),
      Fragment: "Fragment",
    },
    "react-router-dom": { useSearchParams: () => [new URLSearchParams(), () => {}] },
    "@/lib/services/storage": {
      useProjectData: () => ({
        data, isHydrating: false,
        updateEditions: (updates) => {
          data.editions = typeof updates === "function" ? updates(data.editions) : updates;
        },
      }),
      useModName: () => "Test Mod",
    },
    "@/hooks/use-confirm-delete": { useConfirmDelete: () => ({}) },
    "@/lib/core/search": {},
    "@phosphor-icons/react": {},
    "@/lib/export/rust-codegen-export": {},
    "@/lib/app/global-user-variables": {},
    "@/lib/rules/auto-description": {
      shouldOverwriteDescriptionOnRuleSave: () => false,
    },
    "@/lib/items/item-order": { applyItemUpdatesWithOrderSwap },
    "@/lib/content/templates": {
      useTemplateStore: () => ({ getItemTemplatesForType: () => [] }),
    },
    "@/lib/app/global-alerts-bus": {},
    "@/lib/description/description-loc-vars": { getItemLocVarsFromUserVariables: () => [] },
  });
  const children = EditionsPage().props.children;
  return {
    data,
    rules: children.find((child) => child?.type === "RuleBuilder").props,
    info: children.find((child) => child?.type === "InfoDialog").props,
    page: children.find((child) => child?.type === "ItemPage").props,
  };
}

test("saving edition rules and other partial edits retains its selected shader", () => {
  for (const shader of ["foil", "holo", "custom_shader", false]) {
    const { data, rules, info, page } = editionSaveHandlers(shader);
    const newRules = [{ id: "score", trigger: "card_scored", effects: [] }];
    rules.onSave(newRules);
    assert.equal(data.editions[0].shader, shader);
    assert.equal(data.editions[0].rules, newRules);
    assert.equal(data.editions[0].description, "Custom description");
    rules.onUpdateItem({ userVariables: [{ id: "bonus", name: "bonus", type: "number", initialValue: 5 }] });
    rules.onUpdateItem({ customCode: { calculate: "return nil" } });
    info.onSave("edition", { name: "Renamed Edition" });
    page.renderCard(data.editions[0], { selectedAce: "Spades" }).props.onUpdate({ unlocked: false });
    assert.equal(data.editions[0].shader, shader);
    assert.equal(data.editions[0].name, "Renamed Edition");
    assert.equal(data.editions[0].unlocked, false);
  }
});

test("explicit edition shader changes still replace or clear the shader", () => {
  const { data, info, rules } = editionSaveHandlers();
  info.onSave("edition", { shader: "holo" });
  assert.equal(data.editions[0].shader, "holo");
  info.onSave("edition", { shader: "" });
  assert.equal(data.editions[0].shader, false);
  rules.onSave([{ id: "score", effects: [] }]);
  assert.equal(data.editions[0].shader, false);
  info.onSave("edition", { shader: "custom_shader" });
  assert.equal(data.editions[0].shader, "custom_shader");
  info.onSave("edition", { shader: false });
  assert.equal(data.editions[0].shader, false);
});

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

test("description bindings include game variables used by conditions, chance groups and loops", () => {
  const game = (id, multiplier = 1, startsFrom = 0) => ({
    value: `GAMEVAR:${id}|${multiplier}|${startsFrom}`, valueType: "game_var",
  });
  const tokens = buildDescriptionVariableTokens({ objectType: "joker", rules: [{
    conditionGroups: [{ conditions: [{ params: { value: game("cards_in_deck") } }] }],
    effects: [{ id: "chips", type: "add_chips", params: { value: game("joker_count", 2, 3) } }],
    randomGroups: [{ id: "chance", chance_numerator: game("hands_remaining"),
      chance_denominator: { value: 2 }, effects: [] }],
    loops: [{ repetitions: game("joker_count", 2, 3), effects: [{
      id: "money", type: "set_dollars", params: { value: game("current_money", -1, 5) },
    }] }],
  }] });
  const bindings = tokens.filter((token) => token.category === "game").map((token) => token.binding);
  assert.deepEqual(JSON.parse(JSON.stringify(bindings)), [
    { kind: "game", id: "cards_in_deck" },
    { kind: "game", id: "joker_count" },
    { kind: "game", id: "hands_remaining" },
    { kind: "game", id: "current_money" },
    { kind: "game", id: "joker_count", multiplier: 2, startsFrom: 3 },
    { kind: "game", id: "current_money", multiplier: -1, startsFrom: 5 },
  ]);
  assert.equal(tokens[0].binding.fallback.value, "GAMEVAR:joker_count|2|3");
  assert.ok(tokens.some((token) => token.label === "3 + (2 × Joker Count)"));
});

test("description game bindings recognize both typed aliases and raw parameter strings", () => {
  const tokens = buildDescriptionVariableTokens({ rules: [{ effects: [{ params: {
    first: { value: "joker_count", valueType: "game_var" },
    second: { value: "cards_in_deck", valueType: "gameVariable" },
    raw: "GAMEVAR:cards_in_deck|0|4",
    duplicate: { value: "GAMEVAR:cards_in_deck|0.0|4e0", valueType: "game_var" },
    ordinary: { value: "cards_in_hand", valueType: "variable" },
  } }] }] });
  assert.deepEqual(JSON.parse(JSON.stringify(tokens.map((token) => token.binding))), [
    { kind: "game", id: "cards_in_deck" },
    { kind: "game", id: "joker_count" },
    { kind: "game", id: "cards_in_deck", multiplier: 0, startsFrom: 4 },
  ]);
});

test("newly recognized typed aliases append after existing description game slots", () => {
  const tokens = buildDescriptionVariableTokens({ rules: [{ effects: [{ params: {
    newVariable: { value: "hands_remaining", valueType: "game_var" },
    newRawVariable: "GAMEVAR:cards_in_hand|1|0",
    existingVariableAlias: { value: "joker_count", valueType: "game_var" },
    existingEncoded: { value: "GAMEVAR:cards_in_deck|2|3", valueType: "game_var" },
    existingTyped: { value: "current_money", valueType: "gameVariable" },
    existingEncodedLater: { value: "GAMEVAR:joker_count|1|0", valueType: "game_var" },
  } }] }] });
  assert.deepEqual(JSON.parse(JSON.stringify(tokens.map((token) => token.binding))), [
    { kind: "game", id: "cards_in_deck" },
    { kind: "game", id: "current_money" },
    { kind: "game", id: "joker_count" },
    { kind: "game", id: "hands_remaining" },
    { kind: "game", id: "cards_in_hand" },
    { kind: "game", id: "cards_in_deck", multiplier: 2, startsFrom: 3 },
  ]);
});

function renderDescriptionEditor(item, search = "") {
  const element = (type, props) => ({ type, props });
  const imports = {
    react: {
      memo: (component) => component, useCallback: (callback) => callback,
      useEffect() {}, useMemo: (factory) => factory(), useRef: () => ({ current: null }),
      useState: (initial) => [initial === "" ? search : initial, () => {}],
    },
    "react/jsx-runtime": { jsx: element, jsxs: element },
    "@/lib/core/utils": { cn: () => "" },
    "@/lib/balatro/balatro-text-formatter": {},
    "@/lib/rules/description-variable-registry": descriptionRegistry,
    "@/lib/rules/auto-description": { generateDescriptionFromRules: () => "" },
    "@/lib/core/search": loadTypeScript("src/lib/core/search.ts"),
    "@phosphor-icons/react": {},
  };
  for (const component of ["button", "input", "scroll-area", "textarea", "separator", "tooltip"]) {
    imports[`@/components/ui/${component}`] = {};
  }
  const { DescriptionEditor } = loadTypeScript("src/components/pages/description-editor.tsx", imports);
  return DescriptionEditor({ value: "", onChange() {}, item });
}

function elementText(node) {
  if (Array.isArray(node)) return node.map(elementText).join("");
  if (node && typeof node === "object") return elementText(node.props?.children);
  return typeof node === "string" || typeof node === "number" ? String(node) : "";
}

function elementButtons(node) {
  if (Array.isArray(node)) return node.flatMap(elementButtons);
  if (!node || typeof node !== "object") return [];
  return [
    ...(node.type === "button" ? [node] : []),
    ...elementButtons(node.props?.children),
  ];
}

test("description editor exposes game variables with their exported placeholder numbers", () => {
  const item = { objectType: "joker", userVariables: [{ name: "bonus", initialValue: 2 }],
    rules: [{ effects: [{ id: "chips", type: "add_chips", params: {
      value: { value: "GAMEVAR:joker_count|2|3", valueType: "game_var" },
    } }] }] };
  for (const search of ["", "joker_count"]) {
    const tree = renderDescriptionEditor(item, search);
    const labels = elementButtons(tree).map(elementText);
    assert.ok(elementText(tree).includes("Game Variables"));
    assert.ok(labels.some((label) => label.startsWith("Joker Count#3#")), search);
    assert.ok(labels.some((label) => label.startsWith("3 + (2 × Joker Count)#4#")), search);
    assert.ok(!labels.some((label) => label.startsWith("chips0")), "Internal config slots stay hidden");
  }
});

test("explicit localization slots retain their order when rules use game variables", () => {
  const tokens = buildDescriptionVariableTokens({ locVars: { vars: [7, 7, "value"] },
    rules: [{ effects: [{ params: { value: { value: "GAMEVAR:joker_count|2|3" } } }] }] });
  assert.deepEqual(JSON.parse(JSON.stringify(tokens.map((token) => token.binding))), [
    { kind: "literal", value: 7 }, { kind: "literal", value: 7 }, { kind: "literal", value: "value" },
  ]);
});

test("description game bindings validate finite starting values and multipliers", () => {
  for (const binding of [
    { kind: "game", id: "joker_count" },
    { kind: "game", id: "joker_count", multiplier: -1.5, startsFrom: 2 },
    { kind: "game", id: "joker_count", multiplier: 0, starts_from: 0 },
  ]) {
    assert.equal(runPreExportChecks(project("jokers", { descriptionVariables: [binding] })).length, 0);
  }
  for (const field of ["multiplier", "startsFrom", "starts_from"]) {
    for (const value of [NaN, Infinity, -Infinity, "2", null]) {
      const issues = runPreExportChecks(project("jokers", { descriptionVariables: [{
        kind: "game", id: "joker_count", [field]: value,
      }] }));
      assert.equal(issues.length, 1, `${field} ${value}`);
      assert.match(issues[0].message, /starting value or multiplier is invalid/);
    }
  }
});
