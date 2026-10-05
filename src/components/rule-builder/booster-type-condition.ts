import type {
  Condition,
  GlobalConditionTypeDefinition,
  GlobalEffectTypeDefinition,
} from "./types";

const hasBoosterKey = (value: unknown): value is string =>
  typeof value === "string" && value.trim().length > 0;

export function normalizeBoosterTypeConditionParams(
  params: Condition["params"] | undefined,
): Condition["params"] {
  const currentParams = params ?? {};
  const legacyValue = currentParams.value;
  if (
    hasBoosterKey(currentParams.booster_key?.value) ||
    !hasBoosterKey(legacyValue?.value)
  ) {
    return currentParams;
  }

  return { ...currentParams, booster_key: { ...legacyValue } };
}

export function generateBoosterTypeConditionTitle(
  condition: Pick<Condition, "params">,
  definition: Pick<GlobalConditionTypeDefinition | GlobalEffectTypeDefinition, "label" | "params">,
): string {
  const params = normalizeBoosterTypeConditionParams(condition.params);
  const boosterKey = params.booster_key?.value;
  const parameter = definition.params.find((param) => param.id === "booster_key");
  const options = typeof parameter?.options === "function"
    ? parameter.options(params)
    : parameter?.options;
  const boosterLabel = hasBoosterKey(boosterKey)
    ? options?.find((option) => option.value === boosterKey)?.label ?? boosterKey
    : "Choose a booster";
  const operator = params.operator?.value;
  const operatorLabel = operator === "not_equal" || operator === "not_equals"
    ? "≠"
    : "=";

  return `If ${definition.label} ${operatorLabel} ${boosterLabel}`;
}
