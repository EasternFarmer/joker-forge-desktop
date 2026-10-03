use crate::compiler::context::CompileContext;
use crate::compiler::effects::utils::{get_str, get_str_default, value_to_lua_str};
use crate::compiler::effects::EffectOutput;
use crate::lua_ast::*;
use crate::types::EffectDef;

fn is_scoring_trigger(trigger: &str) -> bool {
    matches!(trigger, "hand_played" | "card_scored")
}

// ---------------------------------------------------------------------------
// edit_reroll_price
// ---------------------------------------------------------------------------

/// Edit Reroll Price: modifies the reroll cost in the shop.
pub fn edit_reroll_price(effect: &EffectDef, ctx: &mut CompileContext) -> EffectOutput {
    let operation = get_str_default(effect, "operation", "add");
    let value_str = value_to_lua_str(effect, "value", ctx, "reroll_cost");

    let lua = match operation.as_str() {
        "subtract" => format!(
            "G.GAME.round_resets.reroll_cost = G.GAME.round_resets.reroll_cost - {val}\n\
            G.GAME.current_round.reroll_cost = math.max(0, G.GAME.current_round.reroll_cost - {val})",
            val = value_str
        ),
        _ => format!(
            "G.GAME.round_resets.reroll_cost = G.GAME.round_resets.reroll_cost + {val}\n\
            G.GAME.current_round.reroll_cost = math.max(0, G.GAME.current_round.reroll_cost + {val})",
            val = value_str
        ),
    };

    EffectOutput {
        return_fields: vec![],
        pre_return: vec![lua_raw_stmt(lua)],
        config_vars: vec![],
        message: Some(lua_str("Reroll Cost Changed")),
        colour: Some(lua_raw_expr("G.C.MONEY")),

        segment_id: None,
    }
}

// ---------------------------------------------------------------------------
// edit_interest_cap
// ---------------------------------------------------------------------------

/// Edit Interest Cap: modifies G.GAME.interest_cap.
pub fn edit_interest_cap(effect: &EffectDef, ctx: &mut CompileContext) -> EffectOutput {
    let operation = get_str_default(effect, "operation", "add");
    let value_str = value_to_lua_str(effect, "value", ctx, "interest_cap");

    let inner = match operation.as_str() {
        "subtract" => format!("G.GAME.interest_cap = G.GAME.interest_cap - {}", value_str),
        "set" => format!("G.GAME.interest_cap = {}", value_str),
        "multiply" => format!("G.GAME.interest_cap = G.GAME.interest_cap * {}", value_str),
        "divide" => format!("G.GAME.interest_cap = G.GAME.interest_cap / {}", value_str),
        _ => format!("G.GAME.interest_cap = G.GAME.interest_cap + {}", value_str),
    };

    EffectOutput {
        return_fields: vec![],
        pre_return: vec![lua_raw_stmt(inner)],
        config_vars: vec![],
        message: Some(lua_str("Interest Cap Changed")),
        colour: Some(lua_raw_expr("G.C.MONEY")),

        segment_id: None,
    }
}

// ---------------------------------------------------------------------------
// discount_items
// ---------------------------------------------------------------------------

/// Current editor discounts target particular item categories. Legacy projects
/// without those fields retain their global discount-percent operation.
pub fn discount_items(effect: &EffectDef, ctx: &mut CompileContext) -> EffectOutput {
    if has_targeted_discount_params(effect) {
        let (discount_type, discount_method, amount) = discount_settings(effect, ctx);
        let code = format!(
            "if G and G.GAME then\n{}\n\
            G.GAME.jf_item_discounts = G.GAME.jf_item_discounts or {{}}\n\
            table.insert(G.GAME.jf_item_discounts, {{ item_type = {}, method = {}, amount = {} }})\n{}\nend",
            persistent_discount_hook(),
            Emitter::new().emit_expr_to_string(&lua_str(discount_type)),
            Emitter::new().emit_expr_to_string(&lua_str(discount_method)),
            amount,
            refresh_prices(),
        );
        return EffectOutput {
            pre_return: vec![lua_raw_stmt(code)],
            message: Some(lua_str("Items Discounted")),
            colour: Some(lua_raw_expr("G.C.MONEY")),
            ..Default::default()
        };
    }
    let operation = get_str_default(effect, "operation", "add");
    let value_str = value_to_lua_str(effect, "value", ctx, "item_prices");

    let refresh = refresh_prices();

    let inner = match operation.as_str() {
        "subtract" => format!(
            "G.GAME.discount_percent = (G.GAME.discount_percent or 0) - {}\n{}",
            value_str, refresh
        ),
        "set" => format!("G.GAME.discount_percent = {}\n{}", value_str, refresh),
        "multiply" => format!(
            "G.GAME.discount_percent = (G.GAME.discount_percent or 0) * {}\n{}",
            value_str, refresh
        ),
        "divide" => format!(
            "G.GAME.discount_percent = (G.GAME.discount_percent or 0) / {}\n{}",
            value_str, refresh
        ),
        _ => format!(
            "G.GAME.discount_percent = (G.GAME.discount_percent or 0) + {}\n{}",
            value_str, refresh
        ),
    };

    EffectOutput {
        return_fields: vec![],
        pre_return: vec![lua_raw_stmt(inner)],
        config_vars: vec![],
        message: Some(lua_str("Items Discounted")),
        colour: Some(lua_raw_expr("G.C.MONEY")),

        segment_id: None,
    }
}

pub(crate) fn has_targeted_discount_params(effect: &EffectDef) -> bool {
    ["discount_type", "discount_method", "discount_amount", "discountType", "discountMethod", "discountAmount"]
        .iter()
        .any(|key| effect.params.contains_key(*key))
        || !(effect.params.contains_key("operation") || effect.params.contains_key("value"))
}

pub(crate) fn discount_settings(
    effect: &EffectDef,
    ctx: &mut CompileContext,
) -> (String, String, String) {
    let discount_type = get_str(effect, "discount_type")
        .or_else(|| get_str(effect, "discountType"))
        .unwrap_or_else(|| "planet".into());
    let discount_method = get_str(effect, "discount_method")
        .or_else(|| get_str(effect, "discountMethod"))
        .unwrap_or_else(|| "make_free".into());
    let amount_key = if effect.params.contains_key("discountAmount") && !effect.params.contains_key("discount_amount") {
        "discountAmount"
    } else {
        "discount_amount"
    };
    let amount = value_to_lua_str(effect, amount_key, ctx, "discount_amount");
    (discount_type, discount_method, amount)
}

pub(crate) fn refresh_prices() -> &'static str {
    "for _, item in pairs(G and G.I and G.I.CARD or {}) do\n\
        if item.set_cost then item:set_cost() end\n\
    end"
}

pub(crate) fn discount_type_to_condition(discount_type: &str) -> &'static str {
    match discount_type {
        "planet" => "(item_set == 'Planet' or (item_set == 'Booster' and center.kind == 'Celestial'))",
        "tarot" => "(item_set == 'Tarot' or (item_set == 'Booster' and center.kind == 'Arcana'))",
        "spectral" => "(item_set == 'Spectral' or (item_set == 'Booster' and center.kind == 'Spectral'))",
        "standard" => "(item_set == 'Default' or item_set == 'Enhanced' or (item_set == 'Booster' and center.kind == 'Standard'))",
        "jokers" => "item_set == 'Joker'",
        "vouchers" => "item_set == 'Voucher'",
        "all_consumables" => "is_consumable",
        "all_cards" => "(item_set == 'Joker' or is_consumable or item_set == 'Default' or item_set == 'Enhanced' or item_set == 'Booster')",
        "all_shop_items" => "(item_set == 'Joker' or is_consumable or item_set == 'Default' or item_set == 'Enhanced' or item_set == 'Booster' or item_set == 'Voucher')",
        _ => "false",
    }
}

pub(crate) fn discount_method_to_logic(method: &str, amount: &str) -> String {
    match method {
        "make_free" => "self.cost = 0".into(),
        "percentage_reduction" => format!(
            "self.cost = math.max(0, math.floor(self.cost * (1 - (math.max(0, {amount})) / 100)))"
        ),
        "flat_reduction" => format!("self.cost = math.max(0, self.cost - math.max(0, {amount}))"),
        _ => String::new(),
    }
}

pub(crate) fn discount_card_locals() -> &'static str {
    "local center = self.config and self.config.center or {}\n\
    local ability = self.ability or {}\n\
    local item_set = ability.set or center.set\n\
    local is_consumable = ability.consumeable or center.consumeable or item_set == 'Tarot' or item_set == 'Planet' or item_set == 'Spectral'"
}

pub(crate) fn update_discount_sell_cost() -> &'static str {
    "if self.set_sell_value then\n\
        self:set_sell_value()\n\
    else\n\
        self.sell_cost = math.max(1, math.floor(self.cost / 2)) + (self.ability and self.ability.extra_value or 0)\n\
    end\n\
    self.sell_cost_label = self.facing == 'back' and '?' or self.sell_cost"
}

/// Install once across exported objects/mods. Only plain discount records are
/// stored in G.GAME, so discounts survive saving and apply to future shop cards.
pub(crate) fn persistent_discount_hook() -> String {
    let categories = ["planet", "tarot", "spectral", "standard", "jokers", "vouchers", "all_consumables", "all_cards", "all_shop_items"];
    let matches = categories.iter().map(|category| format!(
        "(discount.item_type == '{category}' and {})", discount_type_to_condition(category)
    )).collect::<Vec<_>>().join(" or\n");
    format!(
        "if not Card.jf_item_discount_hook then\n\
        Card.jf_item_discount_hook = true\n\
        local card_set_cost_ref = Card.set_cost\n\
        function Card:set_cost(...)\n\
            local result = card_set_cost_ref(self, ...)\n\
            if type(self.cost) == 'number' then\n{}\n\
                local original_cost = self.cost\n\
                for _, discount in ipairs(G and G.GAME and G.GAME.jf_item_discounts or {{}}) do\n\
                    if {} then\n\
                        local amount = tonumber(discount.amount) or 0\n\
                        if discount.method == 'make_free' then\n{}\n\
                        elseif discount.method == 'percentage_reduction' then\n{}\n\
                        elseif discount.method == 'flat_reduction' then\n{}\n\
                        end\n\
                    end\n\
                end\n\
                if self.cost ~= original_cost then\n{}\nend\n\
            end\n\
            return result\n\
        end\n\
    end",
        discount_card_locals(), matches,
        discount_method_to_logic("make_free", "amount"),
        discount_method_to_logic("percentage_reduction", "amount"),
        discount_method_to_logic("flat_reduction", "amount"),
        update_discount_sell_cost(),
    )
}

// ---------------------------------------------------------------------------
// edit_item_weight
// ---------------------------------------------------------------------------

/// Edit Item Weight: modifies spawn rates for item types.
///
/// `item_weight_type` controls whether it modifies `_rate` or `_mod`.
pub fn edit_item_weight(effect: &EffectDef, ctx: &mut CompileContext) -> EffectOutput {
    let operation = get_str_default(effect, "operation", "add");
    let key = get_str_default(effect, "key", "");
    let weight_type = get_str_default(effect, "weight_type", "rate");
    let value_str = value_to_lua_str(effect, "value", ctx, "item_rate");

    let item_code = if weight_type == "rarity_weight" {
        format!("G.GAME.{}_mod", key)
    } else {
        format!("G.GAME.{}_rate", key)
    };

    let assign = match operation.as_str() {
        "subtract" => format!("{} = {} - {}", item_code, item_code, value_str),
        "set" => format!("{} = {}", item_code, value_str),
        "multiply" => format!("{} = {} * {}", item_code, item_code, value_str),
        "divide" => format!("{} = {} / {}", item_code, item_code, value_str),
        _ => format!("{} = {} + {}", item_code, item_code, value_str),
    };

    EffectOutput {
        return_fields: vec![],
        pre_return: vec![lua_raw_stmt(assign)],
        config_vars: vec![],
        message: Some(lua_str("Spawn Rate Changed")),
        colour: Some(lua_raw_expr("G.C.BLUE")),

        segment_id: None,
    }
}

// ---------------------------------------------------------------------------
// edit_winner_ante
// ---------------------------------------------------------------------------

/// Edit Winner Ante: modifies G.GAME.win_ante (the ante needed to win).
pub fn edit_winner_ante(
    effect: &EffectDef,
    ctx: &mut CompileContext,
    trigger: &str,
) -> EffectOutput {
    let operation = get_str_default(effect, "operation", "set");
    let custom_message = get_str(effect, "customMessage");
    let value_str = value_to_lua_str(effect, "value", ctx, "winner_ante_value");
    let scoring = is_scoring_trigger(trigger);

    let (ante_code, default_msg) = match operation.as_str() {
        "add" => (
            format!(
                "local ante = G.GAME.win_ante + {val}\n\
                local int_part, frac_part = math.modf(ante)\n\
                local rounded = int_part + (frac_part >= 0.5 and 1 or 0)\n\
                G.GAME.win_ante = rounded",
                val = value_str
            ),
            format!("\"Winner Ante +\"..tostring({})", value_str),
        ),
        "subtract" => (
            format!(
                "local ante = G.GAME.win_ante - {val}\n\
                local int_part, frac_part = math.modf(ante)\n\
                local rounded = int_part + (frac_part >= 0.5 and 1 or 0)\n\
                G.GAME.win_ante = rounded",
                val = value_str
            ),
            format!("\"Winner Ante -\"..tostring({})", value_str),
        ),
        _ => (
            format!("G.GAME.win_ante = {val}", val = value_str),
            format!("\"Winner Ante set to \"..tostring({})..'!'", value_str),
        ),
    };

    let message_expr = custom_message
        .map(lua_str)
        .unwrap_or_else(|| lua_raw_expr(default_msg));

    if scoring {
        EffectOutput {
            return_fields: vec![],
            pre_return: vec![lua_raw_stmt(ante_code)],
            config_vars: vec![],
            message: Some(message_expr),
            colour: Some(lua_raw_expr("G.C.FILTER")),

            segment_id: None,
        }
    } else {
        let func_body = vec![lua_raw_stmt(format!("{}\nreturn true", ante_code))];
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
            message: Some(message_expr),
            colour: Some(lua_raw_expr("G.C.FILTER")),

            segment_id: None,
        }
    }
}

// ---------------------------------------------------------------------------
// edit_end_round_hand_money
// ---------------------------------------------------------------------------

/// Edit End Round Hand Money: modifies G.GAME.modifiers.money_per_hand.
pub fn edit_end_round_hand_money(effect: &EffectDef, ctx: &mut CompileContext) -> EffectOutput {
    let operation = get_str_default(effect, "operation", "add");
    let value_str = value_to_lua_str(effect, "value", ctx, "hand_money");

    let code = match operation.as_str() {
        "subtract" => format!(
            "G.GAME.modifiers.money_per_hand = (G.GAME.modifiers.money_per_hand or 1) - {}",
            value_str
        ),
        "set" => format!("G.GAME.modifiers.money_per_hand = {}", value_str),
        _ => format!(
            "G.GAME.modifiers.money_per_hand = (G.GAME.modifiers.money_per_hand or 1) + {}",
            value_str
        ),
    };

    EffectOutput {
        return_fields: vec![],
        pre_return: vec![lua_raw_stmt(code)],
        config_vars: vec![],
        message: Some(lua_str("End-Round Money Changed")),
        colour: Some(lua_raw_expr("G.C.MONEY")),

        segment_id: None,
    }
}

// ---------------------------------------------------------------------------
// edit_end_round_discard_money
// ---------------------------------------------------------------------------

/// Edit End Round Discard Money: modifies G.GAME.modifiers.money_per_discard.
pub fn edit_end_round_discard_money(effect: &EffectDef, ctx: &mut CompileContext) -> EffectOutput {
    let operation = get_str_default(effect, "operation", "set");
    let value_str = value_to_lua_str(effect, "value", ctx, "discard_money");

    let code = match operation.as_str() {
        "subtract" => format!(
            "G.GAME.modifiers.money_per_discard = (G.GAME.modifiers.money_per_discard or 0) - {}",
            value_str
        ),
        "add" => format!(
            "G.GAME.modifiers.money_per_discard = (G.GAME.modifiers.money_per_discard or 0) + {}",
            value_str
        ),
        _ => format!("G.GAME.modifiers.money_per_discard = {}", value_str),
    };

    EffectOutput {
        return_fields: vec![],
        pre_return: vec![lua_raw_stmt(code)],
        config_vars: vec![],
        message: Some(lua_str("Discard Money Changed")),
        colour: Some(lua_raw_expr("G.C.MONEY")),

        segment_id: None,
    }
}
