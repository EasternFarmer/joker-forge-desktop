import { useId, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  parseBulkPools,
  type BulkEditableItem,
  type BulkItemEdits,
  type JokerBulkValues,
  type ConsumableBulkValues,
  type PoolEditMode,
} from "@/lib/items/bulk-edit";

interface BulkEditItemsDialogProps {
  kind: "joker" | "consumable";
  items: BulkEditableItem[];
  rarityOptions?: { value: string; label: string }[];
  setOptions?: { value: string; label: string }[];
  poolOptions?: string[];
  modPrefix?: string;
  onApply: (edits: BulkItemEdits) => void;
  onClose: () => void;
}

const commonFlags = [
  { key: "unlocked", label: "Unlocked by default" },
  { key: "discovered", label: "Discovered by default" },
];
const jokerFlags = [
  { key: "appears_in_shop", label: "Appears in shop" },
  { key: "blueprint_compat", label: "Blueprint compatible" },
  { key: "eternal_compat", label: "Eternal compatible" },
  { key: "perishable_compat", label: "Perishable compatible" },
  { key: "force_eternal", label: "Force Eternal" },
  { key: "force_perishable", label: "Force Perishable" },
  { key: "force_rental", label: "Force Rental" },
  { key: "force_negative", label: "Force Negative" },
  { key: "force_foil", label: "Force Foil" },
  { key: "force_holographic", label: "Force Holographic" },
  { key: "force_polychrome", label: "Force Polychrome" },
];
const consumableFlags = [
  { key: "hidden", label: "Hidden" },
  { key: "can_repeat_soul", label: "Can repeat Soul" },
];
const UNCHANGED_OPTION = "__jf_bulk_unchanged__";

function readValue(item: BulkEditableItem, key: string): unknown {
  if (key === "perishable_compat" && item.objectType === "joker") {
    return item.perishable_compat ?? true;
  }
  return (item as unknown as Record<string, unknown>)[key];
}

export function BulkEditItemsDialog({
  kind,
  items,
  rarityOptions = [],
  setOptions = [],
  poolOptions = [],
  modPrefix = "",
  onApply,
  onClose,
}: BulkEditItemsDialogProps) {
  const id = useId();
  const flags = [
    ...commonFlags,
    ...(kind === "joker" ? jokerFlags : consumableFlags),
  ];
  const [values, setValues] = useState<Record<string, string>>({});
  const [poolMode, setPoolMode] = useState<PoolEditMode | "">("");
  const [poolInput, setPoolInput] = useState("");
  const automaticPool = modPrefix.trim() ? `${modPrefix.trim()}_jokers` : "";
  const pools = parseBulkPools(poolInput);
  const effectivePoolMode = poolMode || (poolInput.trim() ? "add" : "");
  const hasPoolChange =
    effectivePoolMode === "clear" || Boolean(poolInput.trim());
  const changedValues = Object.entries(values).filter(
    ([, value]) => value.trim() !== "",
  );
  const hasChanges =
    changedValues.length > 0 || (kind === "joker" && hasPoolChange);
  const costInvalid =
    Boolean(values.cost?.trim()) &&
    (!Number.isInteger(Number(values.cost)) ||
      Number(values.cost) < 0 ||
      Number(values.cost) > 2147483647);
  const poolsInvalid =
    kind === "joker" &&
    Boolean(poolInput.trim()) &&
    effectivePoolMode !== "clear" &&
    (pools.length === 0 ||
      pools.some((pool) => pool.toLowerCase() === automaticPool.toLowerCase()));
  const invalid = costInvalid || poolsInvalid;

  const changeValue = (key: string, value: string) =>
    setValues((previous) => ({ ...previous, [key]: value }));
  const mixed = (key: string) =>
    items.length > 1 &&
    items.some((item) => {
      const value = readValue(item, key);
      const first = readValue(items[0], key);
      if (Array.isArray(value) && Array.isArray(first)) {
        return (
          value.length !== first.length ||
          value.some((entry) => !first.includes(entry))
        );
      }
      return value !== first;
    });
  const fieldLabel = (key: string, label: string) => (
    <div className="flex items-center justify-between gap-3">
      <Label htmlFor={`${id}-${key}`}>{label}</Label>
      {mixed(key) && (
        <span className="text-xs text-muted-foreground">Mixed values</span>
      )}
    </div>
  );
  const selectField = (
    key: string,
    label: string,
    options: { value: string; label: string }[],
  ) => {
    const choices = [...options];
    if (key === "rarity" || key === "set") {
      for (const item of items) {
        const existing = String(readValue(item, key) ?? "");
        if (existing && !choices.some((option) => option.value === existing)) {
          choices.push({ value: existing, label: existing });
        }
      }
    }
    return (
      <div className="space-y-2">
        {fieldLabel(key, label)}
        <Select
          value={values[key] || ""}
          onValueChange={(value) =>
            changeValue(key, value === UNCHANGED_OPTION ? "" : value)
          }
        >
          <SelectTrigger
            id={`${id}-${key}`}
            className="w-full"
            aria-label={label}
          >
            <SelectValue placeholder="Leave unchanged" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={UNCHANGED_OPTION}>Leave unchanged</SelectItem>
            {choices.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
    );
  };
  const apply = () => {
    if (invalid || !hasChanges || items.length === 0) return;
    const changes: Record<string, string | number | boolean> = {};
    for (const [key, value] of changedValues) {
      changes[key] =
        key === "cost"
          ? Number(value)
          : key === "rarity"
            ? ["1", "2", "3", "4"].includes(value)
              ? Number(value)
              : value
            : key === "set"
              ? value
              : value === "true";
    }
    if (kind === "joker") {
      onApply({
        kind,
        values: changes as JokerBulkValues,
        ...(hasPoolChange && effectivePoolMode
          ? { pools: { mode: effectivePoolMode, values: pools } }
          : {}),
      });
    } else {
      onApply({ kind, values: changes as ConsumableBulkValues });
    }
  };

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent className="sm:max-w-2xl max-h-[85vh] flex flex-col">
        <DialogHeader>
          <DialogTitle>
            Edit {items.length} selected {kind === "joker" ? "Jokers" : "Consumables"}
          </DialogTitle>
          <DialogDescription>
            Enter only the values you want to change. Blank fields keep each
            card's existing value.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-5 overflow-y-auto min-h-0 px-1 pb-1">
          {kind === "joker" && (
            <div className="space-y-3 rounded-lg border border-border p-4">
              {fieldLabel("pools", "Custom pools")}
              <Select
                value={effectivePoolMode}
                onValueChange={(value) => {
                  if (value === UNCHANGED_OPTION) {
                    setPoolMode("");
                    setPoolInput("");
                  } else {
                    setPoolMode(value as PoolEditMode);
                  }
                }}
              >
                <SelectTrigger
                  id={`${id}-pools`}
                  className="w-full"
                  aria-label="Pool operation"
                >
                  <SelectValue placeholder="Leave unchanged" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={UNCHANGED_OPTION}>Leave unchanged</SelectItem>
                  <SelectItem value="add">Add to pools</SelectItem>
                  <SelectItem value="remove">Remove from pools</SelectItem>
                  <SelectItem value="replace">Replace custom pools</SelectItem>
                  <SelectItem value="clear">Clear custom pools</SelectItem>
                </SelectContent>
              </Select>
              {effectivePoolMode !== "clear" && (
                <>
                  <Label htmlFor={`${id}-pool-input`}>Pools (comma separated)</Label>
                  <Input
                    id={`${id}-pool-input`}
                    value={poolInput}
                    onChange={(event) => setPoolInput(event.target.value)}
                    placeholder="pool_one, pool_two"
                    list={`${id}-pool-options`}
                    aria-invalid={poolsInvalid}
                  />
                  <datalist id={`${id}-pool-options`}>
                    {poolOptions
                      .filter((pool) => pool !== automaticPool)
                      .map((pool) => <option key={pool} value={pool} />)}
                  </datalist>
                </>
              )}
              <p className="text-xs text-muted-foreground">
                The mod prefix is added during export. The automatic Joker pool
                stays managed by the app.
              </p>
              {poolsInvalid && (
                <p role="alert" className="text-sm text-destructive">
                  Enter at least one custom pool. The automatic pool cannot be
                  edited here.
                </p>
              )}
            </div>
          )}
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div className="space-y-2">
              {fieldLabel("cost", "Cost ($)")}
              <Input
                id={`${id}-cost`}
                type="number"
                min={0}
                max={2147483647}
                step={1}
                aria-label="Cost ($)"
                value={values.cost || ""}
                placeholder="Leave unchanged"
                aria-invalid={costInvalid}
                onChange={(event) => changeValue("cost", event.target.value)}
              />
              {costInvalid && (
                <p role="alert" className="text-sm text-destructive">
                  Enter a whole-number cost between 0 and 2,147,483,647.
                </p>
              )}
            </div>
            {kind === "joker"
              ? selectField("rarity", "Rarity", rarityOptions)
              : selectField("set", "Consumable set", setOptions)}
            {flags.map((field) => (
              <div key={field.key}>
                {selectField(field.key, field.label, [
                  { value: "true", label: "Yes" },
                  { value: "false", label: "No" },
                ])}
              </div>
            ))}
          </div>
        </div>
        <DialogFooter className="pt-2 border-t border-border">
          <Button variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button
            onClick={apply}
            disabled={invalid || !hasChanges || items.length === 0}
          >
            Apply to {items.length} cards
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
