import type { ConsumableData, JokerData } from "@/lib/core/types";

export type BulkEditableItem = JokerData | ConsumableData;
export type JokerBulkValues = Partial<
  Pick<
    JokerData,
    | "cost"
    | "rarity"
    | "unlocked"
    | "discovered"
    | "appears_in_shop"
    | "blueprint_compat"
    | "eternal_compat"
    | "perishable_compat"
    | "force_eternal"
    | "force_perishable"
    | "force_rental"
    | "force_negative"
    | "force_foil"
    | "force_holographic"
    | "force_polychrome"
  >
>;
export type ConsumableBulkValues = Partial<
  Pick<
    ConsumableData,
    "cost" | "set" | "unlocked" | "discovered" | "hidden" | "can_repeat_soul"
  >
>;
export type PoolEditMode = "add" | "remove" | "replace" | "clear";
export type BulkItemEdits =
  | {
      kind: "joker";
      values: JokerBulkValues;
      pools?: { mode: PoolEditMode; values: string[] };
    }
  | { kind: "consumable"; values: ConsumableBulkValues };

export function parseBulkPools(value: string): string[] {
  return [
    ...new Set(
      value
        .split(",")
        .map((pool) => pool.trim())
        .filter(Boolean),
    ),
  ];
}

export function applyBulkItemEdits<T extends BulkEditableItem>(
  items: T[],
  selectedIds: ReadonlySet<string>,
  edits: BulkItemEdits,
): T[] {
  return items.map((item) => {
    if (!selectedIds.has(item.id) || item.objectType !== edits.kind) return item;
    const updated = { ...item, ...edits.values } as T;
    if (item.objectType === "joker" && edits.kind === "joker" && edits.pools) {
      const existing = item.pools ?? [];
      const { mode, values } = edits.pools;
      const pools =
        mode === "clear"
          ? []
          : mode === "replace"
            ? values
            : mode === "remove"
              ? existing.filter((pool) => !values.includes(pool))
              : [...existing, ...values];
      (updated as JokerData).pools = [...new Set(pools)];
    }
    return updated;
  });
}

export function duplicateItem<T extends BulkEditableItem>(
  item: T,
  existing: T[],
): T {
  const copy = structuredClone(item);
  const baseKey = `${item.objectKey}_copy`;
  const keys = new Set(existing.map((entry) => entry.objectKey));
  let objectKey = baseKey;
  let suffix = 2;
  while (keys.has(objectKey)) objectKey = `${baseKey}_${suffix++}`;
  return {
    ...copy,
    id: crypto.randomUUID(),
    objectKey,
    name: `${item.name} (Copy)`,
    orderValue: Math.max(0, ...existing.map((entry) => entry.orderValue)) + 1,
  };
}
