//! Input types that mirror the TypeScript `JokerData` interface and a mapper
//! that converts them into the canonical `balatro_codegen::types::JokerDef`.
//!
//! This module is the *single source of truth* for the `JokerData → JokerDef`
//! conversion. Previously this logic lived in the TypeScript `mapJokerToRustDef`
//! function; having it here means adding a new effect/variable type only requires
//! updating Rust, not both the TypeScript mapper and the Rust codegen.

use balatro_codegen::types::{
    AppearanceDef, AtlasPos, BoosterCardRuleDef, BoosterDef, ConditionDef, ConditionGroupDef, ConsumableDef, ConsumableTypeDef, DescriptionVariableBinding,
    DeckDef, DisplaySize, EditionDef, EffectDef, EnhancementDef, JokerDef, LogicOp, LoopGroupDef,
    ParamValue, RandomGroupDef, RarityDef, RuleDef, SealDef, TypedValue, UnlockDef, UserVarType,
    UserVariableDef, VoucherDef,
};
use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

// ---------------------------------------------------------------------------
// Input types, match the TypeScript `JokerData` / `Rule` shapes exactly
// ---------------------------------------------------------------------------

/// Atlas position sent alongside joker data at export time.
#[derive(Debug, Clone, Deserialize)]
pub struct AtlasPosInput {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LocalizationEntryInput {
    pub language: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// Mirrors the TypeScript `JokerData` interface.
///
/// Fields use their original TypeScript names (mix of camelCase and snake_case)
/// via individual `#[serde(rename)]` attributes where needed.
#[derive(Debug, Deserialize)]
pub struct JokerDataInput {
    #[serde(rename = "objectKey")]
    pub object_key: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub localizations: Vec<LocalizationEntryInput>,
    pub cost: i32,
    /// Can be a number (1–4) or string ("common" | "uncommon" | "rare" | "legendary").
    pub rarity: Value,
    #[serde(default)]
    pub blueprint_compat: Option<bool>,
    #[serde(default)]
    pub eternal_compat: Option<bool>,
    #[serde(default)]
    pub perishable_compat: Option<bool>,
    #[serde(default)]
    pub unlocked: Option<bool>,
    #[serde(default)]
    pub discovered: Option<bool>,
    /// Scale width as a percentage (100 = 1×). Used to compute `display_size`.
    #[serde(default)]
    pub scale_w: Option<f64>,
    /// Scale height as a percentage (100 = 1×).
    #[serde(default)]
    pub scale_h: Option<f64>,
    #[serde(default)]
    pub rules: Vec<RuleInput>,
    #[serde(rename = "userVariables", default)]
    pub user_variables: Vec<UserVariableInput>,
    #[serde(rename = "descriptionVariables", default)]
    pub description_variables: Option<Vec<DescriptionVariableBinding>>,
    #[serde(default)]
    pub force_eternal: bool,
    #[serde(default)]
    pub force_perishable: bool,
    #[serde(default)]
    pub force_rental: bool,
    #[serde(default)]
    pub force_foil: bool,
    #[serde(default)]
    pub force_holographic: bool,
    #[serde(default)]
    pub force_polychrome: bool,
    #[serde(default)]
    pub force_negative: bool,
    #[serde(default, rename = "ignoreSlotLimit")]
    pub ignore_slot_limit: bool,
    #[serde(default)]
    pub info_queues: Vec<String>,
    #[serde(default)]
    pub pools: Vec<String>,
    #[serde(default)]
    pub appears_in_shop: Option<bool>,
    #[serde(default)]
    pub appear_flags: Option<String>,
    #[serde(default, rename = "unlockTrigger")]
    pub unlock_trigger: Option<String>,
    #[serde(default, rename = "unlockDescription")]
    pub unlock_description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConsumableDataInput {
    #[serde(rename = "objectKey")]
    pub object_key: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub localizations: Vec<LocalizationEntryInput>,
    pub set: String,
    #[serde(default)]
    pub cost: Option<i32>,
    #[serde(default)]
    pub unlocked: Option<bool>,
    #[serde(default)]
    pub discovered: Option<bool>,
    #[serde(default)]
    pub hidden: Option<bool>,
    #[serde(default)]
    pub can_repeat_soul: Option<bool>,
    #[serde(default)]
    pub rules: Vec<RuleInput>,
    #[serde(rename = "userVariables", default)]
    pub user_variables: Vec<UserVariableInput>,
    #[serde(rename = "descriptionVariables", default)]
    pub description_variables: Option<Vec<DescriptionVariableBinding>>,
    #[serde(default)]
    pub atlas: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EnhancementDataInput {
    #[serde(rename = "objectKey")]
    pub object_key: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub localizations: Vec<LocalizationEntryInput>,
    #[serde(default)]
    pub rules: Vec<RuleInput>,
    #[serde(rename = "userVariables", default)]
    pub user_variables: Vec<UserVariableInput>,
    #[serde(rename = "descriptionVariables", default)]
    pub description_variables: Option<Vec<DescriptionVariableBinding>>,
    #[serde(default)]
    pub any_suit: Option<bool>,
    #[serde(default)]
    pub replace_base_card: Option<bool>,
    #[serde(default)]
    pub no_rank: Option<bool>,
    #[serde(default)]
    pub no_suit: Option<bool>,
    #[serde(default)]
    pub always_scores: Option<bool>,
    #[serde(default)]
    pub unlocked: Option<bool>,
    #[serde(default)]
    pub discovered: Option<bool>,
    #[serde(default)]
    pub no_collection: Option<bool>,
    #[serde(default)]
    pub weight: Option<f64>,
    #[serde(default)]
    pub atlas: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SealDataInput {
    #[serde(rename = "objectKey")]
    pub object_key: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub localizations: Vec<LocalizationEntryInput>,
    #[serde(default)]
    pub rules: Vec<RuleInput>,
    #[serde(rename = "userVariables", default)]
    pub user_variables: Vec<UserVariableInput>,
    #[serde(rename = "descriptionVariables", default)]
    pub description_variables: Option<Vec<DescriptionVariableBinding>>,
    #[serde(default)]
    pub badge_colour: Option<String>,
    #[serde(default)]
    pub unlocked: Option<bool>,
    #[serde(default)]
    pub discovered: Option<bool>,
    #[serde(default)]
    pub no_collection: Option<bool>,
    #[serde(default)]
    pub sound: Option<String>,
    #[serde(default)]
    pub pitch: Option<f64>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub atlas: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EditionDataInput {
    #[serde(rename = "objectKey")]
    pub object_key: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub localizations: Vec<LocalizationEntryInput>,
    #[serde(default)]
    pub rules: Vec<RuleInput>,
    #[serde(rename = "userVariables", default)]
    pub user_variables: Vec<UserVariableInput>,
    #[serde(rename = "descriptionVariables", default)]
    pub description_variables: Option<Vec<DescriptionVariableBinding>>,
    #[serde(default)]
    pub shader: Option<Value>,
    #[serde(default)]
    pub in_shop: Option<bool>,
    #[serde(default)]
    pub weight: Option<f64>,
    #[serde(default)]
    pub extra_cost: Option<i32>,
    #[serde(default)]
    pub apply_to_float: Option<bool>,
    #[serde(default)]
    pub badge_colour: Option<String>,
    #[serde(default)]
    pub sound: Option<String>,
    #[serde(default)]
    pub pitch: Option<f64>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub disable_shadow: Option<bool>,
    #[serde(default)]
    pub disable_base_shader: Option<bool>,
    #[serde(default)]
    pub unlocked: Option<bool>,
    #[serde(default)]
    pub discovered: Option<bool>,
    #[serde(default)]
    pub no_collection: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct VoucherDataInput {
    #[serde(rename = "objectKey")]
    pub object_key: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub localizations: Vec<LocalizationEntryInput>,
    #[serde(default, rename = "unlockDescription")]
    pub unlock_description: Option<String>,
    #[serde(default)]
    pub cost: Option<i32>,
    #[serde(default)]
    pub unlocked: Option<bool>,
    #[serde(default)]
    pub discovered: Option<bool>,
    #[serde(default)]
    pub no_collection: Option<bool>,
    #[serde(default)]
    pub can_repeat_soul: Option<bool>,
    #[serde(default)]
    pub requires: Option<String>,
    #[serde(default)]
    pub rules: Vec<RuleInput>,
    #[serde(rename = "userVariables", default)]
    pub user_variables: Vec<UserVariableInput>,
    #[serde(rename = "descriptionVariables", default)]
    pub description_variables: Option<Vec<DescriptionVariableBinding>>,
    #[serde(default)]
    pub draw_shader_sprite: Option<Value>,
    #[serde(default)]
    pub atlas: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeckDataInput {
    #[serde(rename = "objectKey")]
    pub object_key: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub localizations: Vec<LocalizationEntryInput>,
    #[serde(default)]
    pub rules: Vec<RuleInput>,
    #[serde(rename = "userVariables", default)]
    pub user_variables: Vec<UserVariableInput>,
    #[serde(rename = "descriptionVariables", default)]
    pub description_variables: Option<Vec<DescriptionVariableBinding>>,
    #[serde(default)]
    pub unlocked: Option<bool>,
    #[serde(default)]
    pub discovered: Option<bool>,
    #[serde(default)]
    pub no_collection: Option<bool>,
    #[serde(default, rename = "Config_vouchers")]
    pub config_vouchers: Vec<String>,
    #[serde(default, rename = "Config_consumables")]
    pub config_consumables: Vec<String>,
    #[serde(default)]
    pub no_interest: bool,
    #[serde(default)]
    pub no_faces: bool,
    #[serde(default)]
    pub erratic_deck: bool,
    #[serde(default)]
    pub atlas: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BoosterDataInput {
    #[serde(rename = "objectKey")]
    pub object_key: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub localizations: Vec<LocalizationEntryInput>,
    #[serde(default = "default_booster_type")]
    pub booster_type: String,
    #[serde(default)]
    pub config: BoosterConfigInput,
    #[serde(default)]
    pub card_rules: Vec<BoosterCardRuleDef>,
    #[serde(default)]
    pub cost: Option<i32>,
    #[serde(default)]
    pub weight: Option<f64>,
    #[serde(default)]
    pub draw_hand: Option<bool>,
    #[serde(default)]
    pub instant_use: Option<bool>,
    #[serde(default)]
    pub unlocked: Option<bool>,
    #[serde(default)]
    pub discovered: Option<bool>,
    #[serde(default)]
    pub hidden: Option<bool>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub group_key: Option<String>,
    #[serde(default)]
    pub background_colour: Option<String>,
    #[serde(default)]
    pub special_colour: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct BoosterConfigInput {
    #[serde(default)]
    pub extra: Option<i32>,
    #[serde(default)]
    pub choose: Option<i32>,
}

fn default_booster_type() -> String {
    "joker".to_string()
}

#[derive(Debug, Deserialize)]
pub struct RarityDataInput {
    pub key: String,
    pub name: String,
    pub badge_colour: String,
    #[serde(default)]
    pub default_weight: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct ConsumableSetDataInput {
    pub key: String,
    pub name: String,
    pub primary_colour: String,
    pub secondary_colour: String,
    #[serde(default)]
    pub shop_rate: Option<f64>,
    pub collection_rows: [i32; 2],
    pub collection_name: String,
    #[serde(default)]
    pub default_card: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundDataInput {
    pub key: String,
    pub sound_string: String,
    #[serde(default)]
    pub audio_bytes: Option<Vec<u8>>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub pitch: Option<f64>,
    #[serde(default)]
    pub replace: Option<String>,
}

/// Mirrors the TypeScript `Rule` interface.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleInput {
    pub id: String,
    pub trigger: String,
    #[serde(default)]
    pub condition_groups: Vec<ConditionGroupInput>,
    #[serde(default)]
    pub effects: Vec<EffectInput>,
    #[serde(default)]
    pub random_groups: Vec<RandomGroupInput>,
    /// TypeScript field is `loops`: not `loopGroups`.
    #[serde(default)]
    pub loops: Vec<LoopGroupInput>,
}

/// Mirrors the TypeScript `ConditionGroup` interface.
#[derive(Debug, Deserialize)]
pub struct ConditionGroupInput {
    /// `"and"` | `"or"`
    pub operator: String,
    #[serde(default)]
    pub conditions: Vec<ConditionInput>,
}

/// Mirrors the TypeScript `Condition` interface.
#[derive(Debug, Deserialize)]
pub struct ConditionInput {
    #[serde(default)]
    pub id: String,
    #[serde(rename = "type")]
    pub condition_type: String,
    #[serde(default)]
    pub negate: bool,
    #[serde(default)]
    pub operator: Option<String>,
    #[serde(default)]
    pub params: HashMap<String, WrappedParamInput>,
}

/// Mirrors the TypeScript `Effect` interface.
#[derive(Debug, Deserialize)]
pub struct EffectInput {
    #[serde(default)]
    pub id: String,
    #[serde(rename = "type")]
    pub effect_type: String,
    #[serde(default)]
    pub params: HashMap<String, WrappedParamInput>,
    #[serde(default, rename = "customMessage")]
    pub custom_message: Option<String>,
    #[serde(default, rename = "messageMode")]
    pub message_mode: Option<String>,
}

/// Mirrors the TypeScript `RandomGroup` interface.
#[derive(Debug, Deserialize)]
pub struct RandomGroupInput {
    pub id: String,
    pub chance_numerator: WrappedParamInput,
    pub chance_denominator: WrappedParamInput,
    #[serde(default)]
    pub effects: Vec<EffectInput>,
}

/// Mirrors the TypeScript `LoopGroup` interface.
#[derive(Debug, Deserialize)]
pub struct LoopGroupInput {
    pub id: String,
    /// TypeScript field is `repetitions`.
    pub repetitions: WrappedParamInput,
    #[serde(default)]
    pub effects: Vec<EffectInput>,
}

/// A raw TypeScript param value: `{ value: T, valueType?: string }`.
///
/// The TypeScript frontend stores all effect/condition params in this
/// wrapped form. `valueType` is present only for dynamic values
/// (game variables, user variables, ranges: etc.).
#[derive(Debug)]
pub struct WrappedParamInput {
    pub value: Value,
    pub value_type: Option<String>,
}

impl<'de> Deserialize<'de> for WrappedParamInput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum WrappedOrRaw {
            Wrapped {
                value: Value,
                #[serde(rename = "valueType", default)]
                value_type: Option<String>,
            },
            Raw(Value),
        }

        match WrappedOrRaw::deserialize(deserializer)? {
            WrappedOrRaw::Wrapped { value, value_type } => Ok(Self { value, value_type }),
            WrappedOrRaw::Raw(value) => Ok(Self {
                value,
                value_type: None,
            }),
        }
    }
}

/// Mirrors the TypeScript `UserVariable` interface.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserVariableInput {
    pub name: String,
    #[serde(rename = "type", default)]
    pub var_type: String,
    #[serde(default)]
    pub is_global: bool,
    #[serde(default)]
    pub is_persistent: bool,
    pub initial_value: Option<f64>,
    pub initial_suit: Option<String>,
    pub initial_rank: Option<String>,
    pub initial_poker_hand: Option<String>,
    pub initial_key: Option<String>,
    pub initial_text: Option<String>,
}

/// A joker entry for `batch_export_jokers`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchJokerEntry {
    pub joker_data: JokerDataInput,
    pub pos: AtlasPosInput,
    pub soul_pos: Option<AtlasPosInput>,
    /// Filename to write: e.g. `"j_my_joker.lua"`.
    pub file_name: String,
    /// Optional custom Lua code. When present, skip compilation and use this.
    #[serde(default)]
    pub custom_lua: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchConsumableEntry {
    pub consumable_data: ConsumableDataInput,
    pub pos: AtlasPosInput,
    #[serde(default)]
    pub soul_pos: Option<AtlasPosInput>,
    pub file_name: String,
    #[serde(default)]
    pub custom_lua: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchVoucherEntry {
    pub voucher_data: VoucherDataInput,
    pub pos: AtlasPosInput,
    #[serde(default)]
    pub soul_pos: Option<AtlasPosInput>,
    pub file_name: String,
    #[serde(default)]
    pub custom_lua: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchDeckEntry {
    pub deck_data: DeckDataInput,
    pub pos: AtlasPosInput,
    pub file_name: String,
    #[serde(default)]
    pub custom_lua: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchEnhancementEntry {
    pub enhancement_data: EnhancementDataInput,
    pub pos: AtlasPosInput,
    pub file_name: String,
    #[serde(default)]
    pub custom_lua: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSealEntry {
    pub seal_data: SealDataInput,
    pub pos: AtlasPosInput,
    pub file_name: String,
    #[serde(default)]
    pub custom_lua: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchEditionEntry {
    pub edition_data: EditionDataInput,
    pub file_name: String,
    #[serde(default)]
    pub custom_lua: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchBoosterEntry {
    pub booster_data: BoosterDataInput,
    pub pos: AtlasPosInput,
    pub file_name: String,
    #[serde(default)]
    pub custom_lua: Option<String>,
}

/// Mirrors the TypeScript `ModMetadata` interface for package export.
#[derive(Debug, Clone, Deserialize)]
pub struct ModMetadataInput {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub author: Vec<String>,
    pub description: String,
    pub prefix: String,
    pub main_file: String,
    pub version: String,
    pub priority: i64,
    pub badge_colour: String,
    pub badge_text_colour: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
    #[serde(default)]
    pub provides: Vec<String>,
    #[serde(default)]
    pub disable_vanilla: bool,
}

// ---------------------------------------------------------------------------
// Conversion, JokerDataInput → JokerDef
// ---------------------------------------------------------------------------

/// Convert raw `JokerDataInput` (from the TypeScript frontend) into a canonical
/// `JokerDef` suitable for `balatro_codegen::compile_joker_with_options`.
pub fn joker_data_to_def(
    input: &JokerDataInput,
    mod_prefix: &str,
    pos: AtlasPosInput,
    soul_pos: Option<AtlasPosInput>,
) -> JokerDef {
    let appearance = map_appearance(input);
    let unlock = map_unlock(input);

    JokerDef {
        key: input.object_key.clone(),
        name: input.name.clone(),
        description: split_description(&input.description),
        cost: input.cost,
        rarity: normalize_rarity(&input.rarity, mod_prefix),
        blueprint_compat: input.blueprint_compat.unwrap_or(false),
        eternal_compat: input.eternal_compat.unwrap_or(false),
        perishable_compat: input.perishable_compat.unwrap_or(true),
        unlocked: input.unlocked.unwrap_or(true),
        discovered: input.discovered.unwrap_or(true),
        atlas: "CustomJokers".to_string(),
        pos: AtlasPos { x: pos.x, y: pos.y },
        soul_pos: soul_pos.map(|sp| AtlasPos { x: sp.x, y: sp.y }),
        display_size: compute_display_size(input.scale_w, input.scale_h),
        rules: input.rules.iter().map(map_rule).collect(),
        appearance,
        unlock,
        user_variables: input.user_variables.iter().map(map_user_variable).collect(),
        description_variables: input.description_variables.clone(),
        force_eternal: input.force_eternal,
        force_perishable: input.force_perishable,
        force_rental: input.force_rental,
        force_foil: input.force_foil,
        force_holographic: input.force_holographic,
        force_polychrome: input.force_polychrome,
        force_negative: input.force_negative,
        ignore_slot_limit: input.ignore_slot_limit,
        info_queues: input.info_queues.clone(),
    }
}

pub fn consumable_data_to_def(
    input: &ConsumableDataInput,
    pos: AtlasPosInput,
    soul_pos: Option<AtlasPosInput>,
) -> ConsumableDef {
    let requested_atlas = input.atlas.as_deref().unwrap_or("CustomConsumables").trim();
    let atlas = if requested_atlas.is_empty()
        || requested_atlas.eq_ignore_ascii_case("Consumables")
        || requested_atlas.eq_ignore_ascii_case("CustomConsumables")
        || requested_atlas.ends_with("_Consumables")
    {
        "CustomConsumables".to_string()
    } else {
        requested_atlas.to_string()
    };

    ConsumableDef {
        key: input.object_key.clone(),
        name: input.name.clone(),
        description: split_description(&input.description),
        set: input.set.clone(),
        cost: input.cost,
        unlocked: input.unlocked,
        discovered: input.discovered,
        hidden: input.hidden,
        can_repeat_soul: input.can_repeat_soul,
        atlas,
        pos: AtlasPos { x: pos.x, y: pos.y },
        soul_pos: soul_pos.map(|sp| AtlasPos { x: sp.x, y: sp.y }),
        rules: input.rules.iter().map(map_rule).collect(),
        user_variables: input.user_variables.iter().map(map_user_variable).collect(),
        description_variables: input.description_variables.clone(),
    }
}

pub fn enhancement_data_to_def(input: &EnhancementDataInput, pos: AtlasPosInput) -> EnhancementDef {
    EnhancementDef {
        key: input.object_key.clone(),
        name: input.name.clone(),
        description: split_description(&input.description),
        atlas: input
            .atlas
            .clone()
            .unwrap_or_else(|| "CustomEnhancements".to_string()),
        pos: AtlasPos { x: pos.x, y: pos.y },
        rules: input.rules.iter().map(map_rule).collect(),
        user_variables: input.user_variables.iter().map(map_user_variable).collect(),
        description_variables: input.description_variables.clone(),
        any_suit: input.any_suit,
        replace_base_card: input.replace_base_card,
        no_rank: input.no_rank,
        no_suit: input.no_suit,
        always_scores: input.always_scores,
        unlocked: input.unlocked,
        discovered: input.discovered,
        no_collection: input.no_collection,
        weight: input.weight,
    }
}

pub fn seal_data_to_def(input: &SealDataInput, pos: AtlasPosInput) -> SealDef {
    SealDef {
        key: input.object_key.clone(),
        name: input.name.clone(),
        description: split_description(&input.description),
        atlas: input
            .atlas
            .clone()
            .unwrap_or_else(|| "CustomSeals".to_string()),
        pos: AtlasPos { x: pos.x, y: pos.y },
        rules: input.rules.iter().map(map_rule).collect(),
        user_variables: input.user_variables.iter().map(map_user_variable).collect(),
        description_variables: input.description_variables.clone(),
        badge_colour: input.badge_colour.clone(),
        unlocked: input.unlocked,
        discovered: input.discovered,
        no_collection: input.no_collection,
        sound: input
            .sound
            .clone()
            .unwrap_or_else(|| "gold_seal".to_string()),
        pitch: input.pitch,
        volume: input.volume,
    }
}

pub fn edition_data_to_def(input: &EditionDataInput) -> EditionDef {
    EditionDef {
        key: input.object_key.clone(),
        name: input.name.clone(),
        description: split_description(&input.description),
        rules: input.rules.iter().map(map_rule).collect(),
        user_variables: input.user_variables.iter().map(map_user_variable).collect(),
        description_variables: input.description_variables.clone(),
        shader: option_value_to_string(input.shader.as_ref()),
        in_shop: input.in_shop,
        weight: input.weight,
        extra_cost: input.extra_cost,
        apply_to_float: input.apply_to_float,
        badge_colour: input.badge_colour.clone(),
        sound: input.sound.clone(),
        pitch: input.pitch,
        volume: input.volume,
        disable_shadow: input.disable_shadow,
        disable_base_shader: input.disable_base_shader,
        unlocked: input.unlocked,
        discovered: input.discovered,
        no_collection: input.no_collection,
    }
}

pub fn voucher_data_to_def(
    input: &VoucherDataInput,
    pos: AtlasPosInput,
    soul_pos: Option<AtlasPosInput>,
) -> VoucherDef {
    let requested_atlas = input.atlas.as_deref().unwrap_or("CustomVouchers").trim();
    let atlas = if requested_atlas.is_empty()
        || requested_atlas.eq_ignore_ascii_case("Voucher")
        || requested_atlas.eq_ignore_ascii_case("CustomVouchers")
        || requested_atlas.ends_with("_Voucher")
        || requested_atlas.ends_with("_Vouchers")
    {
        "CustomVouchers".to_string()
    } else {
        requested_atlas.to_string()
    };

    VoucherDef {
        key: input.object_key.clone(),
        name: input.name.clone(),
        description: split_description(&input.description),
        unlock_description: split_description(input.unlock_description.as_deref().unwrap_or("")),
        cost: input.cost,
        unlocked: input.unlocked,
        discovered: input.discovered,
        no_collection: input.no_collection,
        can_repeat_soul: input.can_repeat_soul,
        requires: input.requires.clone(),
        atlas,
        pos: AtlasPos { x: pos.x, y: pos.y },
        soul_pos: soul_pos.map(|sp| AtlasPos { x: sp.x, y: sp.y }),
        rules: input.rules.iter().map(map_rule).collect(),
        user_variables: input.user_variables.iter().map(map_user_variable).collect(),
        description_variables: input.description_variables.clone(),
        draw_shader_sprite: option_value_to_string(input.draw_shader_sprite.as_ref()),
    }
}

pub fn deck_data_to_def(input: &DeckDataInput, mod_prefix: &str, pos: AtlasPosInput) -> DeckDef {
    let requested_atlas = input.atlas.as_deref().unwrap_or("CustomDecks").trim();
    let normalized_prefixed_enhancers = normalize_mod_prefixed_key(mod_prefix, "Enhancers");
    let atlas = if requested_atlas.is_empty()
        || requested_atlas.eq_ignore_ascii_case("Enhancers")
        || requested_atlas.eq_ignore_ascii_case("CustomDecks")
        || requested_atlas == normalized_prefixed_enhancers
    {
        "CustomDecks".to_string()
    } else {
        requested_atlas.to_string()
    };

    DeckDef {
        key: input.object_key.clone(),
        name: input.name.clone(),
        description: split_description(&input.description),
        atlas,
        pos: AtlasPos { x: pos.x, y: pos.y },
        rules: input.rules.iter().map(map_rule).collect(),
        user_variables: input.user_variables.iter().map(map_user_variable).collect(),
        description_variables: input.description_variables.clone(),
        unlocked: input.unlocked,
        discovered: input.discovered,
        no_collection: input.no_collection,
        config_vouchers: input.config_vouchers.clone(),
        config_consumables: input.config_consumables.clone(),
        no_interest: input.no_interest,
        no_faces: input.no_faces,
        erratic_deck: input.erratic_deck,
    }
}

pub fn booster_data_to_def(input: &BoosterDataInput, pos: AtlasPosInput) -> BoosterDef {
    BoosterDef {
        key: input.object_key.clone(),
        name: input.name.clone(),
        description: split_description(&input.description),
        atlas: "CustomBoosters".to_string(),
        pos: AtlasPos { x: pos.x, y: pos.y },
        cost: input.cost,
        weight: input.weight,
        kind: input.kind.clone(),
        draw: None,
        extra: input.config.extra,
        choose: input.config.choose,
        booster_type: input.booster_type.clone(),
        card_rules: input.card_rules.clone(),
        draw_hand: input.draw_hand,
        instant_use: input.instant_use,
        unlocked: input.unlocked,
        discovered: input.discovered,
        hidden: input.hidden,
        group_key: input.group_key.clone(),
        background_colour: input.background_colour.clone(),
        special_colour: input.special_colour.clone(),
        rules: vec![],
    }
}

pub fn rarity_data_to_def(input: &RarityDataInput) -> RarityDef {
    RarityDef {
        key: input.key.trim().to_ascii_lowercase(),
        name: input.name.clone(),
        badge_colour: input.badge_colour.clone(),
        default_weight: input.default_weight.unwrap_or(1.0),
    }
}

pub fn consumable_set_data_to_def(input: &ConsumableSetDataInput) -> ConsumableTypeDef {
    ConsumableTypeDef {
        key: input.key.trim().to_ascii_lowercase(),
        name: input.name.clone(),
        collection_name: Some(input.collection_name.clone()),
        primary_colour: input.primary_colour.clone(),
        secondary_colour: input.secondary_colour.clone(),
        collection_rows: (input.collection_rows[0], input.collection_rows[1]),
        default_card: option_value_to_string(input.default_card.as_ref()),
        shop_rate: input.shop_rate,
    }
}

// ---------------------------------------------------------------------------
// Rule / condition / effect mappers
// ---------------------------------------------------------------------------

fn map_rule(rule: &RuleInput) -> RuleDef {
    let (retrigger, destroy) = compute_rule_flags(rule);

    RuleDef {
        id: rule.id.clone(),
        trigger: rule.trigger.clone(),
        retrigger,
        destroy,
        condition_groups: rule
            .condition_groups
            .iter()
            .map(map_condition_group)
            .collect(),
        effects: rule.effects.iter().map(map_effect).collect(),
        random_groups: rule.random_groups.iter().map(map_random_group).collect(),
        loop_groups: rule.loops.iter().map(map_loop_group).collect(),
    }
}

fn map_condition_group(cg: &ConditionGroupInput) -> ConditionGroupDef {
    ConditionGroupDef {
        logic_operator: if cg.operator.eq_ignore_ascii_case("or") {
            LogicOp::Or
        } else {
            LogicOp::And
        },
        conditions: cg.conditions.iter().map(map_condition).collect(),
    }
}

fn map_condition(c: &ConditionInput) -> ConditionDef {
    ConditionDef {
        id: c.id.clone(),
        condition_type: c.condition_type.clone(),
        negate: c.negate,
        operator: c.operator.as_deref().and_then(parse_logic_op),
        params: map_params(&c.params),
    }
}

fn map_effect(e: &EffectInput) -> EffectDef {
    let mut params = map_params(&e.params);
    if matches!(e.effect_type.as_str(), "edit_hand_size" | "edit_play_size" | "edit_discard_size") {
        if let Some(message) = &e.custom_message {
            params.insert("customMessage".to_string(), ParamValue::Str(message.clone()));
        }
        if let Some(mode) = &e.message_mode {
            params.insert("messageMode".to_string(), ParamValue::Str(mode.clone()));
        }
    }
    EffectDef {
        id: e.id.clone(),
        effect_type: e.effect_type.clone(),
        params,
    }
}

fn map_random_group(rg: &RandomGroupInput) -> RandomGroupDef {
    RandomGroupDef {
        id: rg.id.clone(),
        chance_numerator: wrapped_to_param(&rg.chance_numerator),
        chance_denominator: wrapped_to_param(&rg.chance_denominator),
        effects: rg.effects.iter().map(map_effect).collect(),
    }
}

fn map_loop_group(lg: &LoopGroupInput) -> LoopGroupDef {
    LoopGroupDef {
        id: lg.id.clone(),
        count: wrapped_to_param(&lg.repetitions),
        effects: lg.effects.iter().map(map_effect).collect(),
    }
}

fn map_params(params: &HashMap<String, WrappedParamInput>) -> HashMap<String, ParamValue> {
    params
        .iter()
        .map(|(k, v)| (k.clone(), wrapped_to_param(v)))
        .collect()
}

// ---------------------------------------------------------------------------
// User variable mapper
// ---------------------------------------------------------------------------

fn map_user_variable(v: &UserVariableInput) -> UserVariableDef {
    let var_type = match v.var_type.as_str() {
        "suit" => UserVarType::Suit,
        "rank" => UserVarType::Rank,
        "pokerhand" => UserVarType::PokerHand,
        "key" => UserVarType::Key,
        "text" => UserVarType::Text,
        _ => UserVarType::Number,
    };

    let initial_value = match v.var_type.as_str() {
        "suit" => ParamValue::Str(v.initial_suit.clone().unwrap_or_else(|| "Spades".into())),
        "rank" => ParamValue::Str(v.initial_rank.clone().unwrap_or_else(|| "Ace".into())),
        "pokerhand" => ParamValue::Str(
            v.initial_poker_hand
                .clone()
                .unwrap_or_else(|| "High Card".into()),
        ),
        "key" => ParamValue::Str(v.initial_key.clone().unwrap_or_else(|| "none".into())),
        "text" => ParamValue::Str(v.initial_text.clone().unwrap_or_default()),
        _ => ParamValue::Float(v.initial_value.unwrap_or(0.0)),
    };

    UserVariableDef {
        name: v.name.clone(),
        var_type,
        initial_value,
        is_global: v.is_global,
        is_persistent: v.is_persistent,
    }
}

pub fn map_user_variable_inputs(values: &[UserVariableInput]) -> Vec<UserVariableDef> {
    values.iter().map(map_user_variable).collect()
}

// ---------------------------------------------------------------------------
// Primitive helpers
// ---------------------------------------------------------------------------

/// Convert a `{ value: valueType? }` wrapped param into a `ParamValue`.
fn wrapped_to_param(w: &WrappedParamInput) -> ParamValue {
    if let Some(ref vt) = w.value_type {
        return ParamValue::Typed(TypedValue {
            value: w.value.clone(),
            value_type: vt.clone(),
        });
    }
    json_value_to_param(&w.value)
}

/// Convert a bare `serde_json::Value` to `ParamValue`.
fn json_value_to_param(v: &Value) -> ParamValue {
    match v {
        Value::Number(n) => n
            .as_i64()
            .map(ParamValue::Int)
            .unwrap_or_else(|| ParamValue::Float(n.as_f64().unwrap_or(0.0))),
        Value::Bool(b) => ParamValue::Bool(*b),
        Value::String(s) => ParamValue::Str(s.clone()),
        _ => ParamValue::Str(v.to_string()),
    }
}

fn option_value_to_string(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::String(s)) => Some(s.clone()),
        _ => None,
    }
}

/// Map a rarity value that is either a number (1–4) or a string to the
/// canonical lowercase string used by `balatro_codegen`.
///
/// Custom rarity strings are normalized to include the mod prefix so the
/// generated joker rarity key matches the pool created by `SMODS.Rarity`.
fn normalize_rarity(rarity: &Value, mod_prefix: &str) -> String {
    match rarity {
        Value::String(s) => {
            let normalized = s.trim().to_ascii_lowercase();
            if normalized.is_empty() {
                return "common".to_string();
            }

            if is_vanilla_rarity_key(&normalized) {
                return normalized;
            }

            let prefix = mod_prefix.trim().to_ascii_lowercase();
            if prefix.is_empty() || normalized.starts_with(&format!("{}_", prefix)) {
                normalized
            } else {
                format!("{}_{}", prefix, normalized)
            }
        }
        Value::Number(n) => match n.as_u64().unwrap_or(1) {
            2 => "uncommon",
            3 => "rare",
            4 => "legendary",
            _ => "common",
        }
        .to_string(),
        _ => "common".to_string(),
    }
}

fn is_vanilla_rarity_key(value: &str) -> bool {
    matches!(value, "common" | "uncommon" | "rare" | "legendary")
}

/// Split an HTML-formatted description string into individual lines.
///
/// Replaces `<br>` variants and `[s]` with newlines and trims each line.
/// Keep blank rows as a space because Balatro's localization parser drops
/// empty strings. Entirely blank descriptions use `["No description"]`.
/// Carry the latest formatting tag until a replacement or reset so a formatted
/// selection keeps its style after Balatro starts parsing a new line.
fn split_description(desc: &str) -> Vec<String> {
    // Handle common <br> variants case-insensitively without pulling in a regex dep
    let normalized = desc
        .replace("<br />", "\n")
        .replace("<br/>", "\n")
        .replace("<br>", "\n")
        .replace("<BR />", "\n")
        .replace("<BR/>", "\n")
        .replace("<BR>", "\n")
        .replace("[s]", "\n");

    let lines: Vec<String> = normalized
        .split('\n')
        .map(|l| l.trim().to_string())
        .collect();

    if lines.iter().all(|line| line.is_empty()) {
        vec!["No description".to_string()]
    } else {
        let mut active_tag = String::new();
        lines
            .into_iter()
            .map(|line| {
                let formatted = if active_tag.is_empty() || line.starts_with('{') {
                    line.clone()
                } else {
                    format!("{active_tag}{line}")
                };
                let mut remaining = line.as_str();
                let mut visible = String::new();
                while let Some(start) = remaining.find('{') {
                    let Some(end) = remaining[start..].find('}') else { break };
                    visible.push_str(&remaining[..start]);
                    let tag = &remaining[start..=start + end];
                    active_tag = if tag == "{}" { String::new() } else { tag.to_string() };
                    remaining = &remaining[start + end + 1..];
                }
                visible.push_str(remaining);
                // Styled spaces can disappear under a background tag. Preserve
                // spacer rows without styling, retaining the style for later text.
                if visible.trim().is_empty() { " ".to_string() } else { formatted }
            })
            .collect()
    }
}

/// Compute an optional `DisplaySize` from `scale_w` / `scale_h` percentage values.
///
/// Returns `None` when both are effectively 1× (within floating-point epsilon),
/// matching the TypeScript `getDisplaySizeOverride` behaviour.
fn compute_display_size(scale_w: Option<f64>, scale_h: Option<f64>) -> Option<DisplaySize> {
    let w = scale_w.map(|v| v / 100.0).unwrap_or(1.0);
    let h = scale_h.map(|v| v / 100.0).unwrap_or(1.0);
    if (w - 1.0).abs() < 0.0001 && (h - 1.0).abs() < 0.0001 {
        None
    } else {
        Some(DisplaySize { w, h })
    }
}

fn parse_logic_op(operator: &str) -> Option<LogicOp> {
    match operator.trim().to_ascii_lowercase().as_str() {
        "or" => Some(LogicOp::Or),
        "and" => Some(LogicOp::And),
        _ => None,
    }
}

fn compute_rule_flags(rule: &RuleInput) -> (bool, bool) {
    let mut retrigger = false;
    let mut destroy = false;

    for effect in &rule.effects {
        update_rule_flags_from_effect(&effect.effect_type, &mut retrigger, &mut destroy);
    }
    for group in &rule.random_groups {
        for effect in &group.effects {
            update_rule_flags_from_effect(&effect.effect_type, &mut retrigger, &mut destroy);
        }
    }
    for group in &rule.loops {
        for effect in &group.effects {
            update_rule_flags_from_effect(&effect.effect_type, &mut retrigger, &mut destroy);
        }
    }

    (retrigger, destroy)
}

fn update_rule_flags_from_effect(effect_type: &str, retrigger: &mut bool, destroy: &mut bool) {
    match effect_type {
        "retrigger_playing_card" | "retrigger_cards" | "retrigger" => {
            *retrigger = true;
        }
        "destroy_playing_card" | "destroy_card" => {
            *destroy = true;
        }
        _ => {}
    }
}

fn map_appearance(input: &JokerDataInput) -> Option<AppearanceDef> {
    // Custom pool membership is registered through SMODS.ObjectType in
    // main.lua. Turning it into an `in_pool` predicate rejects those same
    // cards because SMODS does not pass the ObjectType key to that callback.
    let appears_in = Vec::new();
    let mut not_appears_in = Vec::new();
    let mut appear_flags = Vec::new();

    if input.appears_in_shop == Some(false) {
        not_appears_in.push("sho".to_string());
    }

    if let Some(flags) = &input.appear_flags {
        for flag in flags.split(',').map(str::trim).filter(|f| !f.is_empty()) {
            appear_flags.push(flag.to_string());
        }
    }

    if appears_in.is_empty() && not_appears_in.is_empty() && appear_flags.is_empty() {
        None
    } else {
        Some(AppearanceDef {
            appears_in,
            not_appears_in,
            appear_flags,
        })
    }
}

fn map_unlock(input: &JokerDataInput) -> Option<UnlockDef> {
    let condition = input
        .unlock_trigger
        .as_ref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())?
        .to_string();

    let description = split_description(
        input
            .unlock_description
            .as_deref()
            .unwrap_or("Unlocked by default."),
    );

    Some(UnlockDef {
        condition,
        description,
    })
}

// ---------------------------------------------------------------------------
// Rust-side package text builders (entry.ts parity)
// ---------------------------------------------------------------------------

fn escape_lua_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

fn normalize_mod_prefixed_key(mod_prefix: &str, object_key: &str) -> String {
    let raw = object_key.trim();
    let prefix = mod_prefix.trim();
    if raw.is_empty() {
        return raw.to_string();
    }
    if prefix.is_empty() || raw.starts_with(&format!("{}_", prefix)) {
        return raw.to_string();
    }
    format!("{}_{}", prefix, raw)
}

fn normalize_class_prefixed_key(mod_prefix: &str, class_prefix: &str, object_key: &str) -> String {
    let raw = object_key.trim();
    let mod_prefix = mod_prefix.trim();
    let class_prefix = class_prefix.trim();
    if raw.is_empty() {
        return raw.to_string();
    }
    if class_prefix.is_empty() {
        return normalize_mod_prefixed_key(mod_prefix, raw);
    }

    let class_prefix_with_sep = format!("{}_", class_prefix);
    if let Some(rest) = raw.strip_prefix(&class_prefix_with_sep) {
        if mod_prefix.is_empty() || rest.starts_with(&format!("{}_", mod_prefix)) {
            return raw.to_string();
        }
        return format!("{}_{}_{}", class_prefix, mod_prefix, rest);
    }

    if mod_prefix.is_empty() {
        format!("{}_{}", class_prefix, raw)
    } else {
        format!("{}_{}_{}", class_prefix, mod_prefix, raw)
    }
}

fn build_lua_string_array(lines: &[String], indent_level: usize) -> String {
    if lines.is_empty() {
        return "{}".to_string();
    }

    let line_indent = "  ".repeat(indent_level + 1);
    let closing_indent = "  ".repeat(indent_level);
    let mut out = String::from("{\n");
    for (idx, line) in lines.iter().enumerate() {
        let escaped = escape_lua_string(line);
        out.push_str(&format!("{}[{}] = '{}'", line_indent, idx + 1, escaped));
        if idx + 1 != lines.len() {
            out.push_str(",\n");
        } else {
            out.push('\n');
        }
    }
    out.push_str(&format!("{}}}", closing_indent));
    out
}

fn split_optional_description(desc: Option<&str>) -> Option<Vec<String>> {
    let raw = desc.unwrap_or("").trim();
    if raw.is_empty() {
        None
    } else {
        Some(split_description(raw))
    }
}

#[derive(Clone)]
struct LocalizationDescriptionEntry {
    name: String,
    text: Vec<String>,
    unlock: Option<Vec<String>>,
}

type LocalizationSetMap = BTreeMap<String, LocalizationDescriptionEntry>;
type LocalizationDescriptions = BTreeMap<String, LocalizationSetMap>;
type LocalizationByLocale = BTreeMap<String, LocalizationDescriptions>;

fn insert_localization_entry(
    locales: &mut LocalizationByLocale,
    locale: &str,
    set: &str,
    key: &str,
    name: &str,
    text: &[String],
    unlock: Option<&Vec<String>>,
) {
    let locale_key = locale.trim();
    if locale_key.is_empty() {
        return;
    }

    let set_map = locales
        .entry(locale_key.to_string())
        .or_default()
        .entry(set.to_string())
        .or_default();

    set_map.insert(
        key.to_string(),
        LocalizationDescriptionEntry {
            name: name.to_string(),
            text: text.to_vec(),
            unlock: unlock.cloned(),
        },
    );
}

fn insert_item_localizations(
    locales: &mut LocalizationByLocale,
    base_locale: &str,
    set: &str,
    key: &str,
    base_name: &str,
    base_description: &str,
    base_unlock: Option<&str>,
    localizations: &[LocalizationEntryInput],
) {
    let base_text = split_description(base_description);
    let base_unlock_lines = split_optional_description(base_unlock);
    insert_localization_entry(
        locales,
        base_locale,
        set,
        key,
        base_name,
        &base_text,
        base_unlock_lines.as_ref(),
    );

    for localization in localizations {
        let locale = localization.language.trim();
        if locale.is_empty() {
            continue;
        }

        let localized_name = if localization.name.trim().is_empty() {
            base_name
        } else {
            localization.name.as_str()
        };
        let localized_description = if localization.description.trim().is_empty() {
            base_description
        } else {
            localization.description.as_str()
        };
        let localized_text = split_description(localized_description);

        insert_localization_entry(
            locales,
            locale,
            set,
            key,
            localized_name,
            &localized_text,
            base_unlock_lines.as_ref(),
        );
    }
}

fn render_localization_lua(
    descriptions: &LocalizationDescriptions,
    dictionary: Option<&BTreeMap<String, String>>,
) -> String {
    let mut out = String::from("return {\n  descriptions = {\n");

    let mut set_iter = descriptions.iter().peekable();
    while let Some((set, entries)) = set_iter.next() {
        out.push_str(&format!("    ['{}'] = {{\n", escape_lua_string(set)));

        let mut entry_iter = entries.iter().peekable();
        while let Some((key, entry)) = entry_iter.next() {
            out.push_str(&format!("      ['{}'] = {{\n", escape_lua_string(key)));
            out.push_str(&format!(
                "        name = '{}',\n",
                escape_lua_string(&entry.name)
            ));
            out.push_str(&format!(
                "        text = {}",
                build_lua_string_array(&entry.text, 4)
            ));

            if let Some(unlock) = &entry.unlock {
                out.push_str(&format!(
                    ",\n        unlock = {}",
                    build_lua_string_array(unlock, 4)
                ));
            }

            out.push_str("\n      }");
            if entry_iter.peek().is_some() {
                out.push_str(",\n");
            } else {
                out.push('\n');
            }
        }

        out.push_str("    }");
        if set_iter.peek().is_some() {
            out.push_str(",\n");
        } else {
            out.push('\n');
        }
    }

    out.push_str("  }");
    if let Some(dictionary) = dictionary.filter(|entries| !entries.is_empty()) {
        out.push_str(",\n  misc = {\n    dictionary = {\n");
        for (key, value) in dictionary {
            out.push_str(&format!(
                "      ['{}'] = '{}',\n",
                escape_lua_string(key),
                escape_lua_string(value)
            ));
        }
        out.push_str("    }\n  }");
    }
    out.push_str("\n}\n");
    out
}

pub fn build_localization_lua_files(
    mod_prefix: &str,
    base_locale: &str,
    jokers: &[BatchJokerEntry],
    consumables: &[BatchConsumableEntry],
    vouchers: &[BatchVoucherEntry],
    decks: &[BatchDeckEntry],
    enhancements: &[BatchEnhancementEntry],
    seals: &[BatchSealEntry],
    editions: &[BatchEditionEntry],
    boosters: &[BatchBoosterEntry],
) -> BTreeMap<String, String> {
    let mut locales: LocalizationByLocale = BTreeMap::new();
    let mut dictionaries: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();

    for entry in jokers {
        let data = &entry.joker_data;
        let key = normalize_class_prefixed_key(mod_prefix, "j", &data.object_key);
        insert_item_localizations(
            &mut locales,
            base_locale,
            "Joker",
            &key,
            &data.name,
            &data.description,
            data.unlock_description.as_deref(),
            &data.localizations,
        );
    }

    for entry in consumables {
        let data = &entry.consumable_data;
        let key = normalize_class_prefixed_key(mod_prefix, "c", &data.object_key);
        insert_item_localizations(
            &mut locales,
            base_locale,
            &data.set,
            &key,
            &data.name,
            &data.description,
            None,
            &data.localizations,
        );
    }

    for entry in vouchers {
        let data = &entry.voucher_data;
        let key = normalize_class_prefixed_key(mod_prefix, "v", &data.object_key);
        insert_item_localizations(
            &mut locales,
            base_locale,
            "Voucher",
            &key,
            &data.name,
            &data.description,
            data.unlock_description.as_deref(),
            &data.localizations,
        );
    }

    for entry in decks {
        let data = &entry.deck_data;
        let key = normalize_class_prefixed_key(mod_prefix, "b", &data.object_key);
        insert_item_localizations(
            &mut locales,
            base_locale,
            "Back",
            &key,
            &data.name,
            &data.description,
            None,
            &data.localizations,
        );
    }

    for entry in enhancements {
        let data = &entry.enhancement_data;
        let key = normalize_class_prefixed_key(mod_prefix, "m", &data.object_key);
        insert_item_localizations(
            &mut locales,
            base_locale,
            "Enhanced",
            &key,
            &data.name,
            &data.description,
            None,
            &data.localizations,
        );
    }

    for entry in seals {
        let data = &entry.seal_data;
        let normalized_key = normalize_mod_prefixed_key(mod_prefix, &data.object_key);
        let key = format!("{}_seal", normalized_key.to_ascii_lowercase());
        insert_item_localizations(
            &mut locales,
            base_locale,
            "Other",
            &key,
            &data.name,
            &data.description,
            None,
            &data.localizations,
        );
    }

    for entry in editions {
        let data = &entry.edition_data;
        let key = normalize_class_prefixed_key(mod_prefix, "e", &data.object_key);
        insert_item_localizations(
            &mut locales,
            base_locale,
            "Edition",
            &key,
            &data.name,
            &data.description,
            None,
            &data.localizations,
        );
    }

    for entry in boosters {
        let data = &entry.booster_data;
        let raw_key = data.object_key.trim();
        let local_key = raw_key.strip_prefix("p_").unwrap_or(raw_key);
        let local_key = if mod_prefix.trim().is_empty() {
            local_key
        } else {
            local_key.strip_prefix(&format!("{}_", mod_prefix.trim())).unwrap_or(local_key)
        };
        let key = normalize_class_prefixed_key(mod_prefix, "p", local_key);
        insert_item_localizations(
            &mut locales,
            base_locale,
            "Other",
            &key,
            &data.name,
            &data.description,
            None,
            &data.localizations,
        );

        // SMODS.Booster's pack opening screen reads this separate dictionary entry.
        let dictionary_key = format!("k_booster_group_{}", key);
        let group_name = data.group_key.as_deref().map(str::trim).filter(|name| !name.is_empty());
        if !base_locale.trim().is_empty() {
            dictionaries.entry(base_locale.trim().to_string()).or_default().insert(
                dictionary_key.clone(),
                group_name.unwrap_or(&data.name).to_string(),
            );
        }
        for localization in &data.localizations {
            let locale = localization.language.trim();
            if locale.is_empty() {
                continue;
            }
            let localized_name = if localization.name.trim().is_empty() {
                &data.name
            } else {
                &localization.name
            };
            dictionaries.entry(locale.to_string()).or_default().insert(
                dictionary_key.clone(),
                group_name.unwrap_or(localized_name).to_string(),
            );
        }
    }

    locales
        .into_iter()
        .map(|(locale, descriptions)| {
            let lua = render_localization_lua(&descriptions, dictionaries.get(&locale));
            (locale, lua)
        })
        .collect()
}

fn param_value_to_lua_literal(value: &ParamValue) -> String {
    match value {
        ParamValue::Int(n) => n.to_string(),
        ParamValue::Float(n) => n.to_string(),
        ParamValue::Bool(b) => {
            if *b {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        ParamValue::Str(s) => format!("'{}'", escape_lua_string(s)),
        ParamValue::Typed(t) => {
            if let Some(n) = t.value.as_i64() {
                return n.to_string();
            }
            if let Some(n) = t.value.as_f64() {
                return n.to_string();
            }
            if let Some(b) = t.value.as_bool() {
                return if b { "true" } else { "false" }.to_string();
            }
            if let Some(s) = t.value.as_str() {
                return format!("'{}'", escape_lua_string(s));
            }
            "nil".to_string()
        }
    }
}

fn register_globals_from_input(
    vars: &[UserVariableInput],
    out: &mut BTreeMap<String, UserVariableDef>,
    persistent_only: bool,
) {
    for var in vars {
        if !var.is_global || var.name.trim().is_empty() {
            continue;
        }
        if persistent_only && !var.is_persistent {
            continue;
        }
        let normalized = var.name.trim().to_ascii_lowercase();
        out.entry(normalized)
            .or_insert_with(|| map_user_variable(var));
    }
}

pub fn collect_global_user_variables(
    jokers: &[BatchJokerEntry],
    consumables: &[BatchConsumableEntry],
    vouchers: &[BatchVoucherEntry],
    decks: &[BatchDeckEntry],
    enhancements: &[BatchEnhancementEntry],
    seals: &[BatchSealEntry],
    editions: &[BatchEditionEntry],
) -> Vec<UserVariableDef> {
    let mut by_name: BTreeMap<String, UserVariableDef> = BTreeMap::new();

    for entry in jokers {
        register_globals_from_input(&entry.joker_data.user_variables, &mut by_name, false);
    }
    for entry in consumables {
        register_globals_from_input(&entry.consumable_data.user_variables, &mut by_name, false);
    }
    for entry in vouchers {
        register_globals_from_input(&entry.voucher_data.user_variables, &mut by_name, false);
    }
    for entry in decks {
        register_globals_from_input(&entry.deck_data.user_variables, &mut by_name, false);
    }
    for entry in enhancements {
        register_globals_from_input(&entry.enhancement_data.user_variables, &mut by_name, false);
    }
    for entry in seals {
        register_globals_from_input(&entry.seal_data.user_variables, &mut by_name, false);
    }
    for entry in editions {
        register_globals_from_input(&entry.edition_data.user_variables, &mut by_name, false);
    }

    by_name.into_values().collect()
}

pub fn collect_persistent_global_user_variables(
    jokers: &[BatchJokerEntry],
    consumables: &[BatchConsumableEntry],
    vouchers: &[BatchVoucherEntry],
    decks: &[BatchDeckEntry],
    enhancements: &[BatchEnhancementEntry],
    seals: &[BatchSealEntry],
    editions: &[BatchEditionEntry],
) -> Vec<UserVariableDef> {
    let mut by_name: BTreeMap<String, UserVariableDef> = BTreeMap::new();

    for entry in jokers {
        register_globals_from_input(&entry.joker_data.user_variables, &mut by_name, true);
    }
    for entry in consumables {
        register_globals_from_input(&entry.consumable_data.user_variables, &mut by_name, true);
    }
    for entry in vouchers {
        register_globals_from_input(&entry.voucher_data.user_variables, &mut by_name, true);
    }
    for entry in decks {
        register_globals_from_input(&entry.deck_data.user_variables, &mut by_name, true);
    }
    for entry in enhancements {
        register_globals_from_input(&entry.enhancement_data.user_variables, &mut by_name, true);
    }
    for entry in seals {
        register_globals_from_input(&entry.seal_data.user_variables, &mut by_name, true);
    }
    for entry in editions {
        register_globals_from_input(&entry.edition_data.user_variables, &mut by_name, true);
    }

    by_name.into_values().collect()
}

pub fn collect_run_scoped_global_user_variables(
    jokers: &[BatchJokerEntry],
    consumables: &[BatchConsumableEntry],
    vouchers: &[BatchVoucherEntry],
    decks: &[BatchDeckEntry],
    enhancements: &[BatchEnhancementEntry],
    seals: &[BatchSealEntry],
    editions: &[BatchEditionEntry],
) -> Vec<UserVariableDef> {
    let mut by_name: BTreeMap<String, UserVariableDef> = BTreeMap::new();

    let mut register_run_scoped = |vars: &[UserVariableInput]| {
        for var in vars {
            if !var.is_global || var.is_persistent || var.name.trim().is_empty() {
                continue;
            }
            let normalized = var.name.trim().to_ascii_lowercase();
            by_name
                .entry(normalized)
                .or_insert_with(|| map_user_variable(var));
        }
    };

    for entry in jokers {
        register_run_scoped(&entry.joker_data.user_variables);
    }
    for entry in consumables {
        register_run_scoped(&entry.consumable_data.user_variables);
    }
    for entry in vouchers {
        register_run_scoped(&entry.voucher_data.user_variables);
    }
    for entry in decks {
        register_run_scoped(&entry.deck_data.user_variables);
    }
    for entry in enhancements {
        register_run_scoped(&entry.enhancement_data.user_variables);
    }
    for entry in seals {
        register_run_scoped(&entry.seal_data.user_variables);
    }
    for entry in editions {
        register_run_scoped(&entry.edition_data.user_variables);
    }

    by_name.into_values().collect()
}

pub fn build_globals_lua(global_vars: &[UserVariableDef]) -> String {
    if global_vars.is_empty() {
        return "return {}\n".to_string();
    }

    let mut lines = Vec::new();
    for var in global_vars {
        lines.push(format!(
            "  ['{}'] = {}",
            escape_lua_string(&var.name),
            param_value_to_lua_literal(&var.initial_value)
        ));
    }

    format!("return {{\n{}\n}}\n", lines.join(",\n"))
}

fn normalize_joker_pool_key(pool: &str, mod_prefix: &str) -> Option<String> {
    let normalized = pool.trim();
    if normalized.is_empty() {
        return None;
    }

    let prefix = mod_prefix.trim();
    if prefix.is_empty() || normalized.starts_with(&format!("{}_", prefix)) {
        Some(normalized.to_string())
    } else {
        Some(format!("{}_{}", prefix, normalized))
    }
}

fn full_joker_center_key(object_key: &str, mod_prefix: &str) -> String {
    let key = object_key.trim();
    let prefix = mod_prefix.trim();
    if !prefix.is_empty() && key.starts_with(&format!("j_{}_", prefix)) {
        key.to_string()
    } else {
        format!("j_{}_{}", prefix, key)
    }
}

fn build_joker_object_types_lua(jokers: &[&BatchJokerEntry], mod_prefix: &str) -> String {
    let mut pools: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let default_pool = normalize_joker_pool_key("jokers", mod_prefix);

    for entry in jokers {
        let joker_key = full_joker_center_key(&entry.joker_data.object_key, mod_prefix);
        if let Some(pool_key) = &default_pool {
            pools
                .entry(pool_key.clone())
                .or_default()
                .push(joker_key.clone());
        }
        for pool in &entry.joker_data.pools {
            if let Some(pool_key) = normalize_joker_pool_key(pool, mod_prefix) {
                pools.entry(pool_key).or_default().push(joker_key.clone());
            }
        }
    }

    if pools.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    for (pool_key, mut joker_keys) in pools {
        joker_keys.sort();
        joker_keys.dedup();

        out.push_str("SMODS.ObjectType({\n");
        out.push_str(&format!("    key = '{}',\n", escape_lua_string(&pool_key)));
        out.push_str("    cards = {\n");
        for joker_key in joker_keys {
            out.push_str(&format!(
                "        ['{}'] = true,\n",
                escape_lua_string(&joker_key)
            ));
        }
        out.push_str("    },\n");
        out.push_str("})\n");
    }

    out
}

pub fn build_main_lua(
    jokers: &[BatchJokerEntry],
    consumables: &[BatchConsumableEntry],
    vouchers: &[BatchVoucherEntry],
    decks: &[BatchDeckEntry],
    enhancements: &[BatchEnhancementEntry],
    seals: &[BatchSealEntry],
    editions: &[BatchEditionEntry],
    mod_prefix: &str,
    boosters: &[BatchBoosterEntry],
    has_mod_icon: bool,
    has_game_logo: bool,
    load_rarities: bool,
    load_consumable_sets: bool,
    load_sounds: bool,
    load_globals: bool,
    disable_vanilla: bool,
    run_scoped_globals: &[UserVariableDef],
) -> String {
    let sorted_jokers: Vec<&BatchJokerEntry> = jokers.iter().collect();

    let sorted_consumables: Vec<&BatchConsumableEntry> = consumables.iter().collect();

    let sorted_vouchers: Vec<&BatchVoucherEntry> = vouchers.iter().collect();

    let sorted_decks: Vec<&BatchDeckEntry> = decks.iter().collect();

    let sorted_enhancements: Vec<&BatchEnhancementEntry> = enhancements.iter().collect();

    let sorted_seals: Vec<&BatchSealEntry> = seals.iter().collect();

    let sorted_editions: Vec<&BatchEditionEntry> = editions.iter().collect();

    let sorted_boosters: Vec<&BatchBoosterEntry> = boosters.iter().collect();

    let mut atlas_decls = String::new();
    if has_mod_icon {
        atlas_decls.push_str("SMODS.Atlas({\n    key = \"modicon\",\n    path = \"ModIcon.png\",\n    px = 34,\n    py = 34,\n    atlas_table = \"ASSET_ATLAS\"\n})\n\n");
    }
    if has_game_logo {
        atlas_decls.push_str("SMODS.Atlas({\n    key = \"balatro\",\n    path = \"balatro.png\",\n    px = 333,\n    py = 216,\n    prefix_config = { key = false },\n    atlas_table = \"ASSET_ATLAS\"\n})\n\n");
    }
    if !sorted_jokers.is_empty() {
        atlas_decls.push_str("SMODS.Atlas({\n    key = \"CustomJokers\",\n    path = \"CustomJokers.png\",\n    px = 71,\n    py = 95,\n    atlas_table = \"ASSET_ATLAS\"\n})\n\n");
    }
    if !sorted_consumables.is_empty() {
        atlas_decls.push_str("SMODS.Atlas({\n    key = \"CustomConsumables\",\n    path = \"CustomConsumables.png\",\n    px = 71,\n    py = 95,\n    atlas_table = \"ASSET_ATLAS\"\n})\n\n");
    }
    if !sorted_enhancements.is_empty() {
        atlas_decls.push_str("SMODS.Atlas({\n    key = \"CustomEnhancements\",\n    path = \"CustomEnhancements.png\",\n    px = 71,\n    py = 95,\n    atlas_table = \"ASSET_ATLAS\"\n})\n\n");
    }
    if !sorted_seals.is_empty() {
        atlas_decls.push_str("SMODS.Atlas({\n    key = \"CustomSeals\",\n    path = \"CustomSeals.png\",\n    px = 71,\n    py = 95,\n    atlas_table = \"ASSET_ATLAS\"\n}):register()\n\n");
    }
    if !sorted_vouchers.is_empty() {
        atlas_decls.push_str("SMODS.Atlas({\n    key = \"CustomVouchers\",\n    path = \"CustomVouchers.png\",\n    px = 71,\n    py = 95,\n    atlas_table = \"ASSET_ATLAS\"\n})\n\n");
    }
    if !sorted_decks.is_empty() {
        atlas_decls.push_str("SMODS.Atlas({\n    key = \"CustomDecks\",\n    path = \"CustomDecks.png\",\n    px = 71,\n    py = 95,\n    atlas_table = \"ASSET_ATLAS\"\n})\n\n");
    }

    if !sorted_boosters.is_empty() {
        atlas_decls.push_str("SMODS.Atlas({\n    key = \"CustomBoosters\",\n    path = \"CustomBoosters.png\",\n    px = 71,\n    py = 95,\n    atlas_table = \"ASSET_ATLAS\"\n})\n\n");
    }

    let mut requires = String::new();
    if load_rarities {
        requires.push_str("assert(SMODS.load_file(\"rarities.lua\"))()\n");
    }
    if load_consumable_sets {
        requires.push_str("assert(SMODS.load_file(\"consumables/sets.lua\"))()\n");
    }
    if load_sounds {
        requires.push_str("assert(SMODS.load_file(\"sounds.lua\"))()\n");
    }
    for j in &sorted_jokers {
        requires.push_str(&format!(
            "assert(SMODS.load_file(\"jokers/{}\"))()\n",
            j.file_name
        ));
    }
    requires.push_str(&build_joker_object_types_lua(&sorted_jokers, mod_prefix));
    for c in &sorted_consumables {
        requires.push_str(&format!(
            "assert(SMODS.load_file(\"consumables/{}\"))()\n",
            c.file_name
        ));
    }
    for e in &sorted_enhancements {
        requires.push_str(&format!(
            "assert(SMODS.load_file(\"enhancements/{}\"))()\n",
            e.file_name
        ));
    }
    for s in &sorted_seals {
        requires.push_str(&format!(
            "assert(SMODS.load_file(\"seals/{}\"))()\n",
            s.file_name
        ));
    }
    for ed in &sorted_editions {
        requires.push_str(&format!(
            "assert(SMODS.load_file(\"editions/{}\"))()\n",
            ed.file_name
        ));
    }
    for v in &sorted_vouchers {
        requires.push_str(&format!(
            "assert(SMODS.load_file(\"vouchers/{}\"))()\n",
            v.file_name
        ));
    }
    for d in &sorted_decks {
        requires.push_str(&format!(
            "assert(SMODS.load_file(\"decks/{}\"))()\n",
            d.file_name
        ));
    }

    for booster in &sorted_boosters {
        requires.push_str(&format!(
            "assert(SMODS.load_file(\"boosters/{}\"))()\n",
            booster.file_name
        ));
    }

    let globals_load = if load_globals {
        "local jf_global_defaults = assert(SMODS.load_file(\"globals.lua\"))()\n\
local jf_profile = (G.PROFILES and G.SETTINGS and G.SETTINGS.profile and G.PROFILES[G.SETTINGS.profile]) or nil\n\
if jf_profile then\n\
    jf_profile.jf_global_vars = jf_profile.jf_global_vars or {}\n\
    local jf_profile_globals = jf_profile.jf_global_vars\n\
    local jf_defaults_applied = false\n\
    for k, v in pairs(jf_global_defaults or {}) do\n\
        if jf_profile_globals[k] == nil then\n\
            jf_profile_globals[k] = v\n\
            jf_defaults_applied = true\n\
        end\n\
    end\n\
    if jf_defaults_applied and G and G.save_progress then\n\
        G:save_progress()\n\
    end\n\
    JF_GLOBALS = setmetatable({}, {\n\
        __index = jf_profile_globals,\n\
        __newindex = function(_, key, value)\n\
            jf_profile_globals[key] = value\n\
            if G and G.save_progress then\n\
                G:save_progress()\n\
            end\n\
        end,\n\
    })\n\
else\n\
    JF_GLOBALS = JF_GLOBALS or {}\n\
    for k, v in pairs(jf_global_defaults or {}) do\n\
        if JF_GLOBALS[k] == nil then\n\
            JF_GLOBALS[k] = v\n\
        end\n\
    end\n\
end\n"
    } else {
        "JF_GLOBALS = JF_GLOBALS or {}\n"
    };

    let game_globals_reset = if run_scoped_globals.is_empty() && !disable_vanilla {
        String::new()
    } else {
        let mut body = String::new();
        if disable_vanilla {
            body.push_str(
                "    for k, v in pairs(G.P_CENTERS) do\n\
        if v.set == 'Joker' and not v.mod then\n\
            G.GAME.banned_keys[k] = true\n\
        end\n\
    end\n",
            );
        }
        if !run_scoped_globals.is_empty() {
            body.push_str(
                "    G.GAME.jf_global_vars = G.GAME.jf_global_vars or {}\n\
    local jf_run_globals = G.GAME.jf_global_vars\n",
            );
        }
        for variable in run_scoped_globals {
            body.push_str(&format!(
                "    if run_start or jf_run_globals['{name}'] == nil then\n\
        jf_run_globals['{name}'] = {initial}\n\
    end\n",
                name = escape_lua_string(&variable.name),
                initial = param_value_to_lua_literal(&variable.initial_value)
            ));
        }
        format!(
            "SMODS.current_mod = SMODS.current_mod or {{}}\n\
SMODS.current_mod.reset_game_globals = function(run_start)\n\
    if not G or not G.GAME then return end\n\
{}end\n",
            body
        )
    };

    // Steamodded only dispatches post-trigger contexts when a loaded mod opts in.
    let optional_features = if jokers.iter().any(|entry| {
        entry.joker_data.rules.iter().any(|rule| rule.trigger == "joker_triggered")
    }) {
        "SMODS.current_mod.optional_features = SMODS.current_mod.optional_features or {}\n\
SMODS.current_mod.optional_features.post_trigger = true\n"
    } else {
        ""
    };

    format!(
        "{}local NFS = require(\"nativefs\")\nto_big = to_big or function(a) return a end\nlenient_bignum = lenient_bignum or function(a) return a end\n{}\n{}{}{}\n",
        atlas_decls, globals_load, game_globals_reset, optional_features, requires
    )
}

pub fn build_sounds_lua(sounds: &[SoundDataInput]) -> String {
    let mut sorted_sounds: Vec<&SoundDataInput> = sounds.iter().collect();
    sorted_sounds.sort_by(|a, b| {
        a.key
            .trim()
            .to_ascii_lowercase()
            .cmp(&b.key.trim().to_ascii_lowercase())
    });

    let mut lines: Vec<String> = Vec::new();
    for sound in sorted_sounds {
        let key = sound.key.trim();
        if key.is_empty() {
            continue;
        }
        let Some(path) = Path::new(sound.sound_string.trim())
            .file_name()
            .and_then(|name| name.to_str())
        else {
            continue;
        };
        let mut block = vec![
            "SMODS.Sound({".to_string(),
            format!("    key = '{}',", escape_lua_string(key)),
            // SMODS resolves this path relative to the mod's assets/sounds directory.
            format!("    path = '{}',", escape_lua_string(path)),
            format!("    pitch = {},", sound.pitch.unwrap_or(1.0)),
            format!("    volume = {},", sound.volume.unwrap_or(1.0)),
        ];
        if let Some(replace) = sound.replace.as_deref() {
            let replace = replace.trim();
            if !replace.is_empty() {
                block.push(format!("    replace = '{}',", escape_lua_string(replace)));
            }
        }
        block.push("})".to_string());
        lines.push(block.join("\n"));
    }

    lines.join("\n\n")
}

#[derive(Serialize)]
struct ModJsonPayload<'a> {
    id: &'a str,
    name: &'a str,
    display_name: &'a str,
    author: &'a [String],
    description: &'a str,
    prefix: &'a str,
    main_file: &'a str,
    version: &'a str,
    priority: i64,
    badge_colour: &'a str,
    badge_text_colour: &'a str,
    dependencies: &'a [String],
    conflicts: &'a [String],
    provides: &'a [String],
}

pub fn build_mod_json(metadata: &ModMetadataInput) -> Result<String, String> {
    let payload = ModJsonPayload {
        id: &metadata.id,
        name: &metadata.name,
        display_name: &metadata.display_name,
        author: &metadata.author,
        description: &metadata.description,
        prefix: &metadata.prefix,
        main_file: &metadata.main_file,
        version: &metadata.version,
        priority: metadata.priority,
        badge_colour: &metadata.badge_colour,
        badge_text_colour: &metadata.badge_text_colour,
        dependencies: &metadata.dependencies,
        conflicts: &metadata.conflicts,
        provides: &metadata.provides,
    };

    serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("Failed to serialize mod metadata: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_effect_message_settings_survive_frontend_mapping() {
        for effect_type in ["edit_hand_size", "edit_play_size", "edit_discard_size"] {
            let input: EffectInput = serde_json::from_value(serde_json::json!({
                "type": effect_type, "customMessage": "Let's play!", "messageMode": "none",
                "params": {
                    "value": {"value": 2},
                    "customMessage": {"value": "Older message"},
                    "messageMode": {"value": "custom"}
                }
            })).unwrap();
            let effect = map_effect(&input);
            assert_eq!(effect.params["customMessage"].as_str(), Some("Let's play!"));
            assert_eq!(effect.params["messageMode"].as_str(), Some("none"));
            assert_eq!(effect.params["value"].as_i64(), Some(2));
        }
    }

    #[test]
    fn size_effect_message_mapping_preserves_old_projects_and_other_effects() {
        let legacy: EffectInput = serde_json::from_value(serde_json::json!({
            "type": "edit_hand_size", "params": {"customMessage": {"value": "Legacy"}}
        })).unwrap();
        let mapped = map_effect(&legacy);
        assert_eq!(mapped.params["customMessage"].as_str(), Some("Legacy"));
        assert!(!mapped.params.contains_key("messageMode"));
        let default: EffectInput = serde_json::from_value(serde_json::json!({
            "type": "edit_play_size", "params": {"value": {"value": 1}}
        })).unwrap();
        assert_eq!(map_effect(&default).params.len(), 1);
        let unrelated: EffectInput = serde_json::from_value(serde_json::json!({
            "type": "add_mult", "customMessage": "New", "messageMode": "none",
            "params": {"customMessage": {"value": "Existing"}}
        })).unwrap();
        let mapped = map_effect(&unrelated);
        assert_eq!(mapped.params["customMessage"].as_str(), Some("Existing"));
        assert!(!mapped.params.contains_key("messageMode"));
    }

    #[test]
    fn description_line_breaks_preserve_each_blank_row() {
        for description in [
            "First\n\n\nLast",
            "First\r\n\r\n\r\nLast",
            "First[s][s][s]Last",
            "First<br><br/><br />Last",
            "First<BR><BR/><BR />Last",
            "First\n \n\t\nLast",
        ] {
            assert_eq!(
                split_description(description),
                vec!["First", " ", " ", "Last"],
                "description {description:?}"
            );
        }
        assert_eq!(split_description("\nFirst\n"), vec![" ", "First", " "]);
        assert_eq!(split_description("  First  \n  Last  "), vec!["First", "Last"]);
        for description in ["", " \t", "\n\n", "[s][s]", "<br><br />"] {
            assert_eq!(split_description(description), vec!["No description"]);
        }
    }

    #[test]
    fn description_formatting_continues_until_replaced_or_reset() {
        for description in [
            "{C:red}First[s]Second[s]Third{}[s]Plain",
            "{C:red}First\nSecond\nThird{}\nPlain",
            "{C:red}First\r\nSecond\r\nThird{}\r\nPlain",
            "{C:red}First<br/>Second<br>Third{}<br />Plain",
        ] {
            assert_eq!(split_description(description), vec![
                "{C:red}First", "{C:red}Second", "{C:red}Third{}", "Plain"
            ], "description {description:?}");
        }
        assert_eq!(split_description("{C:red,E:1,s:1.1}First[s]Second{}"),
            vec!["{C:red,E:1,s:1.1}First", "{C:red,E:1,s:1.1}Second{}"]);
        assert_eq!(split_description("{C:red}First[s]{E:2}Second[s]Third[s]{}Plain"),
            vec!["{C:red}First", "{E:2}Second", "{E:2}Third", "{}Plain"]);
        assert_eq!(split_description("{C:red}{s:1.2}First[s]Second"),
            vec!["{C:red}{s:1.2}First", "{s:1.2}Second"]);
        assert_eq!(split_description("{X:red,C:white}First[s][s]Last{}"),
            vec!["{X:red,C:white}First", " ", "{X:red,C:white}Last{}"]);
        assert_eq!(split_description("{C:red}[s]First[s]{E:1}[s]Second[s]{}[s]Plain"),
            vec![" ", "{C:red}First", " ", "{E:1}Second", " ", "Plain"]);
    }

    #[test]
    fn joker_description_spacing_survives_inline_and_localization_exports() {
        let entry: BatchJokerEntry = serde_json::from_value(serde_json::json!({
            "jokerData": {
                "objectKey": "spaced", "name": "Spaced", "cost": 4, "rarity": "common",
                "description": "First[s][s][s]Last",
                "localizations": [{"language": "fr", "name": "Espacé", "description": "Premier\n\n\nDernier"}]
            },
            "pos": {"x": 0, "y": 0}, "fileName": "spaced.lua"
        })).unwrap();
        let definition = joker_data_to_def(&entry.joker_data, "mod", entry.pos.clone(), None);
        assert_eq!(definition.description, vec!["First", " ", " ", "Last"]);
        let inline = balatro_codegen::Emitter::new().emit_chunk(&balatro_codegen::compile_joker(&definition, "mod"));
        assert!(inline.contains("[2] = ' '") && inline.contains("[3] = ' '"), "{inline}");
        let localized = build_localization_lua_files(
            "mod", "en-us", &[entry], &[], &[], &[], &[], &[], &[], &[],
        );
        for (locale, first, last) in [("en-us", "First", "Last"), ("fr", "Premier", "Dernier")] {
            let lua = &localized[locale];
            let expected = format!("[1] = '{first}',\n          [2] = ' ',\n          [3] = ' ',\n          [4] = '{last}'");
            assert!(lua.contains(&expected), "{lua}");
        }
    }

    fn make_booster_entry() -> BatchBoosterEntry {
        serde_json::from_value(serde_json::json!({
            "boosterData": {
                "objectKey": "new_pack",
                "name": "Custom Pack",
                "description": "Choose #1# of #2# cards.<br/>A custom pack.",
                "booster_type": "playing_card",
                "config": { "extra": 5, "choose": 2 },
                "cost": 6,
                "weight": 0.75,
                "draw_hand": true,
                "instant_use": false,
                "discovered": true,
                "hidden": false,
                "kind": "Custom",
                "background_colour": "102030",
                "special_colour": "#405060",
                "card_rules": [{
                    "weight": 3,
                    "rank": "Ace",
                    "suit": "Spades",
                    "enhancement": "m_glass",
                    "edition": "e_foil",
                    "seal": "Red"
                }],
                "localizations": [{
                    "language": "fr",
                    "name": "Paquet personnalisé",
                    "description": "Choisissez #1# cartes parmi #2#."
                }]
            },
            "pos": { "x": 3, "y": 2 },
            "fileName": "new_pack.lua"
        })).expect("saved frontend Booster data should deserialize")
    }

    #[test]
    fn booster_mapping_keeps_pack_counts_and_card_contents() {
        let entry = make_booster_entry();
        let def = booster_data_to_def(&entry.booster_data, entry.pos);
        assert_eq!(def.atlas, "CustomBoosters");
        assert_eq!((def.pos.x, def.pos.y), (3, 2));
        assert_eq!(def.extra, Some(5));
        assert_eq!(def.choose, Some(2));
        assert_eq!(def.booster_type, "playing_card");
        assert_eq!(def.cost, Some(6));
        assert_eq!(def.weight, Some(0.75));
        assert_eq!(def.draw_hand, Some(true));
        assert_eq!(def.instant_use, Some(false));
        assert_eq!(def.description, vec!["Choose #1# of #2# cards.", "A custom pack."]);
        assert_eq!(def.card_rules.len(), 1);
        assert_eq!(def.card_rules[0].weight, 3.0);
        assert_eq!(def.card_rules[0].rank.as_deref(), Some("Ace"));
        assert_eq!(def.card_rules[0].suit.as_deref(), Some("Spades"));
        assert_eq!(def.card_rules[0].enhancement.as_deref(), Some("m_glass"));
        assert_eq!(def.card_rules[0].edition.as_deref(), Some("e_foil"));
        assert_eq!(def.card_rules[0].seal.as_deref(), Some("Red"));
        assert_eq!(def.background_colour.as_deref(), Some("102030"));
        assert_eq!(def.special_colour.as_deref(), Some("#405060"));
    }

    #[test]
    fn booster_mapping_accepts_old_projects_without_optional_pack_settings() {
        let input: BoosterDataInput = serde_json::from_value(serde_json::json!({
            "objectKey": "old_pack", "name": "Old Pack", "description": "Pack"
        })).unwrap();
        let def = booster_data_to_def(&input, AtlasPosInput { x: 0, y: 0 });
        assert_eq!(def.booster_type, "joker");
        assert_eq!(def.extra, None);
        assert_eq!(def.choose, None);
        assert!(def.card_rules.is_empty());
    }

    #[test]
    fn booster_export_registers_atlas_and_loads_the_pack_file() {
        let boosters = vec![make_booster_entry()];
        let lua = build_main_lua(
            &[], &[], &[], &[], &[], &[], &[], "mod", &boosters,
            false, false, false, false, false, false, false, &[],
        );
        let atlas = lua.find("key = \"CustomBoosters\"").expect("pack atlas must be registered");
        let load = lua.find("assert(SMODS.load_file(\"boosters/new_pack.lua\"))()").expect("pack file must be loaded");
        assert!(atlas < load);
        assert!(lua.contains("path = \"CustomBoosters.png\""));
        assert!(lua.contains("px = 71,\n    py = 95"));
    }

    #[test]
    fn booster_custom_pool_matches_the_project_joker_pool_registration() {
        for pool in ["custom_pool", "mod_custom_pool"] {
            let jokers = vec![make_joker_entry_with_pools(vec![], vec![pool.to_string()])];
            let mut booster = make_booster_entry();
            booster.booster_data.booster_type = "joker".to_string();
            booster.booster_data.card_rules = serde_json::from_value(serde_json::json!([
                { "pool": pool, "weight": 1 }
            ])).unwrap();
            let boosters = vec![booster];
            let main = build_main_lua(
                &jokers, &[], &[], &[], &[], &[], &[], "mod", &boosters,
                false, false, false, false, false, false, false, &[],
            );
            let def = booster_data_to_def(&boosters[0].booster_data, boosters[0].pos.clone());
            let chunk = balatro_codegen::compile_booster(&def, "mod");
            let pack = balatro_codegen::Emitter::new().emit_chunk(&chunk);
            let pack_pool = pack.lines().find_map(|line| {
                line.trim().strip_prefix("set = '").and_then(|value| value.strip_suffix("',"))
            }).expect("Joker Booster must create cards from a pool");

            assert_eq!(pack_pool, "mod_custom_pool", "pool input {pool}");
            assert!(main.contains(&format!("key = '{}',", pack_pool)),
                "Booster pool must be registered in the same mod: {main}");
            assert!(main.contains("['j_mod_test'] = true"));
            assert!(main.find("key = 'mod_custom_pool'").unwrap()
                < main.find("boosters/new_pack.lua").unwrap());
        }
    }

    #[test]
    fn booster_export_localizes_descriptions_and_the_pack_opening_title() {
        let mut boosters = vec![make_booster_entry()];
        let localized = build_localization_lua_files(
            "mod", "en-us", &[], &[], &[], &[], &[], &[], &[], &boosters,
        );
        let english = &localized["en-us"];
        let french = &localized["fr"];
        assert!(english.contains("['Other']"));
        assert!(english.contains("['p_mod_new_pack']"));
        assert!(!english.contains("['Booster']"));
        assert!(english.contains("['k_booster_group_p_mod_new_pack'] = 'Custom Pack'"));
        assert!(french.contains("name = 'Paquet personnalisé'"));
        assert!(french.contains("Choisissez #1# cartes parmi #2#."));
        assert!(french.contains("['k_booster_group_p_mod_new_pack'] = 'Paquet personnalisé'"));

        // Keys pasted from SMODS references resolve to the same local registration key.
        for object_key in ["p_new_pack", "mod_new_pack", "p_mod_new_pack"] {
            boosters[0].booster_data.object_key = object_key.to_string();
            let localized = build_localization_lua_files(
                "mod", "en-us", &[], &[], &[], &[], &[], &[], &[], &boosters,
            );
            assert!(localized["en-us"].contains("['p_mod_new_pack']"));
            assert!(localized["en-us"].contains("['k_booster_group_p_mod_new_pack']"));
        }

        boosters[0].booster_data.group_key = Some("Custom ' Group".to_string());
        let localized = build_localization_lua_files(
            "mod", "en-us", &[], &[], &[], &[], &[], &[], &[], &boosters,
        );
        assert!(localized["en-us"].contains("['k_booster_group_p_mod_new_pack'] = 'Custom \\' Group'"));
        assert!(localized["fr"].contains("['k_booster_group_p_mod_new_pack'] = 'Custom \\' Group'"));
    }

    fn make_sound(key: &str, filename: &str) -> SoundDataInput {
        SoundDataInput {
            key: key.to_string(),
            sound_string: filename.to_string(),
            audio_bytes: Some(vec![1, 2, 3]),
            volume: None,
            pitch: None,
            replace: None,
        }
    }

    #[test]
    fn build_sounds_lua_resolves_upload_in_smods_assets_directory() {
        // Imported filenames may include a directory, but export packages only the basename.
        for filename in ["test.ogg", "sounds/test.ogg", " assets/sounds/test.ogg "] {
            let lua = build_sounds_lua(&[make_sound("test", filename)]);
            let registered_path = lua
                .lines()
                .find_map(|line| {
                    line.trim()
                        .strip_prefix("path = '")
                        .and_then(|path| path.strip_suffix("',"))
                })
                .expect("sound registration must include a path");

            // This is the location that SMODS.Sound.inject reads when the sound is played.
            let smods_path = Path::new("mod/assets/sounds").join(registered_path);
            assert_eq!(smods_path, Path::new("mod/assets/sounds/test.ogg"));
        }
    }

    #[test]
    fn build_sounds_lua_preserves_registration_key_and_playback_settings() {
        let mut sound = make_sound(" test ", "test.ogg");
        sound.pitch = Some(1.25);
        sound.volume = Some(0.5);
        sound.replace = Some(" card1 ".to_string());

        let lua = build_sounds_lua(&[sound]);

        assert!(lua.contains("key = 'test',"));
        // Keep SMODS's default key prefixing enabled for project sounds.
        assert!(!lua.contains("prefix_config"));
        assert!(lua.contains("pitch = 1.25,"));
        assert!(lua.contains("volume = 0.5,"));
        assert!(lua.contains("replace = 'card1',"));
    }

    #[test]
    fn build_sounds_lua_skips_unconfigured_sounds() {
        let lua = build_sounds_lua(&[
            make_sound("", "test.ogg"),
            make_sound("blank", " "),
            make_sound("ready", "ready.ogg"),
        ]);

        assert_eq!(lua.matches("SMODS.Sound({").count(), 1);
        assert!(lua.contains("key = 'ready',"));
        assert!(lua.contains("path = 'ready.ogg',"));
        assert!(lua.contains("pitch = 1,"));
        assert!(lua.contains("volume = 1,"));
    }

    fn make_global_var(name: &str, is_persistent: bool) -> UserVariableInput {
        UserVariableInput {
            name: name.to_string(),
            var_type: "number".to_string(),
            is_global: true,
            is_persistent,
            initial_value: Some(1.0),
            initial_suit: None,
            initial_rank: None,
            initial_poker_hand: None,
            initial_key: None,
            initial_text: None,
        }
    }

    fn make_joker_entry(vars: Vec<UserVariableInput>) -> BatchJokerEntry {
        make_joker_entry_with_pools(vars, vec![])
    }

    fn make_joker_entry_with_pools(
        vars: Vec<UserVariableInput>,
        pools: Vec<String>,
    ) -> BatchJokerEntry {
        BatchJokerEntry {
            joker_data: JokerDataInput {
                object_key: "test".to_string(),
                name: "Test Joker".to_string(),
                description: "Test".to_string(),
                localizations: vec![],
                cost: 4,
                rarity: serde_json::json!("common"),
                blueprint_compat: Some(true),
                eternal_compat: Some(true),
                perishable_compat: Some(true),
                unlocked: Some(true),
                discovered: Some(true),
                scale_w: None,
                scale_h: None,
                rules: vec![],
                user_variables: vars,
                description_variables: None,
                force_eternal: false,
                force_perishable: false,
                force_rental: false,
                force_foil: false,
                force_holographic: false,
                force_polychrome: false,
                force_negative: false,
                ignore_slot_limit: false,
                info_queues: vec![],
                pools,
                appears_in_shop: Some(true),
                appear_flags: None,
                unlock_trigger: None,
                unlock_description: None,
            },
            pos: AtlasPosInput { x: 0, y: 0 },
            soul_pos: None,
            file_name: "j_test.lua".to_string(),
            custom_lua: None,
        }
    }

    #[test]
    fn wrapped_param_input_accepts_wrapped_shape() {
        let parsed: WrappedParamInput =
            serde_json::from_str(r#"{"value": 7, "valueType": "number"}"#)
                .expect("wrapped param should deserialize");

        assert_eq!(parsed.value, serde_json::json!(7));
        assert_eq!(parsed.value_type.as_deref(), Some("number"));
    }

    #[test]
    fn ordered_description_bindings_survive_frontend_mapping() {
        let input: JokerDataInput = serde_json::from_value(serde_json::json!({
            "objectKey": "chance", "name": "Chance", "description": "#1# in #2#", "cost": 4, "rarity": "common",
            "descriptionVariables": [
                { "kind": "probability", "group_id": "chance-group", "part": "numerator" },
                { "kind": "probability", "group_id": "chance-group", "part": "denominator" },
                { "kind": "literal", "value": 2 },
                { "kind": "literal", "value": 2 }
            ]
        })).unwrap();
        let def = joker_data_to_def(&input, "mod", AtlasPosInput { x: 0, y: 0 }, None);
        let bindings = def.description_variables.expect("bindings must reach the generator");
        assert_eq!(bindings.len(), 4);
        assert!(matches!(&bindings[0], DescriptionVariableBinding::Probability { group_id, part: balatro_codegen::types::ProbabilityPart::Numerator } if group_id == "chance-group"));
        assert!(matches!(&bindings[1], DescriptionVariableBinding::Probability { group_id, part: balatro_codegen::types::ProbabilityPart::Denominator } if group_id == "chance-group"));
        assert!(matches!(&bindings[2], DescriptionVariableBinding::Literal { value } if value == &serde_json::json!(2)));
        assert!(matches!(&bindings[3], DescriptionVariableBinding::Literal { value } if value == &serde_json::json!(2)));
    }

    #[test]
    fn game_variable_descriptions_and_loops_survive_frontend_mapping() {
        let input: JokerDataInput = serde_json::from_value(serde_json::json!({
            "objectKey": "dynamic", "name": "Dynamic", "description": "#1#", "cost": 4, "rarity": "common",
            "descriptionVariables": [
                { "kind": "game", "id": "joker_count", "multiplier": 2, "startsFrom": 3 }
            ],
            "rules": [{"id": "loop", "trigger": "hand_played", "loops": [{
                "id": "repeat", "repetitions": { "value": "GAMEVAR:joker_count|2|3", "valueType": "game_var" },
                "effects": [{ "id": "money", "type": "set_dollars", "params": {
                    "value": { "value": "GAMEVAR:joker_count|2|3", "valueType": "gameVariable" }
                }}]
            }]}]
        })).unwrap();
        let def = joker_data_to_def(&input, "mod", AtlasPosInput { x: 0, y: 0 }, None);
        let code = balatro_codegen::Emitter::new().emit_chunk(&balatro_codegen::compile_joker(&def, "mod"));
        assert!(matches!(&def.description_variables.as_ref().unwrap()[0],
            DescriptionVariableBinding::Game { id, multiplier, starts_from }
                if id == "joker_count" && *multiplier == 2.0 && *starts_from == 3.0));
        assert!(matches!(&def.rules[0].loop_groups[0].count,
            ParamValue::Typed(value) if value.value_type == "game_var" && value.value == "GAMEVAR:joker_count|2|3"));
        assert!(code.contains("for i = 1, math.max(0, math.floor(tonumber(3 +"), "{code}");
        assert!(code.contains("G.jokers.cards"), "{code}");
        assert!(code.contains("dollars = 3 +"), "{code}");
    }

    #[test]
    fn wrapped_param_input_accepts_raw_number_shape() {
        let parsed: WrappedParamInput =
            serde_json::from_str("12").expect("raw number param should deserialize");

        assert_eq!(parsed.value, serde_json::json!(12));
        assert_eq!(parsed.value_type, None);
    }

    #[test]
    fn wrapped_param_input_accepts_raw_string_shape() {
        let parsed: WrappedParamInput =
            serde_json::from_str(r#""hello""#).expect("raw string param should deserialize");

        assert_eq!(parsed.value, serde_json::json!("hello"));
        assert_eq!(parsed.value_type, None);
    }

    #[test]
    fn mixed_wrapped_and_raw_params_map_correctly() {
        let effect: EffectInput = serde_json::from_value(serde_json::json!({
            "type": "add_chips",
            "params": {
                "raw_num": 5,
                "wrapped_num": { "value": 10 },
                "wrapped_typed": { "value": "$money", "valueType": "game_variable" }
            }
        }))
        .expect("effect should deserialize with mixed param shapes");

        let mapped = map_params(&effect.params);

        match mapped.get("raw_num") {
            Some(ParamValue::Int(5)) => {}
            other => panic!("expected raw_num Int(5), got {other:?}"),
        }

        match mapped.get("wrapped_num") {
            Some(ParamValue::Int(10)) => {}
            other => panic!("expected wrapped_num Int(10), got {other:?}"),
        }

        match mapped.get("wrapped_typed") {
            Some(ParamValue::Typed(tv)) => {
                assert_eq!(tv.value, serde_json::json!("$money"));
                assert_eq!(tv.value_type, "game_variable");
            }
            other => panic!("expected wrapped_typed Typed(...), got {other:?}"),
        }
    }

    #[test]
    fn normalize_rarity_preserves_vanilla_string_rarity() {
        let rarity = normalize_rarity(&serde_json::json!("rare"), "jkr");
        assert_eq!(rarity, "rare");
    }

    #[test]
    fn normalize_rarity_prefixes_custom_rarity() {
        let rarity = normalize_rarity(&serde_json::json!("superrare"), "jkr");
        assert_eq!(rarity, "jkr_superrare");
    }

    #[test]
    fn normalize_rarity_does_not_double_prefix_custom_rarity() {
        let rarity = normalize_rarity(&serde_json::json!("jkr_superrare"), "jkr");
        assert_eq!(rarity, "jkr_superrare");
    }

    #[test]
    fn deck_data_to_def_prefixes_atlas_key() {
        let input = DeckDataInput {
            object_key: "new_deck".to_string(),
            name: "New Deck".to_string(),
            description: "desc".to_string(),
            localizations: vec![],
            rules: vec![],
            user_variables: vec![],
            description_variables: None,
            unlocked: Some(true),
            discovered: Some(true),
            no_collection: None,
            config_vouchers: vec![],
            config_consumables: vec![],
            no_interest: false,
            no_faces: false,
            erratic_deck: false,
            atlas: Some("Enhancers".to_string()),
        };

        let def = deck_data_to_def(&input, "new_proj", AtlasPosInput { x: 0, y: 0 });
        assert_eq!(def.atlas, "CustomDecks");
    }

    #[test]
    fn collect_global_user_variables_includes_non_persistent_globals() {
        let jokers = vec![
            make_joker_entry(vec![make_global_var("global_non_persistent", false)]),
            make_joker_entry(vec![make_global_var("global_persistent", true)]),
        ];

        let globals = collect_global_user_variables(&jokers, &[], &[], &[], &[], &[], &[]);

        let names: Vec<&str> = globals.iter().map(|value| value.name.as_str()).collect();
        assert!(names.contains(&"global_non_persistent"));
        assert!(names.contains(&"global_persistent"));
    }

    #[test]
    fn collect_persistent_global_user_variables_filters_non_persistent() {
        let jokers = vec![
            make_joker_entry(vec![make_global_var("global_non_persistent", false)]),
            make_joker_entry(vec![make_global_var("global_persistent", true)]),
        ];

        let globals =
            collect_persistent_global_user_variables(&jokers, &[], &[], &[], &[], &[], &[]);

        let names: Vec<&str> = globals.iter().map(|value| value.name.as_str()).collect();
        assert!(!names.contains(&"global_non_persistent"));
        assert!(names.contains(&"global_persistent"));
    }

    #[test]
    fn collect_run_scoped_global_user_variables_only_includes_non_persistent() {
        let jokers = vec![
            make_joker_entry(vec![make_global_var("global_non_persistent", false)]),
            make_joker_entry(vec![make_global_var("global_persistent", true)]),
        ];

        let globals =
            collect_run_scoped_global_user_variables(&jokers, &[], &[], &[], &[], &[], &[]);

        let names: Vec<&str> = globals.iter().map(|value| value.name.as_str()).collect();
        assert!(names.contains(&"global_non_persistent"));
        assert!(!names.contains(&"global_persistent"));
    }

    #[test]
    fn build_main_lua_enables_post_trigger_for_any_exported_joker() {
        let mut triggered_joker = make_joker_entry(vec![]);
        triggered_joker.joker_data.rules = serde_json::from_value(serde_json::json!([
            { "id": "triggered", "trigger": "joker_triggered", "effects": [
                { "id": "mult", "type": "add_mult", "params": { "value": 2 } }
            ] }
        ])).unwrap();
        let jokers = vec![make_joker_entry(vec![]), triggered_joker];
        let lua = build_main_lua(
            &jokers, &[], &[], &[], &[], &[], &[], "mod", &[],
            false, false, false, false, false, false, false, &[],
        );

        let feature = "SMODS.current_mod.optional_features.post_trigger = true";
        assert!(lua.contains("SMODS.current_mod.optional_features = SMODS.current_mod.optional_features or {}"));
        assert_eq!(lua.matches(feature).count(), 1);
        assert!(lua.find(feature).unwrap() < lua.find("jokers/j_test.lua").unwrap());
    }

    #[test]
    fn build_main_lua_leaves_post_trigger_disabled_without_a_triggered_rule() {
        for trigger in [None, Some("joker_evaluated"), Some("hand_played")] {
            let mut joker = make_joker_entry(vec![]);
            if let Some(trigger) = trigger {
                joker.joker_data.rules = serde_json::from_value(serde_json::json!([
                    { "id": "other", "trigger": trigger, "effects": [
                        { "id": "mult", "type": "add_mult", "params": { "value": 2 } }
                    ] }
                ])).unwrap();
            }
            let lua = build_main_lua(
                &[joker], &[], &[], &[], &[], &[], &[], "mod", &[],
                false, false, false, false, false, false, false, &[],
            );
            assert!(!lua.contains("optional_features"), "trigger {trigger:?}");
        }
        let empty = build_main_lua(
            &[], &[], &[], &[], &[], &[], &[], "mod", &[],
            false, false, false, false, false, false, false, &[],
        );
        assert!(!empty.contains("optional_features"));
    }

    #[test]
    fn build_main_lua_registers_reset_game_globals_for_run_scoped_globals() {
        let run_scoped = vec![UserVariableDef {
            name: "global_non_persistent".to_string(),
            var_type: UserVarType::Number,
            initial_value: ParamValue::Int(7),
            is_global: true,
            is_persistent: false,
        }];

        let lua = build_main_lua(
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            "mod",
            &[],
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            &run_scoped,
        );

        assert!(lua.contains("SMODS.current_mod.reset_game_globals = function(run_start)"));
        assert!(lua.contains("G.GAME.jf_global_vars = G.GAME.jf_global_vars or {}"));
        assert!(lua.contains("if run_start or jf_run_globals['global_non_persistent'] == nil then"));
        assert!(lua.contains("jf_run_globals['global_non_persistent'] = 7"));
    }

    #[test]
    fn persistent_globals_initialize_missing_values_without_registering_a_round_reset() {
        let lua = build_main_lua(
            &[], &[], &[], &[], &[], &[], &[], "mod", &[],
            false, false, false, false, false, true, false, &[],
        );
        assert!(lua.contains("if jf_profile_globals[k] == nil then"));
        assert!(lua.contains("if JF_GLOBALS[k] == nil then"));
        assert!(lua.contains("__newindex = function(_, key, value)"));
        assert!(lua.contains("jf_profile_globals[key] = value"));
        assert!(!lua.contains("reset_game_globals"));
    }

    #[test]
    fn build_main_lua_bans_vanilla_jokers_when_disabled() {
        let lua = build_main_lua(
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            "mod",
            &[],
            false,
            false,
            false,
            false,
            false,
            false,
            true,
            &[],
        );

        assert!(lua.contains("SMODS.current_mod.reset_game_globals = function(run_start)"));
        assert!(lua.contains("if v.set == 'Joker' and not v.mod then"));
        assert!(lua.contains("G.GAME.banned_keys[k] = true"));
    }

    #[test]
    fn build_main_lua_registers_joker_object_types_for_custom_pools() {
        let jokers = vec![make_joker_entry_with_pools(
            vec![],
            vec!["overview_jokers".to_string()],
        )];

        assert!(map_appearance(&jokers[0].joker_data).is_none());

        let lua = build_main_lua(
            &jokers,
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            "overview",
            &[],
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            &[],
        );

        assert!(lua.contains("assert(SMODS.load_file(\"jokers/j_test.lua\"))()"));
        assert!(lua.contains("SMODS.ObjectType({"));
        assert!(lua.contains("key = 'overview_jokers'"));
        assert!(lua.contains("['j_overview_test'] = true"));
    }

    #[test]
    fn build_main_lua_registers_every_joker_in_the_default_mod_pool() {
        let jokers = vec![make_joker_entry_with_pools(vec![], vec![])];

        let lua = build_main_lua(
            &jokers,
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            "overview",
            &[],
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            &[],
        );

        assert!(lua.contains("key = 'overview_jokers'"));
        assert!(lua.contains("['j_overview_test'] = true"));
    }
}
