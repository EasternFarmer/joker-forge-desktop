import type {
  Condition,
  Effect,
  LoopGroup,
  RandomGroup,
  Rule,
  SelectedItem,
} from "./types";
import type { CodeSegment } from "@/lib/content/code-sections";

export const getSelectedRule = (
  rules: Rule[],
  selectedItem: SelectedItem,
): Rule | null => {
  if (!selectedItem) {
    return null;
  }

  return rules.find((rule) => rule.id === selectedItem.ruleId) || null;
};

export const getSelectedCondition = (
  rules: Rule[],
  selectedItem: SelectedItem,
): Condition | null => {
  if (
    !selectedItem ||
    selectedItem.type !== "condition" ||
    !selectedItem.itemId
  ) {
    return null;
  }

  const rule = getSelectedRule(rules, selectedItem);
  if (!rule) {
    return null;
  }

  for (const group of rule.conditionGroups) {
    const condition = group.conditions.find(
      (c) => c.id === selectedItem.itemId,
    );
    if (condition) {
      return condition;
    }
  }

  return null;
};

export const getSelectedEffect = (
  rules: Rule[],
  selectedItem: SelectedItem,
): Effect | null => {
  if (!selectedItem || selectedItem.type !== "effect" || !selectedItem.itemId) {
    return null;
  }

  const rule = getSelectedRule(rules, selectedItem);
  if (!rule) {
    return null;
  }

  const mainEffect = rule.effects.find((e) => e.id === selectedItem.itemId);
  if (mainEffect) {
    return mainEffect;
  }

  for (const group of rule.randomGroups) {
    const effect = group.effects.find((e) => e.id === selectedItem.itemId);
    if (effect) {
      return effect;
    }
  }

  for (const group of rule.loops) {
    const effect = group.effects.find((e) => e.id === selectedItem.itemId);
    if (effect) {
      return effect;
    }
  }

  return null;
};

export const getSelectedRandomGroup = (
  rules: Rule[],
  selectedItem: SelectedItem,
): RandomGroup | null => {
  if (
    !selectedItem ||
    selectedItem.type !== "randomgroup" ||
    !selectedItem.randomGroupId
  ) {
    return null;
  }

  const rule = getSelectedRule(rules, selectedItem);
  if (!rule) {
    return null;
  }

  return (
    rule.randomGroups.find(
      (group) => group.id === selectedItem.randomGroupId,
    ) || null
  );
};

export const getSelectedLoopGroup = (
  rules: Rule[],
  selectedItem: SelectedItem,
): LoopGroup | null => {
  if (
    !selectedItem ||
    selectedItem.type !== "loopgroup" ||
    !selectedItem.loopGroupId
  ) {
    return null;
  }

  const rule = getSelectedRule(rules, selectedItem);
  if (!rule) {
    return null;
  }

  return (
    rule.loops.find((group) => group.id === selectedItem.loopGroupId) || null
  );
};

export const resolveSelectedItem = (
  rules: Rule[],
  target: NonNullable<SelectedItem>,
): SelectedItem => {
  const rule = rules.find((entry) => entry.id === target.ruleId);
  if (!rule) return null;
  if (target.type === "trigger") return { type: "trigger", ruleId: rule.id };
  if (target.type === "condition" && target.itemId) {
    const group = rule.conditionGroups.find((entry) =>
      entry.conditions.some((condition) => condition.id === target.itemId));
    return group ? { type: "condition", ruleId: rule.id, itemId: target.itemId, groupId: group.id } : null;
  }
  if (target.type === "effect" && target.itemId) {
    if (rule.effects.some((effect) => effect.id === target.itemId)) {
      return { type: "effect", ruleId: rule.id, itemId: target.itemId };
    }
    const randomGroup = rule.randomGroups.find((group) =>
      group.effects.some((effect) => effect.id === target.itemId));
    if (randomGroup) return {
      type: "effect", ruleId: rule.id, itemId: target.itemId, randomGroupId: randomGroup.id,
    };
    const loop = rule.loops.find((group) =>
      group.effects.some((effect) => effect.id === target.itemId));
    return loop ? { type: "effect", ruleId: rule.id, itemId: target.itemId, loopGroupId: loop.id } : null;
  }
  if (target.type === "randomgroup" && rule.randomGroups.some((group) => group.id === target.randomGroupId)) {
    return { type: "randomgroup", ruleId: rule.id, randomGroupId: target.randomGroupId };
  }
  if (target.type === "loopgroup" && rule.loops.some((group) => group.id === target.loopGroupId)) {
    return { type: "loopgroup", ruleId: rule.id, loopGroupId: target.loopGroupId };
  }
  return null;
};

export const getSelectionForCodeSegment = (rules: Rule[], segmentId: string): SelectedItem => {
  for (const rule of rules) {
    if (segmentId === `rule:${rule.id}` || segmentId === `trigger:${rule.id}`) {
      return { type: "trigger", ruleId: rule.id };
    }
    for (const group of rule.conditionGroups) {
      for (const condition of group.conditions) {
        if (segmentId === `condition:${rule.id}:${condition.id}`) {
          return { type: "condition", ruleId: rule.id, itemId: condition.id, groupId: group.id };
        }
      }
    }
    const effects = [
      ...rule.effects,
      ...rule.randomGroups.flatMap((group) => group.effects),
      ...rule.loops.flatMap((group) => group.effects),
    ];
    const effect = effects.find((entry) => segmentId === `effect:${rule.id}:${entry.id}`);
    if (effect) return resolveSelectedItem(rules, { type: "effect", ruleId: rule.id, itemId: effect.id });
  }
  return null;
};

export const retainTrackedCodeSegments = (
  generatedSegments: CodeSegment[],
  trackedSegments: CodeSegment[],
  rules: Rule[],
): CodeSegment[] => {
  const mappedIds = new Set(generatedSegments.map((segment) => segment.id));
  return [...generatedSegments, ...trackedSegments.filter((segment) =>
    !mappedIds.has(segment.id) && getSelectionForCodeSegment(rules, segment.id) !== null)];
};
