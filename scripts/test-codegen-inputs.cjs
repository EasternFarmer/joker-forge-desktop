/* Run with `node --test scripts/test-codegen-inputs.cjs`. */
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { test } = require("node:test");
const ts = require("typescript");

function loadTypeScript(relativePath, imports = {}, globals = {}) {
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
    ...globals,
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
    "@/lib/description/description-formatting": loadTypeScript("src/lib/description/description-formatting.ts"),
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

const descriptionFormatting = loadTypeScript("src/lib/description/description-formatting.ts");
const descriptionPreview = loadTypeScript("src/lib/balatro/balatro-text-formatter.tsx", {
  react: { default: { Fragment: "Fragment" } },
  "react/jsx-runtime": {
    jsx: (type, props) => ({ type, props }),
    jsxs: (type, props) => ({ type, props }),
  },
});

function allElements(node) {
  if (Array.isArray(node)) return node.flatMap(allElements);
  if (!node || typeof node !== "object") return [];
  return [node, ...allElements(node.props?.children)];
}

function descriptionToolbar(value, start = 0, end = value.length) {
  const changes = [];
  const cursorChanges = [];
  let autoFormatCalls = 0;
  const textarea = {
    value, selectionStart: start, selectionEnd: end,
    focus() {},
    setSelectionRange(nextStart, nextEnd) { cursorChanges.push([nextStart, nextEnd]); },
  };
  const element = (type, props) => ({ type, props });
  const imports = {
    react: {
      memo: (component) => component, useCallback: (callback) => callback,
      useEffect() {}, useMemo: (factory) => factory(),
      useRef: (current) => ({ current: current === null ? textarea : current }),
      useState: (initial) => [initial, () => {}],
    },
    "react/jsx-runtime": { jsx: element, jsxs: element },
    "@/lib/core/utils": { cn: () => "" },
    "@/lib/balatro/balatro-text-formatter": {
      applyAutoFormatting(...args) {
        autoFormatCalls++;
        return descriptionPreview.applyAutoFormatting(...args);
      },
    },
    "@/lib/description/description-formatting": descriptionFormatting,
    "@/lib/rules/description-variable-registry": { buildDescriptionVariableTokens: () => [] },
    "@/lib/rules/auto-description": { generateDescriptionFromRules: () => "" },
    "@/lib/core/search": {},
    "@phosphor-icons/react": {},
  };
  for (const [module, components] of Object.entries({
    button: ["Button"], input: ["Input"], "scroll-area": ["ScrollArea"],
    textarea: ["Textarea"], separator: ["Separator"],
    tooltip: ["Tooltip", "TooltipContent", "TooltipTrigger"],
  })) {
    imports[`@/components/ui/${module}`] = Object.fromEntries(components.map((name) => [name, name]));
  }
  const { DescriptionEditor } = loadTypeScript("src/components/pages/description-editor.tsx", imports, {
    requestAnimationFrame: (callback) => callback(),
  });
  const tree = DescriptionEditor({ value, onChange(nextValue) { changes.push(nextValue); } });
  return {
    tree, changes, cursorChanges,
    get autoFormatCalls() { return autoFormatCalls; },
    click(label) {
      const elements = allElements(tree);
      const effect = elements.find((node) => node.type === "Button" && elementText(node) === label);
      const tooltip = elements.find((node) => node.type === "Tooltip" && elementText(node) === label);
      const button = effect || allElements(tooltip).find((node) => node.type === "button" || node.type === "Button");
      assert.ok(button, `Missing description toolbar action: ${label}`);
      button.props.onClick();
    },
  };
}

test("description colour toolbar formats every selected line and retains blank rows", () => {
  for (const separator of ["[s]", "\n", "\r\n", "\r", "<br>", "<BR />"]) {
    const selected = `Alpha${separator}Beta${separator}${separator}Gamma`;
    const toolbar = descriptionToolbar(`Before ${selected} After`, 7, 7 + selected.length);
    toolbar.click("Red");
    const expected = `Before {C:red}Alpha{}${separator}{C:red}Beta{}${separator}${separator}{C:red}Gamma{} After`;
    assert.deepEqual(toolbar.changes, [expected], separator);
    assert.deepEqual(toolbar.cursorChanges, [[expected.length - 6, expected.length - 6]], separator);
    assert.equal(toolbar.autoFormatCalls, 0, "Explicit colour should not be overwritten by auto-format");
  }
});

test("description effects retain selected colours and restore partial-line suffix styling", () => {
  const value = "{C:blue}Before Alpha[s]{C:red}Beta{}[s]Gamma After";
  const toolbar = descriptionToolbar(value, value.indexOf("Alpha"), value.indexOf(" After"));
  toolbar.click("Float");
  const segments = descriptionPreview.parseBalatroText(toolbar.changes[0]);
  const segmentFor = (text) => segments.find((segment) => segment.text.includes(text));
  assert.equal(segmentFor("Before").textColor, "text-balatro-blue");
  assert.equal(segmentFor("Before").motion, undefined);
  assert.equal(segmentFor("Alpha").textColor, "text-balatro-blue");
  assert.equal(segmentFor("Beta").textColor, "text-balatro-red");
  for (const text of ["Alpha", "Beta", "Gamma"]) assert.equal(segmentFor(text).motion, 1);
  assert.equal(segmentFor("After").motion, undefined);
  assert.equal(segmentFor("After").textColor, undefined);
});

test("description colour selection overrides inner colour tags and resets on all lines", () => {
  const value = "{s:1.2}Before Alpha[s]{C:blue,E:1}Beta{}[s]Gamma After";
  const toolbar = descriptionToolbar(value, value.indexOf("Alpha"), value.indexOf(" After"));
  toolbar.click("Red");
  const segments = descriptionPreview.parseBalatroText(toolbar.changes[0]);
  for (const text of ["Alpha", "Beta", "Gamma"]) {
    assert.equal(segments.find((segment) => segment.text.includes(text)).textColor, "text-balatro-red");
  }
  assert.equal(segments.find((segment) => segment.text.includes("Alpha")).scale, 1.2);
  assert.equal(segments.find((segment) => segment.text.includes("Beta")).motion, 1);
  assert.equal(segments.find((segment) => segment.text.includes("After")).textColor, undefined);
});

test("description background and scale actions preserve selected leading and trailing breaks", () => {
  for (const [label, property, expected] of [
    ["Red BG", "backgroundColor", "bg-balatro-red"], ["Scale", "scale", 1.1],
  ]) {
    const selected = "\nAlpha\n\nBeta\n";
    const toolbar = descriptionToolbar(`Before${selected}After`, 6, 6 + selected.length);
    toolbar.click(label);
    const segments = descriptionPreview.parseBalatroText(toolbar.changes[0]);
    assert.equal(segments.map((segment) => segment.text).join(""), "Before\nAlpha\n\nBeta\nAfter");
    for (const text of ["Alpha", "Beta"]) {
      assert.equal(segments.find((segment) => segment.text.includes(text))[property], expected);
    }
    assert.equal(segments.find((segment) => segment.text.includes("After"))[property], undefined);
    const preview = descriptionPreview.BalatroText({ text: toolbar.changes[0] });
    assert.equal(allElements(preview).filter((node) => node.type === "br").length, 4);
  }
});

test("existing wrapped description styles render through every line until reset", () => {
  for (const separator of ["[s]", "\n", "\r\n", "<br />"]) {
    const value = `{C:red,X:blue,E:1,s:1.2}Alpha${separator}${separator}Beta{} After`;
    const segments = descriptionPreview.parseBalatroText(value);
    assert.equal(segments[0].text, "Alpha\n\nBeta");
    assert.equal(segments[0].textColor, "text-balatro-red");
    assert.equal(segments[0].backgroundColor, "bg-balatro-blue");
    assert.equal(segments[0].motion, 1);
    assert.equal(segments[0].scale, 1.2);
    assert.equal(segments[1].text, " After");
    assert.equal(segments[1].textColor, undefined);
    const elements = allElements(descriptionPreview.BalatroText({ text: value }));
    assert.equal(elements.filter((node) => node.type === "br").length, 2);
    for (const text of ["Alpha", "Beta"]) {
      const span = elements.find((node) => node.type === "span" && node.props.children === text);
      assert.match(span.props.className, /text-balatro-red/);
      assert.match(span.props.className, /bg-balatro-blue/);
      assert.match(span.props.className, /animate-float/);
      assert.equal(span.props.style.fontSize, "1.2em");
    }
  }
});

test("background formatting strips horizontal whitespace while retaining paragraph gaps", () => {
  const segments = descriptionPreview.parseBalatroText("{X:mult,C:white} X2 \t[s][s] X3 \t{} Plain");
  assert.equal(segments[0].text, "X2\n\nX3");
  assert.equal(segments[1].text, " Plain");
  assert.equal(allElements(descriptionPreview.BalatroText({ text: "{X:mult,C:white}X2[s][s]X3{}" }))
    .filter((node) => node.type === "br").length, 2);
});

test("description preview retains native full-tag replacement and reset behavior", () => {
  const segments = descriptionPreview.parseBalatroText("{C:red,E:1}Alpha[s]Beta{s:1.2}Large[s]Larger{}Plain");
  assert.equal(segments[0].textColor, "text-balatro-red");
  assert.equal(segments[0].motion, 1);
  assert.equal(segments[1].text, "Large\nLarger");
  assert.equal(segments[1].scale, 1.2);
  assert.equal(segments[1].textColor, undefined);
  assert.equal(segments[1].motion, undefined);
  assert.equal(segments[2].textColor, undefined);
  assert.equal(segments[2].scale, undefined);
});

test("manual toolbar formatting preserves keywords while ordinary typing still auto-formats", () => {
  const value = "Alpha\n gold \nBeta";
  const toolbar = descriptionToolbar(value);
  toolbar.click("Red");
  assert.ok(toolbar.changes[0].includes("{C:red} gold {}"));
  assert.equal(toolbar.autoFormatCalls, 0);
  const input = allElements(toolbar.tree).find((node) => node.type === "Textarea");
  input.props.onChange({ target: { value: "gold" } });
  assert.equal(toolbar.autoFormatCalls, 1);
  assert.equal(toolbar.changes[1], "{C:attention}Gold{}");
});

test("description cursor insertion and newline controls keep their insertion behavior", () => {
  const colour = descriptionToolbar("Before After", 7, 7);
  colour.click("Red");
  assert.deepEqual(colour.changes, ["Before {C:red}{}After"]);
  assert.deepEqual(colour.cursorChanges, [[14, 14]]);
  const newline = descriptionToolbar("Before After", 7, 7);
  newline.click("New Line");
  assert.deepEqual(newline.changes, ["Before [s]After"]);
  assert.deepEqual(newline.cursorChanges, [[10, 10]]);
  const restore = descriptionFormatting.insertDescriptionTag("{C:blue}Before After", 15, 15, "{E:1}");
  assert.equal(restore.value, "{C:blue}Before {C:blue,E:1}{C:blue}After");
  assert.equal(restore.cursor, 27);
});

test("selected static and variable colours override alternative colour sources", () => {
  for (const [original, applied, removed, retained] of [
    ["V:1,E:1", "C:red", "V", "E:1"],
    ["C:blue,s:1.2", "V:1", "C", "s:1.2"],
    ["B:1,V:1,E:1", "X:red,C:white", "B", "E:1"],
    ["X:blue,T:tip", "B:1", "X", "T:tip"],
  ]) {
    const value = `{${original}}Before Alpha[s]Beta After`;
    const start = value.indexOf("Alpha");
    const end = value.indexOf(" After");
    const result = descriptionFormatting.insertDescriptionTag(value, start, end, `{${applied}}`).value;
    const selectedTags = result.slice(start, result.indexOf(" After")).match(/\{[^}]*\}/g);
    const styled = selectedTags.filter((tag) => tag.includes(applied));
    assert.equal(styled.length, 2);
    for (const tag of styled) {
      assert.ok(!tag.includes(`${removed}:`), tag);
      assert.ok(tag.includes(retained), tag);
    }
    assert.ok(result.endsWith(`{${original}} After`), "Original suffix colour should return");
  }
});

test("whitespace-only selected rows and preview gaps remain unstyled", () => {
  const value = "Alpha[s] \t [s]Beta After";
  const toolbar = descriptionToolbar(value, 0, value.indexOf(" After"));
  toolbar.click("Red BG");
  assert.equal(toolbar.changes[0], "{X:red,C:white}Alpha{}[s] \t [s]{X:red,C:white}Beta{} After");
  const preview = descriptionPreview.BalatroText({ text: "{X:red,C:white}Alpha[s] \t [s]Beta{} After" });
  const elements = allElements(preview);
  assert.equal(elements.filter((node) => node.type === "br").length, 2);
  assert.ok(!elements.some((node) => node.type === "span" && node.props.className?.includes("bg-balatro-red")
    && typeof node.props.children === "string" && !node.props.children.trim()));
});
