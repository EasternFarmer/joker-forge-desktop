import type { Condition, Rule, SelectedItem } from "./types";

export interface RuleCardMemoProps {
  rule: Rule;
  ruleIndex: number;
  selectedItem: SelectedItem;
  isRuleSelected: boolean;
  selectedRuleCount: number;
  item: unknown;
  itemType: string;
  scale: number;
  isPaletteDragging: boolean;
  generateConditionTitle: (condition: Condition) => string;
  onPreviewBlockCode?: (target: {
    type: "trigger" | "condition" | "effect";
    ruleId: string;
    itemId?: string;
    groupId?: string;
  }) => void;
  onMoveSelectedRulesByDelta: (deltaX: number, deltaY: number) => void;
  onFinalizeMultiRuleDrag: () => void;
}

const usesChanceGroupLabels = (rule: Rule): boolean => rule.conditionGroups.some((group) =>
  group.conditions.some((condition) => condition.type === "probability_succeeded"
    && condition.params.source?.value === "chance_group"));

export function areRuleCardPropsEqual(prev: RuleCardMemoProps, next: RuleCardMemoProps): boolean {
  if (prev.rule !== next.rule) return false;
  if (prev.ruleIndex !== next.ruleIndex) return false;
  if (prev.isRuleSelected !== next.isRuleSelected) return false;
  if (prev.selectedRuleCount !== next.selectedRuleCount) return false;
  if (prev.item !== next.item) return false;
  if (prev.itemType !== next.itemType) return false;
  if (prev.scale !== next.scale) return false;
  if (prev.isPaletteDragging !== next.isPaletteDragging) return false;
  if (prev.onPreviewBlockCode !== next.onPreviewBlockCode) return false;
  if (prev.onMoveSelectedRulesByDelta !== next.onMoveSelectedRulesByDelta) return false;
  if (prev.onFinalizeMultiRuleDrag !== next.onFinalizeMultiRuleDrag) return false;
  if (prev.generateConditionTitle !== next.generateConditionTitle && usesChanceGroupLabels(next.rule)) return false;

  const prevSelectedAffectsRule = prev.selectedItem?.ruleId === prev.rule.id;
  const nextSelectedAffectsRule = next.selectedItem?.ruleId === next.rule.id;
  if (prevSelectedAffectsRule !== nextSelectedAffectsRule) return false;
  if (prevSelectedAffectsRule && nextSelectedAffectsRule) {
    if (prev.selectedItem?.type !== next.selectedItem?.type) return false;
    if (prev.selectedItem?.itemId !== next.selectedItem?.itemId) return false;
    if (prev.selectedItem?.groupId !== next.selectedItem?.groupId) return false;
    if (prev.selectedItem?.randomGroupId !== next.selectedItem?.randomGroupId) return false;
    if (prev.selectedItem?.loopGroupId !== next.selectedItem?.loopGroupId) return false;
  }
  return true;
}
