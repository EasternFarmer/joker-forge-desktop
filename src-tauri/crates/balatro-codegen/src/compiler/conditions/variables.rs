use crate::compiler::conditions::utils::{invalid_condition, str_param};
use crate::compiler::context::CompileContext;
use crate::compiler::values::comparison_op;
use crate::compiler::values::resolve_condition_value;
use crate::lua_ast::*;
use crate::types::ConditionDef;

const VARIABLE_NAME_KEYS: &[&str] = &["variable_name", "variableName", "variable"];

fn variable_name<'a>(condition: &'a ConditionDef, condition_type: &str) -> Result<&'a str, Expr> {
    str_param(condition, VARIABLE_NAME_KEYS)
        .ok_or_else(|| invalid_condition(condition_type, "no variable selected"))
}

pub fn internal_variable(condition: &ConditionDef, ctx: &mut CompileContext) -> Option<Expr> {
    let name = match variable_name(condition, "internal_variable") {
        Ok(name) => name,
        Err(marker) => return Some(marker),
    };
    let operator = str_param(condition, &["operator", "op"]).unwrap_or("equals");
    let rhs = resolve_condition_value(&condition.params, "value", ctx, "internal_variable_value")
        .unwrap_or_else(|| lua_int(0));

    Some(comparison_op(operator, ctx.user_var_expr(name), rhs))
}

pub fn key_variable(condition: &ConditionDef, ctx: &CompileContext) -> Option<Expr> {
    let name = match variable_name(condition, "key_variable") {
        Ok(name) => name,
        Err(marker) => return Some(marker),
    };
    let check_type = str_param(condition, &["check_type"]).unwrap_or("custom_text");
    if check_type == "key_var" {
        return match str_param(condition, &["key_variable"]) {
            Some(other) => Some(lua_eq(ctx.user_var_expr(name), ctx.user_var_expr(other))),
            None => Some(invalid_condition(
                "key_variable",
                "no key variable selected",
            )),
        };
    }
    let specific_key = str_param(condition, &["specific_key", "key", "value"]).unwrap_or("none");

    Some(lua_eq(ctx.user_var_expr(name), lua_str(specific_key)))
}

pub fn text_variable(condition: &ConditionDef, ctx: &CompileContext) -> Option<Expr> {
    let name = match variable_name(condition, "text_variable") {
        Ok(name) => name,
        Err(marker) => return Some(marker),
    };
    let text = str_param(condition, &["text", "value"]).unwrap_or("");

    Some(lua_eq(ctx.user_var_expr(name), lua_str(text)))
}

pub fn poker_hand_variable(condition: &ConditionDef, ctx: &CompileContext) -> Option<Expr> {
    let name = match variable_name(condition, "poker_hand_variable") {
        Ok(name) => name,
        Err(marker) => return Some(marker),
    };
    let current = if ctx.user_var_is_global(name) {
        ctx.user_var_expr(name).to_string()
    } else {
        format!(
            "(G and G.GAME and G.GAME.current_round and G.GAME.current_round[{}])",
            lua_str(format!("{name}_hand"))
        )
    };
    let mode = str_param(condition, &["check_type", "checkType"]).unwrap_or("specific");
    match mode {
        "specific" => {
            let hand_name = str_param(
                condition,
                &[
                    "specific_pokerhand",
                    "specific_poker_hand",
                    "poker_hand",
                    "hand",
                    "value",
                ],
            )
            .unwrap_or("High Card");
            Some(lua_eq(lua_raw_expr(current), lua_str(hand_name)))
        }
        "most_played" | "least_played" => {
            let (initial, operator) = if mode == "most_played" {
                ("-math.huge", ">")
            } else {
                ("math.huge", "<")
            };
            Some(lua_raw_expr(format!(
                "(function() local current = {current}; if current == nil then return false end; local hands = (G and G.GAME and G.GAME.hands) or {{}}; local selected, tally = nil, {initial}; for _, hand_name in ipairs((G and G.handlist) or {{}}) do local hand = hands[hand_name]; if hand and hand.visible and (hand.played or 0) {operator} tally then selected = hand_name; tally = hand.played or 0 end end; return selected ~= nil and current == selected end)()"
            )))
        }
        _ => Some(invalid_condition(
            "poker_hand_variable",
            "unknown comparison type",
        )),
    }
}

pub fn rank_variable(condition: &ConditionDef, ctx: &CompileContext) -> Option<Expr> {
    let name = match variable_name(condition, "rank_variable") {
        Ok(name) => name,
        Err(marker) => return Some(marker),
    };
    let rank = str_param(condition, &["specific_rank", "rank", "value"]).unwrap_or("A");
    let canonical = match rank {
        "A" => "Ace",
        "K" => "King",
        "Q" => "Queen",
        "J" => "Jack",
        rank => rank,
    };
    let expected_id = match canonical {
        "Ace" => "14".to_string(),
        "King" => "13".to_string(),
        "Queen" => "12".to_string(),
        "Jack" => "11".to_string(),
        "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "10" => canonical.to_string(),
        _ => format!(
            "(SMODS and SMODS.Ranks and SMODS.Ranks[{key}] and SMODS.Ranks[{key}].id)",
            key = lua_str(rank)
        ),
    };
    // Initial values can contain A/J/Q/K, while a card's base.value contains
    // Ace/Jack/Queen/King. Both initialization and rank changes store the ID.
    // Keep the name fallback for older saves which did not include an ID.
    let source = if ctx.user_var_is_global(name) {
        ctx.user_var_expr(name).to_string()
    } else {
        format!("G and G.GAME and G.GAME.current_round and G.GAME.current_round[{}]", lua_str(format!("{name}_card")))
    };
    Some(lua_raw_expr(format!(
        "(function() local selected = {source}; if type(selected) == 'string' then local rank_names = {{A = 'Ace', K = 'King', Q = 'Queen', J = 'Jack'}}; local rank_ids = {{Ace = 14, King = 13, Queen = 12, Jack = 11, ['2'] = 2, ['3'] = 3, ['4'] = 4, ['5'] = 5, ['6'] = 6, ['7'] = 7, ['8'] = 8, ['9'] = 9, ['10'] = 10}}; local normalized = rank_names[selected] or selected; selected = {{rank = normalized, id = rank_ids[normalized] or (SMODS and SMODS.Ranks and SMODS.Ranks[normalized] and SMODS.Ranks[normalized].id)}} end; if type(selected) ~= 'table' then return false end; local expected = {expected_id}; return (expected ~= nil and selected.id ~= nil and selected.id == expected) or (selected.id == nil and (selected.rank == {rank} or selected.rank == {canonical})) end)()",
        rank = lua_str(rank),
        canonical = lua_str(canonical),
    )))
}

pub fn suit_variable(condition: &ConditionDef, ctx: &CompileContext) -> Option<Expr> {
    let name = match variable_name(condition, "suit_variable") {
        Ok(name) => name,
        Err(marker) => return Some(marker),
    };
    let suit = str_param(condition, &["specific_suit", "suit", "value"]).unwrap_or("Spades");
    if ctx.user_var_is_global(name) {
        return Some(lua_eq(ctx.user_var_expr(name), lua_str(suit)));
    }
    let field = lua_str(format!("{name}_card")).to_string();
    Some(lua_raw_expr(format!(
        "(G and G.GAME and G.GAME.current_round and G.GAME.current_round[{field}] and G.GAME.current_round[{field}].suit == {suit})",
        suit = lua_str(suit),
    )))
}
