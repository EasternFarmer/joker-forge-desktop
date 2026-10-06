import type {
  Condition,
  ConditionParameter,
  Effect,
  EffectParameter,
  GlobalConditionTypeDefinition,
  GlobalEffectTypeDefinition,
  GlobalTriggerDefinition,
  Rule,
} from "../../components/rule-builder/types";
import type { BoundFieldRange, Scalar } from "./live-code-sync";

export interface LiveCodeFieldTarget {
  type: "trigger" | "condition" | "effect" | "randomgroup" | "loopgroup";
  ruleId: string;
  itemId?: string;
  groupId?: string;
  randomGroupId?: string;
  loopGroupId?: string;
  parameterId?: string;
}

export interface LiveCodeFieldLink extends BoundFieldRange {
  label: string;
  fieldLabel: string;
  allowedValues: string;
  description?: string;
  target: LiveCodeFieldTarget;
}

export interface LiveCodeCatalog {
  getCondition: (id: string) => GlobalConditionTypeDefinition | undefined;
  getEffect: (id: string) => GlobalEffectTypeDefinition | undefined;
  getTrigger: (id: string) => GlobalTriggerDefinition | undefined;
}

export const readPathValue = (root: unknown, path: Array<string | number>): unknown =>
  path.reduce<unknown>((value, part) => value && typeof value === "object"
    && !["__proto__", "prototype", "constructor"].includes(String(part))
    && Object.prototype.hasOwnProperty.call(value, part)
    ? (value as Record<string, unknown>)[String(part)]
    : undefined, root);

export const attachFieldIdentities = (ranges: BoundFieldRange[], sourceRules: Rule[]): BoundFieldRange[] =>
  ranges.map((range) => {
    const sourceIds: Record<number, string> = {};
    let current: unknown = { rules: sourceRules };
    range.sourcePath.forEach((part, index) => {
      current = readPathValue(current, [part]);
      if (typeof part === "number") {
        const id = (current as { id?: unknown } | undefined)?.id;
        if (typeof id === "string" && id) sourceIds[index] = id;
      }
    });
    return { ...range, sourceIds };
  });

export const resolveLinkedRulePath = (
  range: BoundFieldRange,
  sourceRules: Rule[],
  currentRules: Rule[],
): Array<string | number> | null => {
  if (range.sourcePath[0] !== "rules") return null;
  let source: unknown = { rules: sourceRules };
  let current: unknown = { rules: currentRules };
  const resolved: Array<string | number> = [];
  for (let index = 0; index < range.sourcePath.length; index += 1) {
    const part = range.sourcePath[index];
    if (typeof part === "number") {
      if (!Array.isArray(current)) return null;
      const original = Array.isArray(source) ? source[part] : undefined;
      const id = range.sourceIds?.[index] ?? (original as { id?: string } | undefined)?.id;
      if (!id) return null;
      const currentIndex = current.findIndex((entry) => entry?.id === id);
      if (currentIndex < 0) return null;
      resolved.push(currentIndex);
      current = current[currentIndex];
      source = original;
    } else {
      if (["__proto__", "prototype", "constructor"].includes(part)
        || !current || typeof current !== "object"
        || !Object.prototype.hasOwnProperty.call(current, part)) return null;
      resolved.push(part);
      current = readPathValue(current, [part]);
      source = readPathValue(source, [part]);
    }
  }
  return resolved;
};

type Parameter = ConditionParameter | EffectParameter;

const resolveParameter = (path: Array<string | number>, rules: Rule[], catalog: LiveCodeCatalog) => {
  const paramsIndex = path.lastIndexOf("params");
  if (paramsIndex < 0 || typeof path[paramsIndex + 1] !== "string") return undefined;
  const owner = readPathValue({ rules }, path.slice(0, paramsIndex)) as Condition | Effect | undefined;
  if (!owner?.type) return undefined;
  const condition = path.includes("conditions");
  const definition = condition ? catalog.getCondition(owner.type) : catalog.getEffect(owner.type);
  const parameter = definition?.params.find((entry) => entry.id === path[paramsIndex + 1]);
  return parameter && definition ? { owner, definition, parameter } : undefined;
};

const parameterOptions = (parameter: Parameter, owner: Condition | Effect) => {
  if (Array.isArray(parameter.options)) return parameter.options;
  if (typeof parameter.options === "function") return parameter.options(owner.params);
  return undefined;
};

export const isLinkedFieldValueAllowed = (
  path: Array<string | number>, rules: Rule[], value: Scalar, catalog: LiveCodeCatalog,
): boolean => {
  if (!path.includes("params")) return true;
  const resolved = resolveParameter(path, rules, catalog);
  if (!resolved) return false;
  const { parameter, owner } = resolved;
  if (parameter.type === "number" || parameter.type === "range") {
    return typeof value === "number" && Number.isFinite(value)
      && (parameter.min === undefined || value >= parameter.min)
      && (parameter.max === undefined || value <= parameter.max);
  }
  if (parameter.type === "text") return typeof value === "string";
  if (parameter.type === "checkbox") return typeof value === "boolean";
  if (parameter.type === "select") {
    const options = parameterOptions(parameter, owner);
    return options?.some((option) => Object.is(option.value, value)) ?? true;
  }
  return true;
};

const allowedValuesLabel = (parameter: Parameter | undefined, valueType: BoundFieldRange["valueType"], owner?: Condition | Effect) => {
  if (parameter?.type === "select" && owner) {
    const options = parameterOptions(parameter, owner);
    if (options?.length) return options.map((option) =>
      option.label === option.value ? JSON.stringify(option.value)
        : `${JSON.stringify(option.value)} (${option.label})`).join(", ");
    return "Choose an available value in the inspector";
  }
  if (valueType === "boolean") return "true or false";
  if (valueType === "string") return "Quoted Lua string";
  const bounds: string[] = [];
  if (parameter?.min !== undefined) bounds.push(`minimum ${parameter.min}`);
  if (parameter?.max !== undefined) bounds.push(`maximum ${parameter.max}`);
  return bounds.length ? `Number (${bounds.join(", ")})` : "Finite number";
};

const titleCase = (value: string): string => value.replace(/_/g, " ").replace(/\b\w/g, (letter) => letter.toUpperCase());

export function buildLiveCodeFieldLinks(
  ranges: BoundFieldRange[], sourceRules: Rule[], currentRules: Rule[], catalog: LiveCodeCatalog, itemType = "joker",
): LiveCodeFieldLink[] {
  return ranges.flatMap((range) => {
    const path = resolveLinkedRulePath(range, sourceRules, currentRules);
    if (!path || typeof path[1] !== "number") return [];
    const rule = currentRules[path[1]];
    if (!rule) return [];
    const target: LiveCodeFieldTarget = { type: "trigger", ruleId: rule.id };
    const labels = [`Rule ${path[1] + 1}`];
    let fieldLabel = titleCase(String(path[path.length - 1]));
    const parameterInfo = resolveParameter(path, currentRules, catalog);
    if (path[2] === "conditionGroups" && typeof path[3] === "number") {
      target.groupId = rule.conditionGroups[path[3]]?.id;
      if (path[4] === "conditions" && typeof path[5] === "number") {
        target.type = "condition";
        target.itemId = rule.conditionGroups[path[3]]?.conditions[path[5]]?.id;
      }
    }
    if (path[2] === "randomGroups" && typeof path[3] === "number") {
      target.randomGroupId = rule.randomGroups[path[3]]?.id;
      target.type = "randomgroup";
      labels.push(`Chance Group ${path[3] + 1}`);
      fieldLabel = path[4] === "chance_numerator" ? "Numerator"
        : path[4] === "chance_denominator" ? "Denominator" : titleCase(String(path[4]));
      target.parameterId = String(path[4]);
      if (path[4] === "effects" && typeof path[5] === "number") {
        target.type = "effect";
        target.itemId = rule.randomGroups[path[3]]?.effects[path[5]]?.id;
      }
    }
    if (path[2] === "loops" && typeof path[3] === "number") {
      target.loopGroupId = rule.loops[path[3]]?.id;
      target.type = "loopgroup";
      labels.push(`Loop ${path[3] + 1}`);
      fieldLabel = path[4] === "repetitions" ? "Repetitions" : titleCase(String(path[4]));
      target.parameterId = String(path[4]);
      if (path[4] === "effects" && typeof path[5] === "number") {
        target.type = "effect";
        target.itemId = rule.loops[path[3]]?.effects[path[5]]?.id;
      }
    }
    if (path[2] === "effects" && typeof path[3] === "number") {
      target.type = "effect";
      target.itemId = rule.effects[path[3]]?.id;
    }
    if (parameterInfo) {
      labels.push(parameterInfo.definition.label);
      fieldLabel = parameterInfo.parameter.label;
      target.parameterId = parameterInfo.parameter.id;
    } else if (target.type === "condition" || target.type === "effect") {
      return [];
    } else if (target.type === "trigger") {
      const trigger = catalog.getTrigger(rule.trigger);
      labels.push(trigger?.label[itemType] ?? trigger?.label.joker ?? titleCase(rule.trigger));
      fieldLabel = titleCase(String(path[2]));
      target.parameterId = String(path[2]);
    }
    return [{ ...range, label: [...labels, fieldLabel].join(" → "), fieldLabel,
      allowedValues: allowedValuesLabel(parameterInfo?.parameter, range.valueType, parameterInfo?.owner),
      description: parameterInfo?.parameter.description, target }];
  });
}
