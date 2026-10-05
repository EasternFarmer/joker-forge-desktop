/* Run with `node --test scripts/test-rule-parameters.cjs`. */
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { test } = require("node:test");
const ts = require("typescript");

function loadTypeScript(relativePath, mockImports = {}) {
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
      if (Object.prototype.hasOwnProperty.call(mockImports, name)) {
        return mockImports[name];
      }
      throw new Error(`Unmocked import in rule parameter test: ${name}`);
    },
  }, { filename: filePath });
  return module.exports;
}

const { isParameterVisible } = loadTypeScript("src/components/rule-builder/parameter-visibility.ts");
const effects = JSON.parse(fs.readFileSync(path.join(
  __dirname, "..", "src-tauri/src/mod_engine/catalog/effects.json",
), "utf8"));

const variableCases = [
  ["number", "modify_internal_variable", "internal_variable", "addNumberVariablesToOptions"],
  ["suit", "change_suit_variable", "suit_variable", "addSuitVariablesToOptions"],
  ["rank", "change_rank_variable", "rank_variable", "addRankVariablesToOptions"],
  ["pokerhand", "change_pokerhand_variable", "pokerhand_variable", "addPokerHandVariablesToOptions"],
  ["key", "change_key_variable", "key_variable", "addKeyVariablesToOptions"],
  ["text", "change_text_variable", "text_variable", "addTextVariablesToOptions"],
];

test("Edit Starting Cards shows filters for matching and random selections", () => {
  const effect = effects.find((entry) => entry.id === "edit_all_starting_cards");
  assert.ok(effect);
  for (const [mode, expectedFields] of [
    ["all", []],
    ["matching", ["target_suit", "target_rank"]],
    ["random", ["target_suit", "target_rank", "count"]],
  ]) {
    const parentValues = { selection_method: { value: mode } };
    const visibleFields = effect.params
      .filter((parameter) => isParameterVisible(parameter, effect.params, parentValues))
      .map((parameter) => parameter.id);
    assert.deepEqual(visibleFields, [
      "selection_method", ...expectedFields,
      "enhancement", "seal", "edition", "suit", "rank",
    ], `${mode} exposes the fields needed to choose and modify its cards`);
  }
  const count = effect.params.find((entry) => entry.id === "count");
  assert.equal(count.type, "number");
  assert.equal(count.default, 2, "Initial count opens in numeric mode");
  assert.equal(count.min, 0);
  assert.equal(count.step, 1);
  assert.deepEqual(count.variableTypes, ["number"]);
});

test("existing Edit All Starting Cards projects keep the all-cards default", () => {
  const effect = effects.find((entry) => entry.id === "edit_all_starting_cards");
  const oldValues = {
    enhancement: { value: "m_glass" },
    edition: { value: "polychrome" },
  };
  assert.equal(effect.params.find((entry) => entry.id === "selection_method").default, "all");
  for (const id of ["target_suit", "target_rank", "count"]) {
    const parameter = effect.params.find((entry) => entry.id === id);
    assert.equal(isParameterVisible(parameter, effect.params, oldValues), false, id);
  }
  for (const id of ["target_suit", "target_rank"]) {
    assert.equal(effect.params.find((entry) => entry.id === id).default, "any", id);
  }
});

test("deck start palette exposes subset editing with every built-in suit and rank", async () => {
  const catalogPath = path.join(__dirname, "..", "src-tauri/src/mod_engine/catalog");
  const readCatalog = (name) => JSON.parse(fs.readFileSync(path.join(catalogPath, name), "utf8"));
  const common = readCatalog("common.json");
  const balatroUtils = loadTypeScript("src/lib/balatro/balatro-utils.ts", {
    "@/lib/items/unlock-utils": {},
  });
  const catalog = loadTypeScript("src/components/rule-builder/rule-catalog.ts", {
    "@phosphor-icons/react": {},
    "@/lib/balatro/balatro-utils": balatroUtils,
    "@/lib/services/entity-bridge": { entityBridge: {
      async getRulebuilderCatalog() {
        return {
          triggers: readCatalog("triggers.json"),
          effects: readCatalog("effects.json"),
          conditions: readCatalog("conditions.json"),
          generic_triggers: common.genericTriggers,
          all_objects: common.allObjects,
          trigger_groups: common.triggerGroups,
          option_sources: common.optionSources,
          option_sets: common.optionSets,
        };
      },
    } },
  });
  await catalog.initializeRuleCatalogFromRust();
  assert.ok(catalog.getTriggers("deck").some((trigger) => trigger.id === "card_used"));
  const availableEffects = catalog.getEffectsForTrigger("card_used", "deck");
  const effect = availableEffects.find((entry) => entry.id === "edit_all_starting_cards");
  assert.ok(effect, "Deck start palette includes the compatible saved effect ID");
  assert.equal(effect.label, "Edit Starting Cards");
  for (const [id, expectedValues] of [
    ["target_suit", ["any", "Spades", "Hearts", "Diamonds", "Clubs"]],
    ["target_rank", ["any", "2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K", "A"]],
  ]) {
    const parameter = effect.params.find((entry) => entry.id === id);
    const options = parameter.options({ selection_method: { value: "matching" } });
    assert.deepEqual(Array.from(options, (option) => option.value), expectedValues, id);
    assert.equal(options[0].label, id === "target_suit" ? "Any Suit" : "Any Rank");
  }
  for (const id of ["edit_starting_suits", "edit_starting_ranks"]) {
    assert.ok(availableEffects.some((entry) => entry.id === id), `Existing ${id} remains available`);
  }
  assert.equal(catalog.getEffectsForTrigger("card_used", "consumable")
    .some((entry) => entry.id === effect.id), false, "Deck editing stays in the deck palette");
});

test("consumable use rules expose variable checks and changes for every variable type", async () => {
  const catalogPath = path.join(__dirname, "..", "src-tauri/src/mod_engine/catalog");
  const readCatalog = (name) => JSON.parse(fs.readFileSync(path.join(catalogPath, name), "utf8"));
  const common = readCatalog("common.json");
  const catalog = loadTypeScript("src/components/rule-builder/rule-catalog.ts", {
    "@phosphor-icons/react": {},
    "@/lib/balatro/balatro-utils": {},
    "@/lib/services/entity-bridge": { entityBridge: {
      async getRulebuilderCatalog() {
        return {
          triggers: readCatalog("triggers.json"),
          effects: readCatalog("effects.json"),
          conditions: readCatalog("conditions.json"),
          generic_triggers: common.genericTriggers,
          all_objects: common.allObjects,
          trigger_groups: common.triggerGroups,
        };
      },
    } },
  });
  await catalog.initializeRuleCatalogFromRust();
  assert.ok(catalog.getTriggers("consumable").some((trigger) => trigger.id === "card_used"));
  const availableEffects = catalog.getEffectsForTrigger("card_used", "consumable");
  const availableConditions = catalog.getConditionsForTrigger("card_used", "consumable");
  for (const [type, effectId, conditionId] of variableCases) {
    for (const [definitions, id] of [[availableEffects, effectId], [availableConditions, conditionId]]) {
      const definition = definitions.find((entry) => entry.id === id);
      assert.ok(definition, `Consumable use palette includes ${id}`);
      const variableParameter = definition.params.find((entry) => entry.id === "variable_name");
      assert.ok(variableParameter?.variableTypes.includes(type), `${id} selects ${type} variables`);
      assert.equal(isParameterVisible(variableParameter, definition.params, {}), true, id);
    }
  }
});

test("consumable variable pickers retain the user variable type for every supported type", () => {
  const variableUtils = loadTypeScript("src/lib/rules/user-variable-utils.ts");
  const consumable = {
    objectType: "consumable",
    userVariables: variableCases.map(([type]) => ({ id: type, name: `local_${type}`, type })),
  };
  for (const [type, , , appendOptions] of variableCases) {
    const options = variableUtils[appendOptions]([], consumable);
    assert.equal(options.length, 1, `${type} excludes other variable types`);
    assert.equal(options[0].value, `local_${type}`);
    assert.equal(options[0].valueType, "user_var");
  }
});

function consumableEffect(id) {
  const effect = effects.find((entry) => entry.id === id);
  assert.ok(effect, `Catalog effect ${id} exists`);
  const parameter = effect.params.find((entry) => entry.id === "specific_card");
  assert.ok(parameter, `${id} provides a Specific Card parameter`);
  return { effect, parameter };
}

test("Destroy and Copy Consumable expose Specific Card for every built-in set spelling", () => {
  for (const id of ["destroy_consumable", "copy_consumable"]) {
    const { effect, parameter } = consumableEffect(id);
    for (const set of ["Tarot", "tarot", "Planet", "planet", "Spectral", "spectral"]) {
      assert.equal(isParameterVisible(parameter, effect.params, {
        consumable_type: { value: set },
      }), true, `${effect.label}: ${set}`);
    }
  }
});

test("Destroy and Copy Consumable expose Specific Card for custom consumable sets", () => {
  for (const id of ["destroy_consumable", "copy_consumable"]) {
    const { effect, parameter } = consumableEffect(id);
    for (const set of ["Runes", "test_Runes", "custom_set", " Tarot "]) {
      assert.equal(isParameterVisible(parameter, effect.params, {
        consumable_type: { value: set },
      }), true, `${effect.label}: ${set}`);
    }
  }
});

test("Destroy and Copy Consumable hide Specific Card for unspecific or missing sets", () => {
  for (const id of ["destroy_consumable", "copy_consumable"]) {
    const { effect, parameter } = consumableEffect(id);
    for (const set of ["random", "RANDOM", " any ", "Any", "keyvar", "KEYVAR", "", " ", null, undefined, false, []]) {
      assert.equal(isParameterVisible(parameter, effect.params, {
        consumable_type: { value: set },
      }), false, `${effect.label}: ${JSON.stringify(set)}`);
    }
    assert.equal(isParameterVisible(parameter, effect.params, {}), false, effect.label);
  }
});

test("Create Consumable retains its catalog's unconditional Specific Card selector", () => {
  const { effect, parameter } = consumableEffect("create_consumable");
  assert.equal(parameter.showWhen, undefined);
  for (const set of ["Tarot", "tarot", "Planet", "Spectral", "Runes", "random", "any", "keyvar", "", undefined]) {
    assert.equal(isParameterVisible(parameter, effect.params, {
      set: { value: set },
    }), true, `${effect.label}: ${String(set)}`);
  }
});

test("ordinary showWhen values keep exact matching rather than consumable normalization", () => {
  const definitions = [
    { id: "mode", type: "select" },
    { id: "amount", type: "number", showWhen: { parameter: "mode", values: ["increase"] } },
  ];
  for (const [mode, expected] of [["increase", true], ["Increase", false], [" increase ", false], ["decrease", false]]) {
    assert.equal(isParameterVisible(definitions[1], definitions, {
      mode: { value: mode },
    }), expected, mode);
  }
});

test("checkbox showWhen conditions accept any checked allowed index", () => {
  const definitions = [
    { id: "options", type: "checkbox" },
    { id: "detail", type: "text", showWhen: { parameter: "options", values: ["1", "2"] } },
  ];
  for (const [checked, expected] of [
    [[true, false, false], false], [[false, true, false], true],
    [[false, false, true], true], [[false, false, false], false],
  ]) {
    assert.equal(isParameterVisible(definitions[1], definitions, {
      options: { value: checked },
    }), expected, JSON.stringify(checked));
  }
});

test("nested parameters remain hidden when an ancestor's condition does not match", () => {
  const { effect, parameter } = consumableEffect("destroy_consumable");
  const definitions = [
    { id: "target_mode", type: "select" },
    ...effect.params.map((entry) => entry.id === "consumable_type"
      ? { ...entry, showWhen: { parameter: "target_mode", values: ["specific"] } }
      : entry),
  ];
  assert.equal(isParameterVisible(parameter, definitions, {
    target_mode: { value: "specific" }, consumable_type: { value: "Tarot" },
  }), true);
  assert.equal(isParameterVisible(parameter, definitions, {
    target_mode: { value: "all" }, consumable_type: { value: "Tarot" },
  }), false);
});

test("missing values fall back to catalog defaults when one is defined", () => {
  const definitions = [
    { id: "mode", type: "select", default: "specific" },
    { id: "detail", type: "text", showWhen: { parameter: "mode", values: ["specific"] } },
  ];
  assert.equal(isParameterVisible(definitions[1], definitions, {}), true);
  assert.equal(isParameterVisible(definitions[1], definitions, {
    mode: { value: "random" },
  }), false);
});

test("missing or invalid parent values safely hide dependent parameters", () => {
  const definitions = [
    { id: "mode", type: "select" },
    { id: "detail", type: "text", showWhen: { parameter: "mode", values: ["specific"] } },
  ];
  for (const parentValues of [{}, { mode: { value: null } }, { mode: { value: 42 } }]) {
    assert.equal(isParameterVisible(definitions[1], definitions, parentValues), false);
  }
  assert.equal(isParameterVisible(definitions[1], [definitions[1]], {}), false);
});

test("cyclic visibility dependencies terminate and hide the malformed fields", { timeout: 1000 }, () => {
  const definitions = [
    { id: "one", type: "select", showWhen: { parameter: "two", values: ["yes"] } },
    { id: "two", type: "select", showWhen: { parameter: "one", values: ["yes"] } },
  ];
  assert.equal(isParameterVisible(definitions[0], definitions, {
    one: { value: "yes" }, two: { value: "yes" },
  }), false);
  const self = { id: "self", type: "select", showWhen: { parameter: "self", values: ["yes"] } };
  assert.equal(isParameterVisible(self, [self], { self: { value: "yes" } }), false);
});

function allElements(node) {
  if (Array.isArray(node)) return node.flatMap(allElements);
  if (!node || typeof node !== "object") return [];
  return [node, ...allElements(node.props?.children)];
}

function messageInspector(effect, groupType = "effects") {
  const updates = [];
  const validationMessages = [];
  const react = {
    memo: (component) => component,
    useEffect() {},
    useMemo: (factory) => factory(),
    useState: (initial) => [initial, (value) => validationMessages.push(value)],
  };
  const element = (type, props) => ({ type, props });
  const imports = {
    react: { default: react, ...react },
    "react/jsx-runtime": { jsx: element, jsxs: element },
    "@/lib/balatro/balatro-utils": {},
    "@/lib/rules/user-variable-utils": { getNumberVariables: () => [] },
    "@/lib/services/storage": { useProjectData: () => ({ data: { sounds: [] } }) },
    "@/lib/app/global-user-variables": {
      collectGlobalVariables: () => [], mergeItemVariablesWithGlobals: (item) => item,
    },
    "./rule-catalog": { getEffectTypeById: (id) => effects.find((entry) => entry.id === id) },
    "./parameter-visibility": { isParameterVisible },
    "@/components/ui/input": { Input: "Input" },
    "@/components/ui/button": {},
    "@/components/ui/select": {
      Select: "Select", SelectContent: "SelectContent", SelectItem: "SelectItem",
      SelectTrigger: "SelectTrigger", SelectValue: "SelectValue",
    },
    "@phosphor-icons/react": {},
    "@/lib/core/validation-utils": loadTypeScript("src/lib/core/validation-utils.ts"),
    "@/lib/content/game-vars": {},
    "@/components/ui/checkbox": {},
    "./item-type-badge": {},
    "@/components/ui/icon-button": {},
    "@/components/ui/tooltip": {},
    "@/components/ui/toggle": {},
    "./panel": {},
    "@/components/ui/help-tooltip-icon": {},
  };
  const { default: Inspector } = loadTypeScript("src/components/rule-builder/inspector.tsx", imports);
  const rule = { id: "rule", trigger: "hand_played", effects: [], randomGroups: [], loops: [] };
  if (groupType === "effects") rule.effects = [effect];
  else rule[groupType] = [{ id: "group", effects: [effect] }];
  const tree = Inspector({
    position: { x: 0, y: 0 }, joker: { id: "joker", userVariables: [] },
    selectedRule: rule, selectedEffect: effect, itemType: "joker",
    onUpdateEffect: (ruleId, effectId, update) => updates.push({ ruleId, effectId, update }),
  });
  const elements = allElements(tree);
  return {
    mode: elements.find((node) => node.type === "Select"),
    options: elements.filter((node) => node.type === "SelectItem"),
    message: elements.find((node) => node.type === "Input"),
    updates, validationMessages,
  };
}

const sizeEffects = ["edit_hand_size", "edit_play_size", "edit_discard_size"];

test("hand, play, and discard size effects offer default, custom, and silent messages", () => {
  for (const type of sizeEffects) {
    for (const groupType of ["effects", "randomGroups", "loops"]) {
      const inspector = messageInspector({ id: "effect", type, params: {} }, groupType);
      assert.equal(inspector.mode.props.value, "default", `${type} ${groupType}`);
      assert.deepEqual(inspector.options.map((node) => [node.props.value, node.props.children]), [
        ["default", "Default message"], ["custom", "Custom message"], ["none", "No message"],
      ]);
      assert.equal(inspector.message, undefined, "Default mode hides custom text");
      inspector.mode.props.onValueChange("none");
      assert.deepEqual(JSON.parse(JSON.stringify(inspector.updates)), [{
        ruleId: "rule", effectId: "effect", update: { messageMode: "none" },
      }]);
    }
  }
});

test("legacy custom messages remain selected and message toggles preserve saved text", () => {
  for (const type of sizeEffects) {
    const effect = { id: "effect", type, params: {}, customMessage: "Let's play!" };
    const inspector = messageInspector(effect);
    assert.equal(inspector.mode.props.value, "custom");
    assert.equal(inspector.message.props.value, effect.customMessage);
    for (const mode of ["none", "default", "custom"]) {
      inspector.mode.props.onValueChange(mode);
      const update = inspector.updates.at(-1).update;
      assert.deepEqual(Object.keys(update), ["messageMode"], "Toggling retains saved text");
      const updated = messageInspector({ ...effect, ...update });
      assert.equal(updated.mode.props.value, mode);
      assert.equal(updated.message?.props.value, mode === "custom" ? effect.customMessage : undefined);
    }
  }
});

test("custom size messages accept ordinary punctuation and retain single-line length limits", () => {
  for (const type of sizeEffects) {
    const inspector = messageInspector({ id: "effect", type, params: {}, messageMode: "custom" });
    assert.equal(inspector.message.props.placeholder, "Leave blank for default message");
    for (const value of ["Let's play!", '"Extra" cards', "A\\B", "`Extra`", ""]) {
      inspector.message.props.onChange({ target: { value } });
      assert.equal(inspector.validationMessages.at(-1), "", value);
      assert.equal(inspector.updates.at(-1).update.customMessage, value || undefined);
      assert.equal(inspector.updates.at(-1).update.messageMode, "custom", "Editing keeps Custom selected");
    }
    for (const [value, expected] of [
      ["x".repeat(101), "Message must be 100 characters or less"],
      ["First\nSecond", "Message cannot contain line breaks"],
      ["First\rSecond", "Message cannot contain line breaks"],
    ]) {
      inspector.message.props.onChange({ target: { value } });
      assert.equal(inspector.validationMessages.at(-1), expected);
    }
  }
});

test("other effects retain their existing custom message input", () => {
  const inspector = messageInspector({
    id: "effect", type: "add_chips", params: {}, customMessage: "Bonus!", messageMode: "none",
  });
  assert.equal(inspector.mode, undefined);
  assert.equal(inspector.message.props.label, "Message");
  assert.equal(inspector.message.props.value, "Bonus!");
  inspector.message.props.onChange({ target: { value: "Let's play!" } });
  assert.equal(inspector.validationMessages.at(-1), "Message cannot contain quotation marks");
});

test("CLI requests preserve message settings for normal, random, and loop effects", () => {
  const { createCliItemRequest, serializeCliItemRequest } = loadTypeScript(
    "src/lib/export/cli-item-generation.ts",
  );
  const effects = sizeEffects.map((type, index) => ({
    id: `effect${index}`, type, messageMode: ["default", "custom", "none"][index],
    customMessage: "Saved custom text", params: { value: index + 1 },
  }));
  const legacy = { id: "legacy", type: "edit_hand_size", customMessage: "Legacy!", params: {} };
  const request = createCliItemRequest({ rules: [{
    id: "rule", effects: [...effects, legacy],
    randomGroups: [{ id: "random", effects: [...effects, legacy] }],
    loops: [{ id: "loop", effects: [...effects, legacy] }],
  }] });
  const rule = JSON.parse(serializeCliItemRequest(request)).itemData.rules[0];
  for (const list of [rule.effects, rule.randomGroups[0].effects, rule.loops[0].effects]) {
    for (let index = 0; index < effects.length; index++) {
      assert.equal(list[index].messageMode, effects[index].messageMode);
      assert.equal(list[index].customMessage, "Saved custom text");
      assert.deepEqual(list[index].params, { value: { value: index + 1 } });
    }
    assert.equal(list[3].customMessage, "Legacy!");
    assert.equal(Object.hasOwn(list[3], "messageMode"), false, "Legacy mode is not invented");
  }
});
