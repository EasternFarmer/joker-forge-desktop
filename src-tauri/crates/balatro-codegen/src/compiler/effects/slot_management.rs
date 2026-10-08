use crate::compiler::context::CompileContext;
use crate::compiler::effects::utils::{get_str, get_str_default, value_to_lua_str};
use crate::compiler::effects::{passive::PassiveEffectOutput, EffectOutput};
use crate::lua_ast::*;
use crate::types::{EffectDef, ObjectType, ParamValue};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve a value param to a Lua expression string: registering a config var
/// for literal numbers.
fn deck_state_change(edit_code: &str, message: &str, colour: &str) -> EffectOutput {
    EffectOutput {
        pre_return: vec![lua_raw_stmt(format!(
            "{edit_code}\n\
            local jf_status_card = G.deck and G.deck.cards and G.deck.cards[1]\n\
            if jf_status_card and type(jf_status_card.juice_up) == 'function' then\n\
                card_eval_status_text(jf_status_card, 'extra', nil, nil, nil, {{message = {message}, colour = {colour}}})\n\
            end"
        ))],
        return_fields: vec![("effect".into(), lua_bool(true))],
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// edit_joker_slots  (modifies G.jokers.config.card_limit)
// ---------------------------------------------------------------------------

/// Edit Joker Slots effect: changes the joker card limit.
///
/// For joker context: returns `func = function() ... end` in the return table.
pub fn edit_joker_slots(
    effect: &EffectDef,
    ctx: &mut CompileContext,
    trigger: &str,
) -> EffectOutput {
    let operation = get_str_default(effect, "operation", "add");
    let custom_message = get_str(effect, "customMessage");
    let value_str = value_to_lua_str(effect, "value", ctx, "joker_slots");

    let (slots_code, colour_str, _default_msg) = match operation.as_str() {
        "subtract" => (
            format!(
                "G.jokers.config.card_limit = math.max(1, G.jokers.config.card_limit - {})",
                value_str
            ),
            "G.C.RED",
            format!("\"-\"..tostring({})..\"+\" Joker Slot\"\"", value_str),
        ),
        "set" => (
            format!("G.jokers.config.card_limit = {}", value_str),
            "G.C.BLUE",
            format!("\"Joker Slots set to \"..tostring({})", value_str),
        ),
        _ => (
            format!(
                "G.jokers.config.card_limit = G.jokers.config.card_limit + {}",
                value_str
            ),
            "G.C.DARK_EDITION",
            format!("\"+\"..tostring({})..\"+\" Joker Slot\"\"", value_str),
        ),
    };

    let msg_lua = custom_message
        .map(|m| format!("\"{}\"", m))
        .unwrap_or_else(|| match operation.as_str() {
            "subtract" => format!("\"-\"..tostring({})..' Joker Slot'", value_str),
            "set" => format!("\"Joker Slots set to \"..tostring({})", value_str),
            _ => format!("\"+\"..tostring({})..' Joker Slot'", value_str),
        });

    if ctx.object_type == ObjectType::Deck && trigger != "card_used" {
        return deck_state_change(&slots_code, &msg_lua, colour_str);
    }

    let func_body = vec![
        lua_raw_stmt(format!(
            "card_eval_status_text(context.blueprint_card or card, 'extra', nil, nil, nil, {{message = {}, colour = {}}})\n\
            {}\n\
            return true",
            msg_lua, colour_str, slots_code
        )),
    ];

    EffectOutput {
        return_fields: vec![(
            "func".to_string(),
            Expr::Function {
                params: vec![],
                body: func_body,
            },
        )],
        pre_return: vec![],
        config_vars: vec![],
        message: None,
        colour: Some(lua_raw_expr("G.C.DARK_EDITION")),

        segment_id: None,
    }
}

/// Edit Joker Slots passive: changes card_limit when joker is added/removed from deck.
pub fn edit_joker_slots_passive(
    effect: &EffectDef,
    ctx: &mut CompileContext,
) -> PassiveEffectOutput {
    let operation = get_str_default(effect, "operation", "add");
    // We use the same value resolution but need raw strings for add/remove
    let count = ctx.next_effect_count("joker_slots");
    let var_name = ctx.unique_var_name("joker_slots", count);

    let value_str = match effect.params.get("value") {
        Some(ParamValue::Int(n)) => {
            ctx.bind_preview_config_parameter(&var_name, "value");
            ctx.add_config_int(&var_name, *n);
            format!("{}.{}", ctx.ability_path(), var_name)
        }
        Some(ParamValue::Float(n)) => {
            ctx.bind_preview_config_parameter(&var_name, "value");
            ctx.add_config_num(&var_name, *n);
            format!("{}.{}", ctx.ability_path(), var_name)
        }
        _ => "1".to_string(),
    };

    let (add_to_deck, remove_from_deck) = match operation.as_str() {
        "subtract" => (
            format!(
                "G.jokers.config.card_limit = math.max(1, G.jokers.config.card_limit - {})",
                value_str
            ),
            format!(
                "G.jokers.config.card_limit = G.jokers.config.card_limit + {}",
                value_str
            ),
        ),
        "set" => (
            format!(
                "card.ability.extra.original_joker_slots = G.jokers.config.card_limit\n\
                G.jokers.config.card_limit = {}",
                value_str
            ),
            "if card.ability.extra.original_joker_slots then\n\
                G.jokers.config.card_limit = card.ability.extra.original_joker_slots\n\
            end"
            .to_string(),
        ),
        _ => (
            format!(
                "G.jokers.config.card_limit = G.jokers.config.card_limit + {}",
                value_str
            ),
            format!(
                "G.jokers.config.card_limit = G.jokers.config.card_limit - {}",
                value_str
            ),
        ),
    };

    PassiveEffectOutput {
        add_to_deck: vec![lua_raw_stmt(add_to_deck)],
        remove_from_deck: vec![lua_raw_stmt(remove_from_deck)],
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// edit_joker_size  (modifies G.jokers.config.highlighted_limit)
// ---------------------------------------------------------------------------

fn joker_selection_target(operation: &str, value: &str) -> String {
    let target = match operation {
        "subtract" => format!("current_joker_selection_limit - ({value})"),
        "set" => format!("({value})"),
        _ => format!("current_joker_selection_limit + ({value})"),
    };
    // Joker areas always allow at least one selection, and selection counts
    // must be whole numbers even when a game variable supplies the amount.
    format!("math.max(1, math.floor({target}))")
}

fn trim_joker_selection() -> &'static str {
    "while G.jokers.highlighted and #G.jokers.highlighted > G.jokers.config.highlighted_limit do\n\
        G.jokers:remove_from_highlighted(G.jokers.highlighted[1])\n\
    end"
}

/// Edit Joker Size effect: changes how many jokers can be highlighted at once.
pub fn edit_joker_size(
    effect: &EffectDef,
    ctx: &mut CompileContext,
    trigger: &str,
) -> EffectOutput {
    let operation = get_str_default(effect, "operation", "add");
    let custom_message = get_str(effect, "customMessage");
    let value_str = value_to_lua_str(effect, "value", ctx, "joker_size");
    let target = joker_selection_target(&operation, &value_str);
    let size_code = format!(
        "if G and G.jokers and G.jokers.config then\n\
            local current_joker_selection_limit = G.jokers.config.highlighted_limit or 1\n\
            G.jokers.config.highlighted_limit = {target}\n{}\nend",
        trim_joker_selection()
    );
    let colour_str = match operation.as_str() {
        "subtract" => "G.C.RED",
        "set" => "G.C.BLUE",
        _ => "G.C.DARK_EDITION",
    };
    let message = custom_message
        .map(lua_str)
        .unwrap_or_else(|| match operation.as_str() {
            "subtract" => lua_raw_expr(format!("\"-\"..tostring({value_str})..' Joker Selection Size'")),
            "set" => lua_raw_expr(format!("\"Joker Selection Size set to \"..tostring({value_str})")),
            _ => lua_raw_expr(format!("\"+\"..tostring({value_str})..' Joker Selection Size'")),
        });
    let deck_start = ctx.object_type == ObjectType::Deck && trigger == "card_used";
    let size_code = if deck_start {
        // Back.apply runs before the new Joker area is constructed. Applying
        // here would either crash or modify the previous run's area.
        format!(
            "G.E_MANAGER:add_event(Event({{func = function()\n{size_code}\nreturn true\nend}}))"
        )
    } else {
        size_code
    };

    EffectOutput {
        // Back.apply and Voucher.redeem ignore calculation return tables.
        // Perform the mutation here so those hooks execute it as well.
        return_fields: vec![],
        pre_return: vec![lua_raw_stmt(size_code)],
        config_vars: vec![],
        message: (!deck_start).then_some(message),
        colour: Some(lua_raw_expr(colour_str)),
        segment_id: None,
    }
}

/// Edit Joker Size passive: changes highlighted_limit when joker is added/removed.
pub fn edit_joker_size_passive(
    effect: &EffectDef,
    ctx: &mut CompileContext,
) -> PassiveEffectOutput {
    let operation = get_str_default(effect, "operation", "add");
    let value_str = value_to_lua_str(effect, "value", ctx, "joker_size");
    let count = ctx.next_effect_count("joker_selection_delta");
    let state_name = ctx.unique_var_name("joker_selection_delta", count);
    let state_path = format!("{}.{}", ctx.ability_path(), state_name);
    let state_owner_path = format!("{}.joker_selection_owner{}", ctx.ability_path(), count);
    let total_path = format!("{}.jf_joker_selection_total_delta", ctx.ability_path());
    let count_path = format!("{}.jf_joker_selection_effect_count", ctx.ability_path());
    let owner_path = format!("{}.jf_joker_selection_owner", ctx.ability_path());
    let target = joker_selection_target(&operation, &value_str);
    // Save the applied change rather than recomputing a game/user variable
    // on removal or restoring a shared snapshot that another effect replaced.
    // copy_card copies ability.extra, but a new Card gets its own ID.
    // Steamodded saves this ID separately, distinguishing a clone from a
    // restored card even when its transient runtime ID changes on load.
    let add_to_deck = format!(
        "if G and G.jokers and G.jokers.config then\n\
            local joker_selection_card_id = card.unique_val__saved_ID or card.ID or card.sort_id\n\
            if {owner_path} ~= joker_selection_card_id then\n\
                {total_path} = nil\n\
                {count_path} = nil\n\
                {owner_path} = joker_selection_card_id\n\
            end\n\
            if {state_path} == nil or {state_owner_path} ~= joker_selection_card_id then\n\
            local current_joker_selection_limit = G.jokers.config.highlighted_limit or 1\n\
            local target_joker_selection_limit = {target}\n\
            {state_path} = target_joker_selection_limit - current_joker_selection_limit\n\
            {state_owner_path} = joker_selection_card_id\n\
            {total_path} = ({total_path} or 0) + {state_path}\n\
            {count_path} = ({count_path} or 0) + 1\n\
            G.jokers.config.highlighted_limit = target_joker_selection_limit\n{}\nend\nend",
        trim_joker_selection()
    );
    let remove_from_deck = format!(
        "local joker_selection_card_id = card.unique_val__saved_ID or card.ID or card.sort_id\n\
        if G and G.jokers and G.jokers.config and {state_path} ~= nil and {state_owner_path} == joker_selection_card_id then\n\
            {count_path} = ({count_path} or 1) - 1\n\
            {state_path} = nil\n\
            {state_owner_path} = nil\n{}\nend",
        // Undo all selection changes supplied by this card at once. Clamping
        // each opposing delta separately could otherwise lose the baseline.
        format!(
            "if {count_path} == 0 then\n\
                G.jokers.config.highlighted_limit = math.max(1, G.jokers.config.highlighted_limit - ({total_path} or 0))\n\
                {total_path} = nil\n\
                {count_path} = nil\n{}\nend",
            trim_joker_selection()
        )
    );

    PassiveEffectOutput {
        add_to_deck: vec![lua_raw_stmt(add_to_deck)],
        remove_from_deck: vec![lua_raw_stmt(remove_from_deck)],
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// edit_consumable_slots  (modifies G.consumeables.config.card_limit)
// ---------------------------------------------------------------------------

/// Edit Consumable Slots effect: changes the consumable card limit.
pub fn edit_consumable_slots(effect: &EffectDef, ctx: &mut CompileContext) -> EffectOutput {
    let operation = get_str_default(effect, "operation", "add");
    let custom_message = get_str(effect, "customMessage");
    let value_str = value_to_lua_str(effect, "value", ctx, "consumable_slots");

    let (slots_code, colour_str) = match operation.as_str() {
        "subtract" => (
            format!("G.consumeables.config.card_limit = math.max(0, G.consumeables.config.card_limit - {})", value_str),
            "G.C.RED",
        ),
        "set" => (
            format!("G.consumeables.config.card_limit = {}", value_str),
            "G.C.BLUE",
        ),
        _ => (
            format!("G.consumeables.config.card_limit = G.consumeables.config.card_limit + {}", value_str),
            "G.C.GREEN",
        ),
    };

    let msg_lua = custom_message
        .map(|m| format!("\"{}\"", m))
        .unwrap_or_else(|| match operation.as_str() {
            "subtract" => format!("\"-\"..tostring({})..' Consumable Slot'", value_str),
            "set" => format!("\"Set to \"..tostring({})..' Consumable Slots'", value_str),
            _ => format!("\"+\"..tostring({})..' Consumable Slot'", value_str),
        });

    if ctx.object_type == ObjectType::Deck {
        return deck_state_change(&slots_code, &msg_lua, colour_str);
    }

    let func_body = vec![
        lua_raw_stmt(format!(
            "{}\n\
            card_eval_status_text(context.blueprint_card or card, 'extra', nil, nil, nil, {{message = {}, colour = {}}})\n\
            return true",
            slots_code, msg_lua, colour_str
        )),
    ];

    EffectOutput {
        return_fields: vec![(
            "func".to_string(),
            Expr::Function {
                params: vec![],
                body: func_body,
            },
        )],
        pre_return: vec![],
        config_vars: vec![],
        message: None,
        colour: Some(lua_raw_expr("G.C.GREEN")),

        segment_id: None,
    }
}

/// Edit Consumable Slots passive: changes consumable card_limit when joker added/removed.
pub fn edit_consumable_slots_passive(
    effect: &EffectDef,
    ctx: &mut CompileContext,
) -> PassiveEffectOutput {
    let operation = get_str_default(effect, "operation", "add");
    let count = ctx.next_effect_count("consumable_slots");
    let var_name = ctx.unique_var_name("consumable_slots", count);

    let value_str = match effect.params.get("value") {
        Some(ParamValue::Int(n)) => {
            ctx.bind_preview_config_parameter(&var_name, "value");
            ctx.add_config_int(&var_name, *n);
            format!("{}.{}", ctx.ability_path(), var_name)
        }
        Some(ParamValue::Float(n)) => {
            ctx.bind_preview_config_parameter(&var_name, "value");
            ctx.add_config_num(&var_name, *n);
            format!("{}.{}", ctx.ability_path(), var_name)
        }
        _ => "1".to_string(),
    };

    let (add_to_deck, remove_from_deck) = match operation.as_str() {
        "subtract" => (
            format!(
                "G.consumeables.config.card_limit = math.max(0, G.consumeables.config.card_limit - {})",
                value_str
            ),
            format!(
                "G.consumeables.config.card_limit = G.consumeables.config.card_limit + {}",
                value_str
            ),
        ),
        "set" => (
            format!(
                "original_slots = G.consumeables.config.card_limit\n{}",
                format!(
                    "G.consumeables.config.card_limit = {}",
                    value_str
                )
            ),
            format!(
                "if original_slots then\n    {}\nend",
                "G.consumeables.config.card_limit = original_slots"
            ),
        ),
        _ => (
            format!(
                "G.consumeables.config.card_limit = G.consumeables.config.card_limit + {}",
                value_str
            ),
            format!(
                "G.consumeables.config.card_limit = G.consumeables.config.card_limit - {}",
                value_str
            ),
        ),
    };

    PassiveEffectOutput {
        add_to_deck: vec![lua_raw_stmt(add_to_deck)],
        remove_from_deck: vec![lua_raw_stmt(remove_from_deck)],
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// edit_item_size  (hand size, play limit, discard limit, voucher/shop slots)
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct ItemSizeData {
    slots_code: &'static str,
    difference_check: &'static str,
    var_name: &'static str,
    custom_message: &'static str,
}

fn item_size_data(item_type: &str) -> ItemSizeData {
    match item_type {
        "voucher_slots" => ItemSizeData {
            slots_code: "SMODS.change_voucher_limit",
            difference_check: "G.GAME.modifiers.extra_vouchers",
            var_name: "voucher_slots",
            custom_message: "Voucher Slots",
        },
        "booster_slots" => ItemSizeData {
            slots_code: "SMODS.change_booster_limit",
            difference_check: "G.GAME.modifiers.extra_boosters",
            var_name: "booster_slots",
            custom_message: "Booster Slots",
        },
        "shop_slots" => ItemSizeData {
            slots_code: "change_shop_size",
            difference_check: "G.GAME.modifiers.shop_size",
            var_name: "shop_slots",
            custom_message: "Shop Slots",
        },
        "play_size" => ItemSizeData {
            slots_code: "SMODS.change_play_limit",
            difference_check: "G.GAME.starting_params.play_limit",
            var_name: "play_size",
            custom_message: "Play Size",
        },
        "discard_size" => ItemSizeData {
            slots_code: "SMODS.change_discard_limit",
            difference_check: "G.GAME.starting_params.discard_limit",
            var_name: "discard_size",
            custom_message: "Discard Size",
        },
        _ => ItemSizeData {
            // hand_size (default)
            slots_code: "G.hand:change_size",
            difference_check: "G.hand.config.card_limit",
            var_name: "hand_size",
            custom_message: "Hand Limit",
        },
    }
}

/// Edit Item Size effect: changes hand size, play/discard limits, or shop/voucher slots.
///
/// The `item_size_type` param on the effect specifies which stat to modify:
/// `"hand_size"` | `"play_size"` | `"discard_size"` | `"voucher_slots"` |
/// `"booster_slots"` | `"shop_slots"`.
pub fn edit_item_size(effect: &EffectDef, ctx: &mut CompileContext) -> EffectOutput {
    let size_type = get_str_default(effect, "item_size_type", "hand_size");
    edit_item_size_typed(effect, ctx, &size_type)
}

/// Explicit-type variant: size type is provided directly (e.g. derived from the
/// effect ID `edit_hand_size` → `"hand_size"`) rather than read from params.
pub fn edit_item_size_typed(
    effect: &EffectDef,
    ctx: &mut CompileContext,
    size_type: &str,
) -> EffectOutput {
    let operation = get_str_default(effect, "operation", "add");
    let custom_message = get_str(effect, "customMessage").filter(|message| !message.trim().is_empty());
    let message_mode = get_str_default(
        effect,
        "messageMode",
        if custom_message.is_some() { "custom" } else { "default" },
    );
    let data = item_size_data(size_type);
    let value_str = value_to_lua_str(effect, "value", ctx, data.var_name);

    let (value_arg, set_code) = match operation.as_str() {
        "subtract" => (format!("-{}", value_str), String::new()),
        "set" => {
            let set = format!(
                "local current_{name} = ({check} or 0)\n\
                local target_{name} = {val}\n\
                local difference = target_{name} - current_{name}",
                name = data.var_name,
                check = data.difference_check,
                val = value_str,
            );
            ("difference".to_string(), set)
        }
        _ => (value_str.clone(), String::new()),
    };

    let msg_lua = custom_message
        .filter(|_| message_mode == "custom")
        .map(|message| Emitter::new().emit_expr_to_string(&lua_str(message)))
        .unwrap_or_else(|| match operation.as_str() {
            "set" => format!(
                "\"{}  set to \"..tostring({})",
                data.custom_message, value_str
            ),
            "add" => format!("\"+\"..tostring({})..' {}'", value_str, data.custom_message),
            "subtract" => format!("\"-\"..tostring({})..' {}'", value_str, data.custom_message),
            _ => format!("\"+\"..tostring({})..' {}'", value_str, data.custom_message),
        });

    let message_code = if message_mode == "none" {
        String::new()
    } else {
        format!("card_eval_status_text(context.blueprint_card or card, 'extra', nil, nil, nil, {{message = {}, colour = G.C.BLUE}})", msg_lua)
    };
    let func_body = vec![lua_raw_stmt(format!(
        "{}\n\
        {}\n\
        {}({})\n\
        return true",
        message_code, set_code, data.slots_code, value_arg
    ))];

    EffectOutput {
        return_fields: vec![(
            "func".to_string(),
            Expr::Function {
                params: vec![],
                body: func_body,
            },
        )],
        pre_return: vec![],
        config_vars: vec![],
        message: None,
        colour: Some(lua_raw_expr("G.C.BLUE")),

        segment_id: None,
    }
}

/// Edit Item Size passive: modifies item sizes when joker is added/removed.
///
/// Reads `item_size_type` from effect params; use `edit_item_size_passive_typed` when
/// the type is known from the effect ID.
pub fn edit_item_size_passive(effect: &EffectDef, ctx: &mut CompileContext) -> PassiveEffectOutput {
    let size_type = get_str_default(effect, "item_size_type", "hand_size");
    edit_item_size_passive_typed(effect, ctx, &size_type)
}

/// Explicit-type variant for edit_item_size_passive.
pub fn edit_item_size_passive_typed(
    effect: &EffectDef,
    ctx: &mut CompileContext,
    size_type: &str,
) -> PassiveEffectOutput {
    let operation = get_str_default(effect, "operation", "add");
    let data = item_size_data(size_type);

    let count = ctx.next_effect_count(data.var_name);
    let var_name = ctx.unique_var_name(&format!("{}_increase", data.var_name), count);

    let value_str = match effect.params.get("value") {
        Some(ParamValue::Int(n)) => {
            ctx.bind_preview_config_parameter(&var_name, "value");
            ctx.add_config_int(&var_name, *n);
            format!("{}.{}", ctx.ability_path(), var_name)
        }
        Some(ParamValue::Float(n)) => {
            ctx.bind_preview_config_parameter(&var_name, "value");
            ctx.add_config_num(&var_name, *n);
            format!("{}.{}", ctx.ability_path(), var_name)
        }
        _ => "1".to_string(),
    };

    let (add_to_deck, remove_from_deck) = match operation.as_str() {
        "subtract" => (
            format!("{}(-{})", data.slots_code, value_str),
            format!("{}({})", data.slots_code, value_str),
        ),
        "set" => (
            format!(
                "card.ability.extra.original_{name} = {check} or 0\n\
                local difference = {val} - {check}\n\
                {code}(difference)",
                name = data.var_name,
                check = data.difference_check,
                val = value_str,
                code = data.slots_code,
            ),
            format!(
                "if card.ability.extra.original_{name} then\n\
                    local difference = card.ability.extra.original_{name} - {check}\n\
                    {code}(difference)\n\
                end",
                name = data.var_name,
                check = data.difference_check,
                code = data.slots_code,
            ),
        ),
        _ => (
            format!("{}({})", data.slots_code, value_str),
            format!("{}(-{})", data.slots_code, value_str),
        ),
    };

    PassiveEffectOutput {
        add_to_deck: vec![lua_raw_stmt(add_to_deck)],
        remove_from_deck: vec![lua_raw_stmt(remove_from_deck)],
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// edit_hands / edit_discards
// ---------------------------------------------------------------------------

fn round_counter_data(counter_type: &str) -> (&'static str, &'static str, &'static str) {
    match counter_type {
        "discards" => ("discards", "ease_discard", "Discards"),
        _ => ("hands", "ease_hands_played", "Hands"),
    }
}

/// Edit round counters (hands/discards): active version used by effects like
/// `edit_hands` and `edit_discards`.
pub fn edit_round_counter_typed(
    effect: &EffectDef,
    ctx: &mut CompileContext,
    counter_type: &str,
) -> EffectOutput {
    let operation = get_str_default(effect, "operation", "add");
    let duration = get_str_default(effect, "duration", "permanent");
    let custom_message = get_str(effect, "customMessage");
    let (counter_key, ease_fn, label) = round_counter_data(counter_type);
    let value_str = value_to_lua_str(effect, "value", ctx, counter_key);

    let edit_code = match (operation.as_str(), duration.as_str()) {
        ("add", "round") => format!(
            "G.GAME.current_round.{k}_left = G.GAME.current_round.{k}_left + {v}",
            k = counter_key,
            v = value_str
        ),
        ("subtract", "round") => format!(
            "G.GAME.current_round.{k}_left = G.GAME.current_round.{k}_left - {v}",
            k = counter_key,
            v = value_str
        ),
        ("set", "round") => format!(
            "G.GAME.current_round.{k}_left = {v}",
            k = counter_key,
            v = value_str
        ),
        ("subtract", _) => format!(
            "G.GAME.round_resets.{k} = G.GAME.round_resets.{k} - {v}\n\
            {ease}(-{v})",
            k = counter_key,
            v = value_str,
            ease = ease_fn
        ),
        ("set", _) => format!(
            "G.GAME.round_resets.{k} = {v}\n\
            {ease}({v} - G.GAME.current_round.{k}_left)",
            k = counter_key,
            v = value_str,
            ease = ease_fn
        ),
        _ => format!(
            "G.GAME.round_resets.{k} = G.GAME.round_resets.{k} + {v}\n\
            {ease}({v})",
            k = counter_key,
            v = value_str,
            ease = ease_fn
        ),
    };

    let (default_message, colour) = match operation.as_str() {
        "subtract" => (
            format!("\"-\"..tostring({})..\" {}\"", value_str, label),
            "G.C.RED",
        ),
        "set" => (
            format!("\"Set to \"..tostring({})..\" {}\"", value_str, label),
            "G.C.BLUE",
        ),
        _ => (
            format!("\"+\"..tostring({})..\" {}\"", value_str, label),
            "G.C.GREEN",
        ),
    };
    let msg_lua = custom_message
        .map(|m| format!("\"{}\"", m))
        .unwrap_or(default_message);

    if ctx.object_type == ObjectType::Deck {
        return deck_state_change(&edit_code, &msg_lua, colour);
    }

    let func_body = vec![lua_raw_stmt(format!(
        "card_eval_status_text(context.blueprint_card or card, 'extra', nil, nil, nil, {{message = {}, colour = {}}})\n\
        {}\n\
        return true",
        msg_lua, colour, edit_code
    ))];

    EffectOutput {
        return_fields: vec![(
            "func".to_string(),
            Expr::Function {
                params: vec![],
                body: func_body,
            },
        )],
        pre_return: vec![],
        config_vars: vec![],
        message: None,
        colour: Some(lua_raw_expr("G.C.GREEN")),

        segment_id: None,
    }
}

/// Edit round counters (hands/discards): passive version for always-on effects.
pub fn edit_round_counter_passive_typed(
    effect: &EffectDef,
    ctx: &mut CompileContext,
    counter_type: &str,
) -> PassiveEffectOutput {
    let operation = get_str_default(effect, "operation", "add");
    let (counter_key, _, _) = round_counter_data(counter_type);
    let count = ctx.next_effect_count(counter_key);
    let var_name = ctx.unique_var_name(&format!("{}_change", counter_key), count);

    let value_str = match effect.params.get("value") {
        Some(ParamValue::Int(n)) => {
            ctx.bind_preview_config_parameter(&var_name, "value");
            ctx.add_config_int(&var_name, *n);
            format!("{}.{}", ctx.ability_path(), var_name)
        }
        Some(ParamValue::Float(n)) => {
            ctx.bind_preview_config_parameter(&var_name, "value");
            ctx.add_config_num(&var_name, *n);
            format!("{}.{}", ctx.ability_path(), var_name)
        }
        _ => "1".to_string(),
    };

    let (add_to_deck, remove_from_deck) = match operation.as_str() {
        "subtract" => (
            format!(
                "G.GAME.round_resets.{k} = math.max(1, G.GAME.round_resets.{k} - {v})",
                k = counter_key,
                v = value_str
            ),
            format!(
                "G.GAME.round_resets.{k} = G.GAME.round_resets.{k} + {v}",
                k = counter_key,
                v = value_str
            ),
        ),
        "set" => (
            format!(
                "card.ability.extra.original_{k} = G.GAME.round_resets.{k}\n\
                G.GAME.round_resets.{k} = {v}",
                k = counter_key,
                v = value_str
            ),
            format!(
                "if card.ability.extra.original_{k} then\n\
                    G.GAME.round_resets.{k} = card.ability.extra.original_{k}\n\
                end",
                k = counter_key
            ),
        ),
        _ => (
            format!(
                "G.GAME.round_resets.{k} = G.GAME.round_resets.{k} + {v}",
                k = counter_key,
                v = value_str
            ),
            format!(
                "G.GAME.round_resets.{k} = G.GAME.round_resets.{k} - {v}",
                k = counter_key,
                v = value_str
            ),
        ),
    };

    PassiveEffectOutput {
        add_to_deck: vec![lua_raw_stmt(add_to_deck)],
        remove_from_deck: vec![lua_raw_stmt(remove_from_deck)],
        ..Default::default()
    }
}
