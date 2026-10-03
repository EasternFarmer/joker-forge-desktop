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
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
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
