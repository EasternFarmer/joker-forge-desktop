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
    "./probability-sources": loadTypeScript("src/components/rule-builder/probability-sources.ts"),
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

const paletteCatalogPromise = (async () => {
  const catalogPath = path.join(__dirname, "..", "src-tauri/src/mod_engine/catalog");
  const readCatalog = (name) => JSON.parse(fs.readFileSync(path.join(catalogPath, name), "utf8"));
  const common = readCatalog("common.json");
  const catalog = loadTypeScript("src/components/rule-builder/rule-catalog.ts", {
    "@phosphor-icons/react": {},
    "@/lib/balatro/balatro-utils": {},
    "@/lib/services/entity-bridge": { entityBridge: {
      async getRulebuilderCatalog() {
        return {
          triggers: readCatalog("triggers.json"), effects: readCatalog("effects.json"),
          conditions: readCatalog("conditions.json"), generic_triggers: common.genericTriggers,
          all_objects: common.allObjects, trigger_groups: common.triggerGroups,
        };
      },
    } },
  });
  await catalog.initializeRuleCatalogFromRust();
  return catalog;
})();

async function searchablePalette(itemType = "joker", trigger = null) {
  const catalog = await paletteCatalogPromise;
  const categories = [
    ...catalog.TRIGGER_CATEGORIES, ...catalog.CONDITION_CATEGORIES, ...catalog.EFFECT_CATEGORIES,
  ];
  const state = [];
  let hookIndex = 0;
  const react = {
    useState(initial) {
      const index = hookIndex++;
      if (!(index in state)) state[index] = index === 1
        ? new Set([...categories.map((category) => category.label), "Other"])
        : initial;
      return [state[index], (value) => {
        state[index] = typeof value === "function" ? value(state[index]) : value;
      }];
    },
    useEffect() {},
    useMemo: (factory) => factory(),
  };
  const element = (type, props) => ({ type, props });
  const { default: BlockPalette } = loadTypeScript("src/components/rule-builder/block-palette.tsx", {
    react: { default: react, ...react },
    "react/jsx-runtime": { jsx: element, jsxs: element },
    "@dnd-kit/core": {},
    "framer-motion": { motion: { div: "motion.div" }, AnimatePresence: "AnimatePresence" },
    "@phosphor-icons/react": {},
    "./block-component": { default: "BlockComponent" },
    "@/components/ui/icon-button": { default: "IconButton" },
    "./item-type-badge": { default: "ItemTypeBadge" },
    "./panel": { default: "Panel" },
    "@/components/ui/help-tooltip-icon": { default: "HelpTooltipIcon" },
    "./rule-catalog": catalog,
  });
  const props = {
    position: { x: 0, y: 0 }, selectedRule: trigger ? { id: "rule", trigger } : null,
    itemType, onAddTrigger() {}, onAddCondition() {}, onAddEffect() {},
    onClose() {}, onPositionChange() {},
  };
  const render = () => {
    hookIndex = 0;
    return allElements(BlockPalette(props));
  };
  return {
    search(value) {
      render().find((node) => node.type === "input").props.onChange({ target: { value } });
      return render().filter((node) => node.props?.blockId).map((node) => node.props.blockId);
    },
    tab(value) {
      const prefix = value === "triggers" ? "Show all trigger" : `Show only ${value}`;
      const toggle = render().find((node) => node.type === "IconButton"
        && node.props.tooltip.startsWith(prefix));
      assert.ok(toggle && !toggle.props.disabled, `${value} tab is available`);
      toggle.props.onClick();
    },
    catalog,
  };
}

test("trigger search finds every hand drawn trigger despite intervening label words", async () => {
  const palette = await searchablePalette();
  for (const query of ["hand drawn", "  HaNd\t DRAWN  ", "drawn hand"]) {
    const matches = palette.search(query);
    assert.ok(matches.includes("hand_drawn"), query);
    assert.ok(matches.includes("first_hand_drawn"), query);
    assert.equal(matches.includes("hand_played"), false, "Every search word must match");
  }
  assert.deepEqual(palette.search("hand drawn nonexistent"), []);
});

test("empty and whitespace searches preserve the complete available trigger list", async () => {
  const palette = await searchablePalette();
  const expected = Array.from(palette.catalog.getTriggers("joker"), (trigger) => trigger.id).sort();
  for (const query of ["", " \t\n "]) {
    assert.deepEqual(palette.search(query).sort(), expected, JSON.stringify(query));
  }
});

test("trigger search keeps item availability while combining label and description words", async () => {
  const jokerPalette = await searchablePalette();
  assert.ok(jokerPalette.search("drawn player").includes("hand_drawn"),
    "Words can match the label and its description together");
  const consumablePalette = await searchablePalette("consumable");
  assert.deepEqual(consumablePalette.search("hand drawn"), [],
    "Joker-only triggers remain unavailable to consumables");
  const expected = Array.from(consumablePalette.catalog.getTriggers("consumable"), (trigger) => trigger.id).sort();
  assert.deepEqual(consumablePalette.search("").sort(), expected);
});

test("condition and effect searches use the same word matching within compatible results", async () => {
  const palette = await searchablePalette("joker", "hand_played");
  assert.ok(palette.search("poker type").includes("hand_type"),
    "Condition words may appear in a different order in the description");
  assert.deepEqual(palette.search("poker type nonexistent"), []);
  palette.tab("effects");
  assert.ok(palette.search("  SIZE\tEDIT ").includes("edit_hand_size"),
    "Effect search handles order, case, and whitespace consistently");
  assert.deepEqual(palette.search("size edit nonexistent"), []);
  const expected = Array.from(palette.catalog.getEffectsForTrigger("hand_played", "joker"), (effect) => effect.id).sort();
  assert.deepEqual(palette.search("").sort(), expected, "Search does not expand the compatibility set");
});

// Queue reducer actions until the next render, as React does for batched events.
function ruleHistoryHarness() {
  const slots = [];
  const pendingActions = [];
  let hookIndex = 0;
  const react = {
    useReducer(reducer, initial, initialize) {
      const index = hookIndex++;
      if (!slots[index]) {
        slots[index] = {
          state: initialize ? initialize(initial) : initial,
          dispatch: (action) => pendingActions.push({ index, action }),
        };
      }
      slots[index].reducer = reducer;
      return [slots[index].state, slots[index].dispatch];
    },
    useCallback(callback, dependencies) {
      const index = hookIndex++;
      const previous = slots[index];
      if (!previous || dependencies.length !== previous.dependencies.length
        || dependencies.some((value, position) => !Object.is(value, previous.dependencies[position]))) {
        slots[index] = { callback, dependencies };
      }
      return slots[index].callback;
    },
    useMemo: (factory) => factory(),
  };
  const { useRuleHistory } = loadTypeScript("src/components/rule-builder/use-rule-history.ts", {
    react: { default: react, ...react },
  });
  const render = () => {
    for (const { index, action } of pendingActions.splice(0)) {
      slots[index].state = slots[index].reducer(slots[index].state, action);
    }
    hookIndex = 0;
    return useRuleHistory();
  };
  return { render, initial: render() };
}

function historyRules(id = "joker", chips = 10) {
  return [{
    id, trigger: "hand_played", blueprintCompatible: true,
    position: { x: 20, y: 40 }, conditionGroups: [], randomGroups: [], loops: [],
    effects: [{ id: `${id}-chips`, type: "add_chips", params: { value: { value: chips } } }],
  }];
}

const historyChips = (history) => history.rules[0]?.effects[0]?.params.value.value;
const plainSnapshot = (value) => JSON.parse(JSON.stringify(value));

test("opening populated rule history makes initial undo and redo harmless", () => {
  const harness = ruleHistoryHarness();
  const callbacks = harness.initial;
  const savedRules = historyRules();
  callbacks.resetHistory(savedRules);
  callbacks.resetHistory(savedRules);
  callbacks.handleUndo();
  callbacks.handleRedo();
  let history = harness.render();
  assert.deepEqual(plainSnapshot(history.rules), savedRules);
  assert.equal(history.canUndo, false);
  assert.equal(history.canRedo, false);
  assert.equal(history.historyTimeline.length, 1);
  assert.equal(history.historyCurrentIndex, 0);
  savedRules[0].effects[0].params.value.value = 999;
  history = harness.render();
  assert.equal(historyChips(history), 10, "Loaded rules are independent of the saved item object");
});

test("the first rule in a genuinely empty builder can undo to empty and redo", () => {
  const harness = ruleHistoryHarness();
  const callbacks = harness.initial;
  callbacks.resetHistory([]);
  callbacks.handleUndo();
  assert.deepEqual(plainSnapshot(harness.render().rules), []);
  callbacks.setRules((rules) => [...rules, ...historyRules("new")]);
  assert.deepEqual(plainSnapshot(harness.render().rules), historyRules("new"));
  callbacks.handleUndo();
  let history = harness.render();
  assert.deepEqual(plainSnapshot(history.rules), []);
  assert.equal(history.canRedo, true);
  callbacks.handleRedo();
  history = harness.render();
  assert.deepEqual(plainSnapshot(history.rules), historyRules("new"));
  assert.equal(history.canRedo, false);
});

test("real rule changes undo and redo the complete saved blocks", () => {
  const harness = ruleHistoryHarness();
  const callbacks = harness.initial;
  callbacks.resetHistory(historyRules());
  harness.render();
  const edited = historyRules("joker", 25);
  callbacks.setRules(edited);
  let history = harness.render();
  assert.equal(historyChips(history), 25);
  assert.equal(history.canUndo, true);
  assert.equal(history.canRedo, false);
  edited[0].position.x = 999;
  edited[0].effects[0].params.value.value = 999;
  callbacks.handleUndo();
  history = harness.render();
  assert.deepEqual(plainSnapshot(history.rules), historyRules());
  assert.equal(history.canUndo, false);
  assert.equal(history.canRedo, true);
  callbacks.handleRedo();
  history = harness.render();
  assert.deepEqual(plainSnapshot(history.rules), historyRules("joker", 25),
    "Later input mutations do not corrupt the redo snapshot");
});

test("queued edits and rapid undo redo always operate on the latest history", () => {
  const harness = ruleHistoryHarness();
  const callbacks = harness.initial;
  const increaseChips = (rules) => rules.map((rule) => ({
    ...rule, effects: rule.effects.map((effect) => ({
      ...effect, params: { ...effect.params, value: { value: effect.params.value.value + 1 } },
    })),
  }));
  callbacks.resetHistory(historyRules());
  callbacks.setRules(increaseChips);
  callbacks.setRules(increaseChips);
  callbacks.handleUndo();
  callbacks.handleUndo();
  callbacks.handleRedo();
  let history = harness.render();
  assert.equal(historyChips(history), 11);
  assert.equal(history.historyCurrentIndex, 1);
  assert.equal(history.canUndo, true);
  assert.equal(history.canRedo, true);
  for (const name of ["setRules", "resetHistory", "handleUndo", "handleRedo", "restoreHistoryAt"]) {
    assert.equal(history[name], callbacks[name], `${name} remains stable between renders`);
  }
  callbacks.handleRedo();
  callbacks.handleUndo();
  callbacks.handleRedo();
  history = harness.render();
  assert.equal(historyChips(history), 12);
  assert.equal(history.canRedo, false);
});

test("equivalent rule updates and unchanged positions preserve available redo", () => {
  const harness = ruleHistoryHarness();
  const callbacks = harness.initial;
  callbacks.resetHistory(historyRules());
  callbacks.setRules(historyRules("joker", 25));
  callbacks.handleUndo();
  let history = harness.render();
  callbacks.setRules((rules) => rules);
  callbacks.setRules((rules) => plainSnapshot(rules));
  callbacks.setRules((rules) => rules.map((rule) => ({
    ...rule, position: { x: rule.position.x, y: rule.position.y },
  })));
  history = harness.render();
  assert.equal(history.historyTimeline.length, 2);
  assert.equal(history.historyCurrentIndex, 0);
  assert.equal(history.canRedo, true, "A no-op drag or update cannot erase redo");
  callbacks.handleRedo();
  assert.equal(historyChips(harness.render()), 25);
});

test("editing after undo replaces only the abandoned future", () => {
  const harness = ruleHistoryHarness();
  const callbacks = harness.initial;
  callbacks.resetHistory(historyRules());
  callbacks.setRules(historyRules("joker", 20));
  callbacks.setRules(historyRules("joker", 30));
  callbacks.handleUndo();
  callbacks.setRules(historyRules("joker", 40));
  callbacks.handleRedo();
  let history = harness.render();
  assert.equal(historyChips(history), 40);
  assert.equal(history.canRedo, false);
  assert.equal(history.historyTimeline.length, 3);
  callbacks.handleUndo();
  history = harness.render();
  assert.equal(historyChips(history), 20, "The earlier real edit is retained");
});

test("reopening or switching items establishes a fresh undo baseline", () => {
  const harness = ruleHistoryHarness();
  const callbacks = harness.initial;
  callbacks.resetHistory(historyRules("first"));
  callbacks.setRules(historyRules("first", 20));
  callbacks.handleUndo();
  harness.render();
  for (const savedRules of [historyRules("first", 30), [], historyRules("second", 50)]) {
    callbacks.resetHistory(savedRules);
    callbacks.handleUndo();
    callbacks.handleRedo();
    const history = harness.render();
    assert.deepEqual(plainSnapshot(history.rules), savedRules);
    assert.equal(history.canUndo, false);
    assert.equal(history.canRedo, false);
    assert.equal(history.historyTimeline.length, 1);
  }
});

test("history panel jumps preserve undo redo and branch from the selected state", () => {
  const harness = ruleHistoryHarness();
  const callbacks = harness.initial;
  callbacks.resetHistory(historyRules("joker", 0));
  for (const amount of [1, 2, 3]) callbacks.setRules(historyRules("joker", amount));
  callbacks.restoreHistoryAt(0);
  let history = harness.render();
  assert.equal(historyChips(history), 0);
  assert.equal(history.canUndo, false);
  assert.equal(history.canRedo, true);
  callbacks.restoreHistoryAt(2);
  callbacks.restoreHistoryAt(2);
  history = harness.render();
  assert.equal(historyChips(history), 2);
  assert.equal(history.historyCurrentIndex, 2);
  assert.equal(history.historyTimeline.length, 4);
  assert.equal(history.canUndo, true);
  assert.equal(history.canRedo, true);
  callbacks.handleRedo();
  callbacks.handleUndo();
  callbacks.setRules(historyRules("joker", 9));
  history = harness.render();
  assert.equal(historyChips(history), 9);
  assert.equal(history.canRedo, false);
  callbacks.handleUndo();
  assert.equal(historyChips(harness.render()), 2);
});

test("bounded rule history retains the newest 64 undo steps and all corresponding redo", () => {
  const harness = ruleHistoryHarness();
  const callbacks = harness.initial;
  callbacks.resetHistory(historyRules("joker", 0));
  for (let amount = 1; amount <= 70; amount++) callbacks.setRules(historyRules("joker", amount));
  let history = harness.render();
  assert.equal(history.historyTimeline.length, 65);
  assert.equal(history.historyCurrentIndex, 64);
  for (let count = 0; count < 75; count++) callbacks.handleUndo();
  history = harness.render();
  assert.equal(historyChips(history), 6);
  assert.equal(history.canUndo, false);
  assert.equal(history.canRedo, true);
  for (let count = 0; count < 75; count++) callbacks.handleRedo();
  history = harness.render();
  assert.equal(historyChips(history), 70);
  assert.equal(history.canRedo, false);
  assert.equal(history.historyTimeline.length, 65);
});

function ruleBuilderInitialization(history) {
  const filePath = path.join(__dirname, "..", "src/components/rule-builder/rule-builder.tsx");
  const source = ts.createSourceFile(filePath, fs.readFileSync(filePath, "utf8"),
    ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let initialization;
  const visit = (node) => {
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression)
      && node.expression.text === "useEffect") {
      const dependencies = node.arguments[1];
      if (dependencies && ts.isArrayLiteralExpression(dependencies)
        && dependencies.elements.length === 2
        && ts.isIdentifier(dependencies.elements[0]) && dependencies.elements[0].text === "isOpen"
        && ts.isPropertyAccessExpression(dependencies.elements[1])
        && dependencies.elements[1].expression.getText(source) === "item"
        && dependencies.elements[1].name.text === "id") {
        initialization = node.arguments[0];
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(source);
  assert.ok(initialization, "The real builder item/open initialization effect is present");

  // Run that effect with the real history hook; other editor state is incidental.
  const context = {
    isOpen: true, item: { id: "first" }, existingRules: [], itemType: "joker", reforged: false,
    normalizeRuleForBuilder: (rule) => rule, cloneRulesSnapshot: plainSnapshot,
    resetHistory: history.resetHistory, setRules: history.setRules,
    setTimeout: () => 1, clearTimeout() {},
  };
  for (const name of [
    "customCodeDebounceRef", "customCodeRef", "lastGeneratedCleanRef", "lastGeneratedSegmentsRef",
    "lastSegmentsRef", "linkedFieldRangesRef", "fieldSourceRulesRef", "editorCodeRef",
    "prevRulesSnapshotRef", "rulesRef", "pendingEditorCodeRef",
  ]) context[name] = { current: null };
  context.editorRevisionRef = { current: 0 };
  context.pendingFieldValuesRef = { current: new Map() };
  context.editedFieldKeysRef = { current: new Set() };
  for (const name of [
    "setCustomCode", "setSelectedItem", "setSelectedRuleIds", "setSelectionRect", "setIsDragSelecting",
    "setSelectedGameVariable", "setLiveCodePreviewTarget", "setLiveCodeWidthPercent",
    "setIsInitialLoadComplete", "setIsFirstSelection", "setShowNoRulesMessage",
  ]) context[name] = () => {};
  const compiled = ts.transpileModule(`(${initialization.getText(source)})`, {
    compilerOptions: { target: ts.ScriptTarget.ES2022 }, fileName: filePath,
  }).outputText;
  const run = vm.runInNewContext(compiled, context, { filename: filePath });
  return { context, run };
}

test("the real builder open and item-switch effects reset history before undo", () => {
  const harness = ruleHistoryHarness();
  const callbacks = harness.initial;
  const initialization = ruleBuilderInitialization(callbacks);
  const openItem = (id, rules) => {
    initialization.context.isOpen = true;
    initialization.context.item = { id };
    initialization.context.existingRules = rules;
    initialization.run();
    callbacks.handleUndo();
    callbacks.handleRedo();
    const history = harness.render();
    assert.deepEqual(plainSnapshot(history.rules), rules);
    assert.equal(history.canUndo, false);
    assert.equal(history.canRedo, false);
  };
  openItem("first", historyRules("first"));
  callbacks.setRules(historyRules("first", 20));
  harness.render();
  initialization.context.isOpen = false;
  initialization.run();
  openItem("first", historyRules("first", 20));
  openItem("second", historyRules("second", 30));
  openItem("empty", []);
});

function builderHandler(name, history, savedRules, selection = {}) {
  const filePath = path.join(__dirname, "..", "src/components/rule-builder/rule-builder.tsx");
  const source = ts.createSourceFile(filePath, fs.readFileSync(filePath, "utf8"),
    ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let initializer;
  const visit = (node) => {
    if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.name.text === name) {
      initializer = node.initializer;
    }
    ts.forEachChild(node, visit);
  };
  visit(source);
  assert.ok(initializer, `The real ${name} handler is present`);
  let nextId = 0;
  const selectedItems = [];
  const selectedRules = [];
  const context = {
    rulesRef: { current: savedRules }, setRules: history.setRules,
    selectedItem: selection.item ?? { type: "trigger", ruleId: savedRules[0]?.id },
    selectedRuleIds: selection.ids ?? [], selectedRuleIdSet: new Set(selection.ids ?? []),
    crypto: { randomUUID: () => `new-${++nextId}` },
    useCallback: (callback) => callback,
    createConditionFromType: (type) => ({ id: `condition-${++nextId}`, type, negate: false, params: {} }),
    setSelectedItem: (item) => selectedItems.push(plainSnapshot(item)),
    setSelectedRuleIds: (ids) => selectedRules.push(Array.from(ids)),
  };
  const compiled = ts.transpileModule(`(${initializer.getText(source)})`, {
    compilerOptions: { target: ts.ScriptTarget.ES2022 }, fileName: filePath,
  }).outputText;
  return {
    run: vm.runInNewContext(compiled, context, { filename: filePath }),
    selectedItems, selectedRules,
  };
}

test("adding a condition selects its actual group with deferred history updates", () => {
  for (const mode of ["new group", "first group", "selected group"]) {
    const harness = ruleHistoryHarness();
    const callbacks = harness.initial;
    const savedRules = historyRules();
    if (mode !== "new group") savedRules[0].conditionGroups = [
      { id: "first-group", operator: "and", conditions: [] },
      { id: "second-group", operator: "or", conditions: [] },
    ];
    callbacks.resetHistory(savedRules);
    const loadedRules = harness.render().rules;
    const handler = builderHandler("addCondition", callbacks, loadedRules,
      mode === "selected group" ? {
        item: { type: "condition", ruleId: "joker", groupId: "second-group" },
      } : {});
    handler.run("hand_type");
    const selected = handler.selectedItems.at(-1);
    assert.equal(selected?.type, "condition", "Selection is made before reducer evaluation");
    assert.equal(typeof selected.groupId, "string", "The new group ID is available immediately");
    const editedRules = harness.render().rules;
    const group = editedRules[0].conditionGroups.find((entry) => entry.id === selected.groupId);
    assert.ok(group.conditions.some((condition) => condition.id === selected.itemId), mode);
    if (mode !== "new group") assert.equal(group.id,
      mode === "selected group" ? "second-group" : "first-group");
    callbacks.handleUndo();
    assert.deepEqual(plainSnapshot(harness.render().rules), savedRules);
    callbacks.handleRedo();
    assert.deepEqual(plainSnapshot(harness.render().rules), plainSnapshot(editedRules));
  }
});

test("duplicating one rule immediately selects a stable undoable copy", () => {
  const harness = ruleHistoryHarness();
  const callbacks = harness.initial;
  callbacks.resetHistory(historyRules());
  const loaded = harness.render().rules;
  const handler = builderHandler("duplicateRule", callbacks, loaded);
  handler.run("joker");
  const selected = handler.selectedItems.at(-1);
  assert.equal(selected?.type, "trigger", "Selection is made before reducer evaluation");
  assert.notEqual(selected.ruleId, "joker");
  const edited = harness.render().rules;
  assert.equal(edited.length, 2);
  assert.equal(edited[1].id, selected.ruleId);
  assert.deepEqual(plainSnapshot(edited[1].position), { x: 50, y: 70 });
  assert.notEqual(edited[1].effects[0].id, edited[0].effects[0].id);
  callbacks.handleUndo();
  assert.deepEqual(plainSnapshot(harness.render().rules), historyRules());
  callbacks.handleRedo();
  assert.deepEqual(plainSnapshot(harness.render().rules), plainSnapshot(edited));
});

test("duplicating multiple rules selects all copies before deferred history evaluation", () => {
  const harness = ruleHistoryHarness();
  const callbacks = harness.initial;
  const savedRules = [...historyRules("first"), ...historyRules("second")];
  callbacks.resetHistory(savedRules);
  const loaded = harness.render().rules;
  const handler = builderHandler("duplicateSelectedRules", callbacks, loaded, {
    ids: ["first", "second"],
  });
  handler.run();
  const selected = handler.selectedRules.at(-1);
  assert.equal(selected?.length, 2, "Selected copy IDs exist before reducer evaluation");
  assert.equal(new Set(selected).size, 2);
  assert.equal(handler.selectedItems.at(-1), null);
  const edited = harness.render().rules;
  assert.equal(edited.length, 4);
  assert.deepEqual(Array.from(edited.slice(2), (rule) => rule.id), selected);
  callbacks.handleUndo();
  assert.deepEqual(plainSnapshot(harness.render().rules), savedRules);
  callbacks.handleRedo();
  assert.deepEqual(plainSnapshot(harness.render().rules), plainSnapshot(edited));
});

function probabilityRules() {
  const group = (id, numerator, denominator) => ({
    id, chance_numerator: { value: numerator }, chance_denominator: { value: denominator },
    respect_probability_effects: false, custom_key: "", effects: [],
  });
  const rule = (id, trigger, randomGroups) => ({
    id, trigger, position: { x: 0, y: 0 }, blueprintCompatible: true,
    conditionGroups: [], effects: [], loops: [], randomGroups,
  });
  return [
    rule("initial-rule", "hand_drawn", [
      group("rgid-original", 1, 5), group("rgid-secondary", 2, 7),
    ]),
    rule("result-rule", "probability_result", [group("rgid-chain", 1, 3)]),
    rule("empty-rule", "hand_played", []),
  ];
}

function probabilityInspector(condition, rules, catalog) {
  const updates = [];
  const react = {
    memo: (component) => component, useEffect() {}, useMemo: (factory) => factory(),
    useState: (initial) => [initial, () => {}],
  };
  const element = (type, props) => ({ type, props });
  const { default: Inspector } = loadTypeScript("src/components/rule-builder/inspector.tsx", {
    react: { default: react, ...react },
    "react/jsx-runtime": { jsx: element, jsxs: element },
    "@/lib/balatro/balatro-utils": {},
    "@/lib/rules/user-variable-utils": { getNumberVariables: () => [] },
    "@/lib/services/storage": { useProjectData: () => ({ data: { sounds: [] } }) },
    "@/lib/app/global-user-variables": {
      collectGlobalVariables: () => [], mergeItemVariablesWithGlobals: (item) => item,
    },
    "./rule-catalog": catalog,
    "./parameter-visibility": { isParameterVisible },
    "./probability-sources": loadTypeScript("src/components/rule-builder/probability-sources.ts"),
    "@/components/ui/input": { Input: "Input" },
    "@/components/ui/button": { Button: "Button" },
    "@/components/ui/select": {
      Select: "Select", SelectContent: "SelectContent", SelectItem: "SelectItem",
      SelectTrigger: "SelectTrigger", SelectValue: "SelectValue",
    },
    "@phosphor-icons/react": {},
    "@/lib/core/validation-utils": loadTypeScript("src/lib/core/validation-utils.ts"),
    "@/lib/content/game-vars": {},
    "@/components/ui/checkbox": {}, "./item-type-badge": {},
    "@/components/ui/icon-button": {}, "@/components/ui/tooltip": {},
    "@/components/ui/toggle": {}, "./panel": {},
    "@/components/ui/help-tooltip-icon": {},
  });
  const selectedRule = { ...probabilityRules()[1], conditionGroups: [{
    id: "conditions", operator: "and", conditions: [condition],
  }] };
  const elements = allElements(Inspector({
    position: { x: 0, y: 0 }, joker: { id: "joker", userVariables: [] },
    rules, selectedRule, selectedCondition: condition, itemType: "joker",
    onUpdateCondition: (ruleId, conditionId, update) => updates.push({ ruleId, conditionId, update }),
  }));
  const fields = new Map(elements
    .filter((node) => typeof node.type === "function" && node.props?.param)
    .map((node) => [node.props.param.id, allElements(node.type(node.props))]));
  return {
    fields,
    fieldSelect: (id) => fields.get(id)?.find((node) => node.type === "Select"),
    fieldOptions: (id) => fields.get(id)?.filter((node) => node.type === "SelectItem") ?? [],
    groupSelect: elements.find((node) => node.type === "Select"),
    groupOptions: elements.filter((node) => node.type === "SelectItem"),
    groupPlaceholder: elements.find((node) => node.type === "SelectValue"),
    updates,
  };
}

const probabilityCondition = (params = {}) => ({
  id: "probability-condition", type: "probability_succeeded", negate: false, params,
});

test("probability result source preserves legacy status and reveals a group only when selected", async () => {
  const catalog = await paletteCatalogPromise;
  const condition = catalog.getConditionTypeById("probability_succeeded");
  const visible = (params) => condition.params
    .filter((param) => isParameterVisible(param, condition.params, params))
    .map((param) => param.id);
  assert.deepEqual(Array.from(visible({ status: { value: "failed" } })), ["source", "status"]);
  assert.deepEqual(Array.from(visible({ source: { value: "any" } })), ["source", "status"]);
  assert.deepEqual(Array.from(visible({ source: { value: "chance_group" } })), [
    "source", "group_id", "status",
  ]);
  const source = condition.params.find((param) => param.id === "source");
  assert.equal(source.default, "any");
  assert.deepEqual(Array.from(source.options, (option) => option.value), ["any", "chance_group"]);
  assert.equal(condition.params.find((param) => param.id === "group_id").type, "select");
  assert.deepEqual(Array.from(condition.params.find((param) => param.id === "status").options,
    (option) => option.value), ["succeeded", "failed"]);
});

test("chance group labels use rule positions while stable IDs survive reordering", () => {
  const { getChanceGroupOptions } = loadTypeScript("src/components/rule-builder/probability-sources.ts");
  const rules = probabilityRules();
  const saved = plainSnapshot(rules);
  const options = getChanceGroupOptions(rules);
  assert.deepEqual(plainSnapshot(options), [
    { value: "rgid-original", label: "Rule 1 · Chance 1 (1 in 5)" },
    { value: "rgid-secondary", label: "Rule 1 · Chance 2 (2 in 7)" },
    { value: "rgid-chain", label: "Rule 2 · Chance 1 (1 in 3)" },
  ]);
  const reordered = [rules[1], { ...rules[0], randomGroups: [...rules[0].randomGroups].reverse() }];
  const selected = getChanceGroupOptions(reordered, "rgid-original")
    .find((option) => option.value === "rgid-original");
  assert.equal(selected.label, "Rule 2 · Chance 2 (1 in 5)");
  assert.equal(selected.disabled, undefined);
  assert.ok(options.every((option) => !option.label.includes("rgid-")), "Raw identifiers stay hidden");
  assert.deepEqual(plainSnapshot(rules), saved, "Building source choices leaves saved rules intact");
});

test("deleted chance sources stay identifiable as unavailable without leaking identifiers", () => {
  const { getChanceGroupOptions } = loadTypeScript("src/components/rule-builder/probability-sources.ts");
  const options = getChanceGroupOptions(probabilityRules(), "rgid-deleted");
  const selected = options.find((option) => option.value === "rgid-deleted");
  assert.deepEqual(plainSnapshot(selected), {
    value: "rgid-deleted", label: "Unavailable chance group", disabled: true,
  });
  assert.equal(options.filter((option) => option.value === "rgid-deleted").length, 1);
  assert.ok(options.every((option) => !option.label.includes("rgid-")));
  assert.deepEqual(plainSnapshot(getChanceGroupOptions([], "rgid-deleted")), [plainSnapshot(selected)]);
  assert.deepEqual(plainSnapshot(getChanceGroupOptions([])), []);
});

test("legacy probability conditions display Any source without changing saved values", async () => {
  const catalog = await paletteCatalogPromise;
  const condition = probabilityCondition({ status: { value: "failed" } });
  const inspector = probabilityInspector(condition, probabilityRules(), catalog);
  assert.equal(inspector.fieldSelect("status").props.value, "failed");
  assert.equal(inspector.fieldSelect("source").props.value, "any");
  assert.equal(inspector.groupSelect, undefined);
  assert.deepEqual(inspector.updates, [], "Displaying a legacy rule creates no hidden edits");
  assert.deepEqual(condition.params, { status: { value: "failed" } });
});

test("probability source and group edits preserve status and unrelated saved parameters", async () => {
  const catalog = await paletteCatalogPromise;
  const condition = probabilityCondition({
    status: { value: "failed" }, source: { value: "chance_group" },
    group_id: { value: "rgid-original" }, unrelated: { value: "saved" },
  });
  const inspector = probabilityInspector(condition, probabilityRules(), catalog);
  assert.equal(inspector.groupSelect.props.value, "rgid-original");
  assert.deepEqual(inspector.groupOptions.map((option) => [option.props.value, option.props.children]), [
    ["rgid-original", "Rule 1 · Chance 1 (1 in 5)"],
    ["rgid-secondary", "Rule 1 · Chance 2 (2 in 7)"],
    ["rgid-chain", "Rule 2 · Chance 1 (1 in 3)"],
  ]);
  inspector.groupSelect.props.onValueChange("rgid-chain");
  const groupUpdate = inspector.updates.at(-1);
  assert.equal(groupUpdate.ruleId, "result-rule");
  assert.equal(groupUpdate.conditionId, condition.id);
  assert.equal(groupUpdate.update.params.group_id.value, "rgid-chain");
  assert.equal(groupUpdate.update.params.source.value, "chance_group");
  assert.equal(groupUpdate.update.params.status.value, "failed");
  assert.equal(groupUpdate.update.params.unrelated.value, "saved");
  inspector.fieldSelect("source").props.onValueChange("any");
  const sourceUpdate = inspector.updates.at(-1).update;
  assert.equal(sourceUpdate.params.source.value, "any");
  assert.equal(sourceUpdate.params.status.value, "failed");
  assert.equal(sourceUpdate.params.unrelated.value, "saved");
  const anyInspector = probabilityInspector({ ...condition, ...sourceUpdate }, probabilityRules(), catalog);
  assert.equal(anyInspector.groupSelect, undefined);
});

test("a missing chance selection requires an explicit choice instead of choosing the first source", async () => {
  const catalog = await paletteCatalogPromise;
  const condition = probabilityCondition({
    status: { value: "failed" }, source: { value: "chance_group" },
  });
  const inspector = probabilityInspector(condition, probabilityRules(), catalog);
  assert.equal(inspector.groupSelect.props.value, undefined);
  assert.match(inspector.groupPlaceholder.props.placeholder, /select|choose/i);
  assert.deepEqual(inspector.updates, []);
  assert.equal(condition.params.group_id, undefined);
  const empty = probabilityInspector(condition, [], catalog);
  assert.equal(empty.groupSelect.props.value, undefined);
  assert.equal(empty.groupOptions.length, 1);
  assert.equal(empty.groupOptions[0].props.disabled, true);
  assert.equal(empty.groupOptions[0].props.children, "No chance groups available");
  assert.deepEqual(empty.updates, []);
});

test("an unavailable saved chance source stays selected and never silently switches", async () => {
  const catalog = await paletteCatalogPromise;
  const condition = probabilityCondition({
    status: { value: "failed" }, source: { value: "chance_group" },
    group_id: { value: "rgid-deleted" },
  });
  const inspector = probabilityInspector(condition, probabilityRules(), catalog);
  assert.equal(inspector.groupSelect.props.value, "rgid-deleted");
  const selected = inspector.groupOptions.find((option) => option.props.value === "rgid-deleted");
  assert.equal(selected.props.children, "Unavailable chance group");
  assert.equal(selected.props.disabled, true);
  assert.deepEqual(inspector.updates, []);
  assert.ok(inspector.groupOptions.every((option) => !option.props.children.includes("rgid-")));
  assert.equal(condition.params.group_id.value, "rgid-deleted");
});

test("chance source selections and group identities survive the generated mod request", () => {
  const { createCliItemRequest, serializeCliItemRequest } = loadTypeScript(
    "src/lib/export/cli-item-generation.ts",
  );
  const rules = probabilityRules();
  rules[1].conditionGroups = [{ operator: "and", conditions: [probabilityCondition({
    source: { value: "chance_group", valueType: "text", label: "A chance group in this Joker" },
    group_id: { value: "rgid-original", valueType: "text" },
    status: { value: "failed", valueType: "text" },
  })] }];
  const exportedRules = JSON.parse(serializeCliItemRequest(createCliItemRequest({ rules }))).itemData.rules;
  const condition = exportedRules[1].conditionGroups[0].conditions[0];
  assert.deepEqual(condition.params, {
    source: { value: "chance_group", valueType: "text" },
    group_id: { value: "rgid-original", valueType: "text" },
    status: { value: "failed", valueType: "text" },
  });
  assert.equal(exportedRules[0].randomGroups[0].id, condition.params.group_id.value);
  assert.equal(exportedRules[1].randomGroups[0].id, "rgid-chain");
});
