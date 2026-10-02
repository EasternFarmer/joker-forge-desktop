import type { BaseGameObject } from "@/lib/core/types";
import type { ProjectData } from "@/lib/services/storage";
import {
  BOSS_BLINDS,
  CUSTOM_SHADERS,
  JOKERS,
  PLANET_CARDS,
  POKER_HANDS,
  RANKS,
  SPECTRAL_CARDS,
  STICKERS,
  SUITS,
  TAGS,
  TAG_TYPES,
  TAROT_CARDS,
  VANILLA_BOOSTERS,
  VANILLA_CONSUMABLE_SETS,
  VANILLA_DECKS,
  VANILLA_EDITIONS,
  VANILLA_ENHANCEMENTS,
  VANILLA_MISCS,
  VANILLA_RARITIES,
  VANILLA_SEALS,
  VANILLA_SHADERS,
  VANILLA_SOUNDS,
  VANILLA_VOUCHERS,
} from "@/lib/balatro/balatro-utils";

export const KEY_REFERENCE_CATEGORIES = [
  "Jokers",
  "Tarot Cards",
  "Planet Cards",
  "Spectral Cards",
  "Consumables",
  "Consumable Sets",
  "Object Types",
  "Decks",
  "Vouchers",
  "Booster Packs",
  "Enhancements",
  "Seals",
  "Editions",
  "Blinds",
  "Tags",
  "Stickers",
  "Sounds",
  "Shaders",
  "Rarities",
  "Suits",
  "Ranks",
  "Poker Hands",
] as const;

export type KeyReferenceCategory = (typeof KEY_REFERENCE_CATEGORIES)[number];

export interface KeyReferenceEntry {
  id: string;
  name: string;
  key: string;
  category: KeyReferenceCategory;
  source: "vanilla" | "mod";
}

const vanillaEntries = (
  category: KeyReferenceCategory,
  items: readonly { key: string; label: string }[],
): KeyReferenceEntry[] =>
  items.map((item) => ({
    id: `vanilla:${category}:${item.key}`,
    name: item.label,
    key: item.key,
    category,
    source: "vanilla",
  }));

const VANILLA_ENTRIES: KeyReferenceEntry[] = [
  ...vanillaEntries("Jokers", JOKERS),
  ...vanillaEntries("Tarot Cards", TAROT_CARDS),
  ...vanillaEntries("Planet Cards", PLANET_CARDS),
  ...vanillaEntries("Spectral Cards", SPECTRAL_CARDS),
  ...vanillaEntries(
    "Consumable Sets",
    VANILLA_CONSUMABLE_SETS.map((item) => ({ ...item, key: item.value })),
  ),
  ...vanillaEntries(
    "Object Types",
    VANILLA_MISCS.filter((item) =>
      ["Playing Card", "Joker", "Voucher"].includes(item.key),
    ),
  ),
  ...vanillaEntries("Decks", [
    ...VANILLA_DECKS.map((item) => ({
      label: item.label,
      key: `b_${item.label.replace(/ Deck$/, "").toLowerCase()}`,
    })),
    { label: "Painted Deck", key: "b_painted" },
  ]),
  ...vanillaEntries("Vouchers", VANILLA_VOUCHERS),
  ...vanillaEntries(
    "Booster Packs",
    VANILLA_BOOSTERS.map((item) => ({ ...item, key: item.value })),
  ),
  ...vanillaEntries("Enhancements", VANILLA_ENHANCEMENTS),
  ...vanillaEntries("Seals", VANILLA_SEALS),
  ...vanillaEntries("Editions", VANILLA_EDITIONS),
  ...vanillaEntries("Blinds", [
    { label: "Small Blind", key: "bl_small" },
    { label: "Big Blind", key: "bl_big" },
    ...BOSS_BLINDS.map((item) => ({ ...item, key: item.value })),
  ]),
  ...vanillaEntries(
    "Tags",
    TAGS.map((item) => ({
      key: TAG_TYPES[item.value],
      label: item.label.split(" - ")[0],
    })),
  ),
  ...vanillaEntries("Stickers", STICKERS),
  ...vanillaEntries("Sounds", VANILLA_SOUNDS),
  ...vanillaEntries("Shaders", [
    ...VANILLA_SHADERS,
    { label: "Dissolve", key: "dissolve" },
    { label: "Played", key: "played" },
    { label: "Unplayed", key: "unplayed" },
  ]),
  ...vanillaEntries(
    "Rarities",
    VANILLA_RARITIES.map((item) => ({ ...item, key: item.label })),
  ),
  ...vanillaEntries(
    "Suits",
    SUITS.map((item) => ({ ...item, key: item.value })),
  ),
  ...vanillaEntries(
    "Ranks",
    RANKS.map((item) => ({ ...item, key: item.label })),
  ),
  ...vanillaEntries(
    "Poker Hands",
    POKER_HANDS.map((item) => ({ ...item, key: item.value })),
  ),
];

const registeredKey = (key: string, modPrefix: string, classPrefix = "") => {
  let result = key.trim();
  if (modPrefix && !result.startsWith(`${modPrefix}_`)) {
    result = `${modPrefix}_${result}`;
  }
  if (classPrefix && !result.startsWith(`${classPrefix}_`)) {
    result = `${classPrefix}_${result}`;
  }
  return result;
};

export const getKeyReferenceEntries = (
  data: ProjectData,
): KeyReferenceEntry[] => {
  const entries = [...VANILLA_ENTRIES];
  const modPrefix = data.metadata.prefix.trim();

  const addEntry = (
    category: KeyReferenceCategory,
    id: string,
    name: string,
    key: string,
  ) => {
    if (!key.trim()) return;
    entries.push({
      id: `mod:${category}:${id}`,
      name: name.trim() || key,
      key,
      category,
      source: "mod",
    });
  };

  const addObjects = (
    category: KeyReferenceCategory,
    items: readonly BaseGameObject[],
    classPrefix: string,
  ) => {
    for (const item of items) {
      if (!item.objectKey.trim()) continue;
      addEntry(
        category,
        item.id,
        item.name,
        registeredKey(item.objectKey, modPrefix, classPrefix),
      );
    }
  };

  addObjects("Jokers", data.jokers, "j");
  for (const item of data.consumables) {
    const category =
      item.set === "Tarot"
        ? "Tarot Cards"
        : item.set === "Planet"
          ? "Planet Cards"
          : item.set === "Spectral"
            ? "Spectral Cards"
            : "Consumables";
    addObjects(category, [item], "c");
  }
  addObjects("Decks", data.decks, "b");
  addObjects("Vouchers", data.vouchers, "v");
  addObjects("Booster Packs", data.boosters, "p");
  addObjects("Enhancements", data.enhancements, "m");
  addObjects("Seals", data.seals, "");
  addObjects("Editions", data.editions, "e");

  for (const item of data.sounds) {
    if (!item.key.trim()) continue;
    addEntry(
      "Sounds",
      item.id,
      item.key,
      registeredKey(item.key, modPrefix),
    );
  }
  for (const item of data.rarities) {
    if (!item.key.trim()) continue;
    addEntry(
      "Rarities",
      item.id,
      item.name,
      registeredKey(item.key.trim().toLowerCase(), modPrefix),
    );
  }
  for (const item of data.consumableSets) {

    addEntry(
      "Consumable Sets",
      item.id,
      item.name,
      item.key.trim().toLowerCase(),
    );
  }

  const vanillaShaderKeys = new Set(
    VANILLA_ENTRIES.filter((item) => item.category === "Shaders").map(
      (item) => item.key,
    ),
  );
  const seenShaders = new Set<string>();
  for (const item of data.editions) {
    const shader = item.shader;
    if (
      !shader ||
      shader === "false" ||
      vanillaShaderKeys.has(shader) ||
      seenShaders.has(shader)
    ) {
      continue;
    }
    seenShaders.add(shader);
    addEntry(
      "Shaders",
      shader,
      CUSTOM_SHADERS.find((option) => option.key === shader)?.label || shader,
      registeredKey(shader, modPrefix),
    );
  }

  return entries;
};
