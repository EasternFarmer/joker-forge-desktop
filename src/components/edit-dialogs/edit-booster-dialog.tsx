import { useCallback, useMemo } from "react";
import { BalatroCard } from "@/components/balatro/balatro-card";
import {
  GenericItemDialog,
  type DialogTab,
} from "@/components/pages/generic-item-dialog";
import { GenericDialogColorPicker } from "@/components/ui/generic-dialog-color-picker";
import { processBalatroCardImage } from "@/lib/media/image-processing-utils";
import type { BoosterData, BoosterType } from "@/lib/core/types";
import { Cards, Gear, Image as ImageIcon, TextT } from "@phosphor-icons/react";
import { BoosterCardRulesEditor, getDefaultBoosterDescription } from "./booster-card-rules-editor";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const BOOSTER_TYPES: Array<{ value: BoosterType; label: string }> = [
  { value: "joker", label: "Joker Pack" },
  { value: "consumable", label: "Consumable Pack" },
  { value: "playing_card", label: "Playing Card Pack" },
  { value: "voucher", label: "Voucher Pack" },
];

interface EditBoosterDialogProps {
  editingItem: BoosterData | null;
  setEditingItem: (item: BoosterData | null) => void;
  onSave: (id: string, updates: Partial<BoosterData>) => void;
}

export function EditBoosterDialog({
  editingItem,
  setEditingItem,
  onSave,
}: EditBoosterDialogProps) {
  const processBoosterImage = processBalatroCardImage;
  const boosterDialogTabs: DialogTab<BoosterData>[] = useMemo(
    () => [
      {
        id: "visual",
        label: "Visual & Data",
        icon: ImageIcon,
        groups: [
          {
            id: "assets",
            label: "Assets",
            className: "grid grid-cols-2 gap-6",
            fields: [
              {
                id: "image",
                type: "image",
                label: "Main Sprite",
                processFile: processBoosterImage,
              },
            ],
          },
          {
            id: "data",
            label: "Basic Data",
            className: "grid grid-cols-2 gap-6",
            fields: [
              {
                id: "name",
                type: "text",
                label: "Name",
                placeholder: "Booster Name",
                className: "col-span-2",
                validate: (val) => (!val ? "Name is required" : null),
              },
              {
                id: "objectKey",
                type: "text",
                label: "Object Key",
                placeholder: "my_pack",
                className: "col-span-2",
              },
              {
                id: "booster_type",
                type: "custom",
                label: "Booster Type",
                description: "Changing pack type resets its content options.",
                render: (value, onChange, item, setField) => (
                  <Select
                    value={value || "joker"}
                    onValueChange={(nextType) => {
                      if (nextType !== item.booster_type) {
                        setField("card_rules", []);
                        setField("draw_hand", nextType === "consumable");
                        if (item.description === getDefaultBoosterDescription(item.booster_type)) {
                          setField("description", getDefaultBoosterDescription(nextType as BoosterType));
                        }
                      }
                      onChange(nextType);
                    }}
                  >
                    <SelectTrigger className="w-full">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {BOOSTER_TYPES.map((type) => (
                        <SelectItem key={type.value} value={type.value}>
                          {type.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                ),
                validate: (value) => BOOSTER_TYPES.some((type) => type.value === value)
                  ? null : "Choose a pack type.",
              },
              {
                id: "cost",
                type: "number",
                label: "Cost ($)",
                min: 0,
              },
            ],
          },
          {
            id: "config",
            label: "Pack Settings",
            className: "grid grid-cols-2 gap-6",
            fields: [
              {
                id: "weight",
                type: "number",
                label: "Weight",
                min: 0,
                step: 0.05,
              },
              {
                id: "config.extra",
                type: "number",
                label: "Cards in Pack",
                min: 1,
                step: 1,
                validate: (value) => Number.isInteger(value) && value >= 1
                  ? null : "Enter a whole number of at least 1.",
              },
              {
                id: "config.choose",
                type: "number",
                label: "Cards to Choose",
                min: 1,
                step: 1,
                validate: (value, item) => {
                  if (!Number.isInteger(value) || value < 1) {
                    return "Enter a whole number of at least 1.";
                  }
                  return value > (item.config.extra ?? 3)
                    ? "Cannot choose more cards than the pack contains." : null;
                },
              },
            ],
          },
          {
            id: "props",
            label: "Properties",
            className: "grid grid-cols-2 gap-6",
            fields: [
              {
                id: "discovered",
                type: "switch",
                label: "Discovered by Default",
              },
              {
                id: "draw_hand",
                type: "switch",
                label: "Draw Your Hand When Opened",
              },
              {
                id: "instant_use",
                type: "switch",
                label: "Use Selected Consumables Immediately",
                hidden: (item) => item.booster_type !== "consumable",
              },
            ],
          },
        ],
      },
      {
        id: "contents",
        label: "Pack Contents",
        icon: Cards,
        groups: [
          {
            id: "card_rules",
            className: "grid grid-cols-1",
            fields: [
              {
                id: "card_rules",
                type: "custom",
                label: "Content Options",
                render: (value, onChange, item) => (
                  <BoosterCardRulesEditor
                    type={item.booster_type}
                    rules={Array.isArray(value) ? value : []}
                    onChange={onChange}
                  />
                ),
                validate: (value) => {
                  if (!Array.isArray(value)) return null;
                  if (value.some((rule) => !Number.isFinite(rule.weight ?? 1) || (rule.weight ?? 1) < 0)) {
                    return "Content weights must be zero or greater.";
                  }
                  return value.length > 0 && !value.some((rule) => (rule.weight ?? 1) > 0)
                    ? "At least one content option needs a weight greater than zero." : null;
                },
              },
            ],
          },
        ],
      },
      {
        id: "description",
        label: "Description",
        icon: TextT,
        groups: [
          {
            id: "desc",
            fields: [
              {
                id: "description",
                type: "rich-textarea",
                label: "Description",
                validate: (val) => (!val ? "Description is required" : null),
              },
            ],
          },
        ],
      },
      {
        id: "advanced",
        label: "Advanced",
        icon: Gear,
        groups: [
          {
            id: "advanced_fields",
            label: "Advanced Settings",
            className: "grid grid-cols-2 gap-6",
            fields: [
              {
                id: "kind",
                type: "text",
                label: "Kind",
                placeholder: "e.g. Ephemeral",
              },
              {
                id: "pack_group",
                type: "custom",
                label: "Pack Group Name",
                description: "Optional display title shown when opening the pack. Defaults to the pack name.",
                render: (_value, _onChange, item, setField) => (
                  <Input
                    value={item.group_key || ""}
                    placeholder="e.g. Mystical Pack"
                    onChange={(event) => setField("group_key", event.target.value)}
                  />
                ),
              },
              {
                id: "hidden",
                type: "switch",
                label: "Hidden from Collection",
              },
            ],
          },
          {
            id: "colors",
            label: "Pack Colors",
            fields: [
              {
                id: "background_colour",
                type: "custom",
                label: "Background Color",
                render: (value, onChange) => (
                  <GenericDialogColorPicker
                    value={value}
                    onChange={onChange}
                    defaultColor="#666666"
                    valueMode="without-hash"
                    placeholder="#666666"
                  />
                ),
              },
              {
                id: "special_colour",
                type: "custom",
                label: "Special Color",
                render: (value, onChange) => (
                  <GenericDialogColorPicker
                    value={value}
                    onChange={onChange}
                    defaultColor="#666666"
                    valueMode="without-hash"
                    placeholder="#666666"
                  />
                ),
              },
            ],
          },
        ],
      },
    ],
    [processBoosterImage],
  );

  const renderPreview = useCallback(
    (item: BoosterData | null) => (
      <BalatroCard type="booster" data={item || {}} size="lg" />
    ),
    [],
  );

  return (
    <GenericItemDialog
      open={!!editingItem}
      onOpenChange={(open) => !open && setEditingItem(null)}
      item={editingItem}
      title={`Edit ${editingItem?.name || "Booster"}`}
      description="Customize the pack's appearance, contents, and selection settings."
      tabs={boosterDialogTabs}
      onSave={onSave}
      showPlaceholderPicker
      placeholderCategory="booster"
      renderPreview={renderPreview}
    />
  );
}
