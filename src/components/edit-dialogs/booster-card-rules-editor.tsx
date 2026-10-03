import { useId, useRef } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Combobox,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
} from "@/components/ui/combobox";
import { Plus, Trash } from "@phosphor-icons/react";
import type { BoosterCardRule, BoosterType } from "@/lib/core/types";
import {
  JOKERS,
  PLANET_CARDS,
  RANKS,
  SPECTRAL_CARDS,
  SUITS,
  TAROT_CARDS,
  VANILLA_EDITIONS,
  VANILLA_ENHANCEMENTS,
  VANILLA_SEALS,
  VANILLA_VOUCHERS,
  getAllConsumableSets,
  getAllRarities,
} from "@/lib/balatro/balatro-utils";
import { useProjectData } from "@/lib/services/storage";

type Option = { value: string; label: string };

function ContentSelect({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: string;
  options: Option[];
  onChange: (value: string) => void;
}) {
  const id = useId();
  const portalContainer = useRef<HTMLElement | null>(null);
  const visibleOptions = options.some((option) => option.value === value)
    ? options
    : [...options, { value, label: value }];
  return (
    <div
      className="space-y-2"
      ref={(element) => {
        portalContainer.current = element?.closest<HTMLElement>("[data-slot=dialog-content]") ?? null;
      }}
    >
      <Label htmlFor={id}>{label}</Label>
      <Combobox
        items={visibleOptions}
        value={visibleOptions.find((option) => option.value === value) ?? null}
        isItemEqualToValue={(option, selected) => option.value === selected.value}
        onValueChange={(option) => {
          if (option) onChange(option.value);
        }}
        autoHighlight
      >
        <ComboboxInput id={id} className="w-full" placeholder={`Search ${label.toLowerCase()}...`} />
        <ComboboxContent container={portalContainer}>
          <ComboboxEmpty>No matching options.</ComboboxEmpty>
          <ComboboxList>
            {(option: Option) => (
              <ComboboxItem key={option.value} value={option}>
                {option.label}
              </ComboboxItem>
            )}
          </ComboboxList>
        </ComboboxContent>
      </Combobox>
    </div>
  );
}

const RANDOM_OPTION = { value: "any", label: "Random" };
const NONE_OPTION = { value: "none", label: "None" };

export function getDefaultBoosterDescription(type: BoosterType): string {
  const cards = type === "joker" ? "Jokers"
    : type === "consumable" ? "Consumables"
      : type === "voucher" ? "Vouchers" : "Playing Cards";
  return `Choose {C:attention}#1#{} of up to\n{C:attention}#2#{} ${cards}`;
}

export function BoosterCardRulesEditor({
  type,
  rules,
  onChange,
}: {
  type: BoosterType;
  rules: BoosterCardRule[];
  onChange: (rules: BoosterCardRule[]) => void;
}) {
  const { data } = useProjectData();
  const prefix = data.metadata.prefix;
  const keyFor = (category: string, key: string) => `${category}_${prefix}_${key}`;
  const jokerOptions: Option[] = [
    ...(data.metadata.disable_vanilla ? [] : JOKERS.map((item) => ({
      value: item.key,
      label: item.label,
    }))),
    ...data.jokers.map((item) => ({
      value: keyFor("j", item.objectKey),
      label: item.name,
    })),
  ];
  const voucherOptions: Option[] = [
    ...(data.metadata.disable_vanilla ? [] : VANILLA_VOUCHERS.map((item) => ({
      value: item.key,
      label: item.label,
    }))),
    ...data.vouchers.map((item) => ({
      value: keyFor("v", item.objectKey),
      label: item.name,
    })),
  ];
  const editionOptions: Option[] = [
    { value: "any", label: "Normal Edition Chance" },
    { value: "random", label: "Random Edition (Guaranteed)" },
    { value: "none", label: "No Edition" },
    ...VANILLA_EDITIONS.map((item) => ({ value: item.key, label: item.label })),
    ...data.editions.map((item) => ({ value: keyFor("e", item.objectKey), label: item.name })),
  ];
  const enhancementOptions: Option[] = [
    { value: "any", label: "Normal Enhancement Chance" },
    { value: "random", label: "Random Enhancement (Guaranteed)" },
    NONE_OPTION,
    ...VANILLA_ENHANCEMENTS.map((item) => ({ value: item.key, label: item.label })),
    ...data.enhancements.map((item) => ({ value: keyFor("m", item.objectKey), label: item.name })),
  ];
  const sealOptions: Option[] = [
    NONE_OPTION,
    ...VANILLA_SEALS.map((item) => ({ value: item.key, label: item.label })),
    ...data.seals.map((item) => ({ value: `${prefix}_${item.objectKey}`, label: item.name })),
  ];
  const pools = Array.from(new Set(data.jokers.flatMap((item) => item.pools || [])))
    .filter(Boolean)
    .map((pool) => ({ value: pool, label: pool }));
  const totalWeight = rules.reduce(
    (total, rule) => total + (
      Number.isFinite(rule.weight ?? 1) && (rule.weight ?? 1) > 0
        ? (rule.weight ?? 1)
        : 0
    ),
    0,
  );
  const updateRule = (index: number, updates: Partial<BoosterCardRule>) =>
    onChange(rules.map((rule, ruleIndex) =>
      ruleIndex === index ? { ...rule, ...updates } : rule,
    ));

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">
        Each card in the pack randomly chooses one of these options. Higher weights make an option more likely. Set a weight to zero to disable that option.
      </p>
      {rules.length === 0 && (
        <div className="rounded-md border border-dashed p-4 text-sm text-muted-foreground">
          This pack contains random {type === "joker" ? "Jokers"
            : type === "consumable" ? "Tarot cards"
              : type === "voucher" ? "Vouchers" : "playing cards"}.
          {" "}Add an option to customize its contents.
        </div>
      )}
      {rules.map((rule, index) => {
        const set = rule.set || "Tarot";
        const vanillaConsumables = set === "Tarot" ? TAROT_CARDS
          : set === "Planet" ? PLANET_CARDS
            : set === "Spectral" ? SPECTRAL_CARDS : [];
        const consumableOptions: Option[] = [
          ...(data.metadata.disable_vanilla ? [] : vanillaConsumables.map((item) => ({
            value: item.key,
            label: item.label,
          }))),
          ...data.consumables.filter((item) => item.set === set).map((item) => ({
            value: keyFor("c", item.objectKey),
            label: item.name,
          })),
        ];
        const specificOptions = type === "joker" ? jokerOptions
          : type === "voucher" ? voucherOptions : consumableOptions;
        const percent = totalWeight > 0 && Number.isFinite(rule.weight ?? 1)
          ? Math.max(0, rule.weight ?? 1) / totalWeight * 100
          : 0;
        const invalidWeight = !Number.isFinite(rule.weight ?? 1) || (rule.weight ?? 1) < 0;
        return (
          <div key={index} className="space-y-4 rounded-lg border p-4">
            <div className="flex items-center justify-between gap-3">
              <span className="text-sm font-semibold">Content Option {index + 1}</span>
              <Button
                type="button"
                variant="ghost"
                size="icon"
                aria-label={`Remove content option ${index + 1}`}
                onClick={() => onChange(rules.filter((_, ruleIndex) => ruleIndex !== index))}
              >
                <Trash className="size-4" />
              </Button>
            </div>
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor={`booster-weight-${index}`}>Weight ({percent.toFixed(1)}% per card)</Label>
                <Input
                  id={`booster-weight-${index}`}
                  type="number"
                  min={0}
                  step={0.1}
                  value={rule.weight ?? 1}
                  aria-invalid={invalidWeight}
                  onChange={(event) => updateRule(index, { weight: Number(event.target.value) })}
                />
                {invalidWeight && (
                  <p className="text-sm text-destructive">Enter a weight of zero or greater.</p>
                )}
              </div>
              {type === "consumable" && (
                <ContentSelect
                  label="Consumable Type"
                  value={set}
                  options={getAllConsumableSets(data.consumableSets).map((item) => ({
                    value: item.value,
                    label: item.label,
                  }))}
                  onChange={(value) => updateRule(index, {
                    set: value,
                    specific_key: undefined,
                    specific_type: undefined,
                  })}
                />
              )}
              {type !== "playing_card" && (
                <ContentSelect
                  label={type === "joker" ? "Joker" : type === "voucher" ? "Voucher" : "Consumable"}
                  value={rule.specific_key || "any"}
                  options={[RANDOM_OPTION, ...specificOptions]}
                  onChange={(value) => updateRule(index, {
                    specific_key: value === "any" ? undefined : value,
                    specific_type: value === "any" ? undefined : type,
                    ...(type === "consumable" ? { set } : {}),
                    pool: undefined,
                    rarity: undefined,
                  })}
                />
              )}
              {type === "joker" && !rule.specific_key && (
                <>
                  <ContentSelect
                    label="Rarity"
                    value={rule.rarity || "any"}
                    options={[
                      { value: "any", label: "Any Rarity" },
                      ...getAllRarities(data.rarities).map((item) => ({
                        value: String(item.value),
                        label: item.label,
                      })),
                    ]}
                    onChange={(value) => updateRule(index, { rarity: value })}
                  />
                  <ContentSelect
                    label="Joker Pool"
                    value={rule.pool || "any"}
                    options={[{ value: "any", label: "All Jokers" }, ...pools]}
                    onChange={(value) => updateRule(index, { pool: value === "any" ? undefined : value })}
                  />
                </>
              )}
              {type === "playing_card" && (
                <>
                  <ContentSelect
                    label="Suit"
                    value={rule.suit || "any"}
                    options={[RANDOM_OPTION, ...SUITS]}
                    onChange={(value) => updateRule(index, { suit: value })}
                  />
                  <ContentSelect
                    label="Rank"
                    value={rule.rank || "any"}
                    options={[RANDOM_OPTION, ...RANKS.map((rank) => ({
                      value: rank.id >= 11 ? rank.label : rank.value,
                      label: rank.label,
                    }))]}
                    onChange={(value) => updateRule(index, { rank: value })}
                  />
                  <ContentSelect
                    label="Enhancement"
                    value={rule.enhancement || "any"}
                    options={enhancementOptions}
                    onChange={(value) => updateRule(index, { enhancement: value })}
                  />
                  <ContentSelect
                    label="Seal"
                    value={rule.seal || "none"}
                    options={sealOptions}
                    onChange={(value) => updateRule(index, { seal: value })}
                  />
                </>
              )}
              {type !== "voucher" && (
                <ContentSelect
                  label="Edition"
                  value={rule.edition || "any"}
                  options={editionOptions}
                  onChange={(value) => updateRule(index, { edition: value })}
                />
              )}
            </div>
          </div>
        );
      })}
      {rules.length > 0 && totalWeight === 0 && (
        <p className="text-sm text-destructive">At least one content option needs a weight greater than zero.</p>
      )}
      <Button
        type="button"
        variant="outline"
        onClick={() => onChange([...rules, { weight: 1, ...(type === "consumable" ? { set: "Tarot" } : {}) }])}
      >
        <Plus className="size-4" /> Add Content Option
      </Button>
    </div>
  );
}
