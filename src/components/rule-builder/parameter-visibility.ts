import type { ConditionParameter, EffectParameter } from "./types";

type Parameter = Pick<
  ConditionParameter | EffectParameter,
  "id" | "showWhen" | "default"
>;
type ParameterValues = Record<string, { value: unknown; valueType?: string }>;

/** Share visibility rules between the inspector list and the field itself. */
export function isParameterVisible(
  parameter: Parameter,
  definitions: readonly Parameter[],
  parentValues: ParameterValues,
): boolean {
  let current: Parameter | undefined = parameter;
  const visited = new Set<string>();

  while (current?.showWhen) {
    if (visited.has(current.id)) return false;
    visited.add(current.id);

    const { parameter: parentId, values }: NonNullable<Parameter["showWhen"]> =
      current.showWhen;
    const parentDefinition: Parameter | undefined = definitions.find(
      (definition) => definition.id === parentId,
    );
    const parentValue = parentValues[parentId]?.value ?? parentDefinition?.default;

    if (
      current.id === "specific_card" &&
      (parentId === "set" || parentId === "consumable_type")
    ) {
      // SMODS set names use Tarot/Planet, while older guards use lowercase.
      // Every concrete custom consumable set also supports specific cards.
      if (typeof parentValue !== "string") return false;
      const selectedSet = parentValue.trim().toLowerCase();
      if (!selectedSet || ["random", "any", "keyvar"].includes(selectedSet)) {
        return false;
      }
    } else if (
      Array.isArray(parentValue) &&
      typeof parentValue[0] === "boolean"
    ) {
      if (!values.some((value) => parentValue[Number(value)])) return false;
    } else if (typeof parentValue === "string") {
      if (!values.includes(parentValue)) return false;
    } else {
      return false;
    }

    current = parentDefinition;
  }
  return true;
}
