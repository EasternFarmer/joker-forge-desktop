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

pub fn poker_hand_variable(condition: &ConditionDef, _ctx: &CompileContext) -> Option<Expr> {
    let name = match variable_name(condition, "poker_hand_variable") {
        Ok(name) => name,
        Err(marker) => return Some(marker),
    };
    let current = format!(
        "(G and G.GAME and G.GAME.current_round and G.GAME.current_round[{}])",
        lua_str(format!("{name}_hand"))
    );
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

pub fn rank_variable(condition: &ConditionDef, _ctx: &CompileContext) -> Option<Expr> {
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
    Some(lua_raw_expr(format!(
        "(function() local selected = G and G.GAME and G.GAME.current_round and G.GAME.current_round[{field}]; if not selected then return false end; local expected = {expected_id}; return (expected ~= nil and selected.id ~= nil and selected.id == expected) or (selected.id == nil and (selected.rank == {rank} or selected.rank == {canonical})) end)()",
        field = lua_str(format!("{name}_card")),
        rank = lua_str(rank),
        canonical = lua_str(canonical),
    )))
}

pub fn suit_variable(condition: &ConditionDef, _ctx: &CompileContext) -> Option<Expr> {
    let name = match variable_name(condition, "suit_variable") {
        Ok(name) => name,
        Err(marker) => return Some(marker),
    };
    let suit = str_param(condition, &["specific_suit", "suit", "value"]).unwrap_or("Spades");
    let field = lua_str(format!("{name}_card")).to_string();
    Some(lua_raw_expr(format!(
        "(G and G.GAME and G.GAME.current_round and G.GAME.current_round[{field}] and G.GAME.current_round[{field}].suit == {suit})",
        suit = lua_str(suit),
    )))
}

#[cfg(test)]
mod comparison_tests {
    use super::*;
    use crate::types::ObjectType;
    use serde_json::json;

    fn context() -> CompileContext {
        CompileContext::new(ObjectType::Joker, "mod".into(), "test".into(), true)
    }

    fn condition(kind: &str, params: serde_json::Value) -> ConditionDef {
        serde_json::from_value(json!({"condition_type": kind, "negate": false, "params": params}))
            .unwrap()
    }

    #[test]
    fn rank_reads_the_current_selector_before_legacy_values() {
        let code = rank_variable(
            &condition(
                "rank_variable",
                json!({
                    "variable_name": {"value": "chosen", "valueType": "user_var"},
                    "specific_rank": {"value": "Q", "valueType": "text"},
                    "rank": "A"
                }),
            ),
            &context(),
        )
        .unwrap()
        .to_string();
        assert!(code.contains("local expected = 12"), "{code}");
        assert!(
            code.contains("selected.rank == 'Q' or selected.rank == 'Queen'"),
            "{code}"
        );
        assert!(
            code.contains("G and G.GAME and G.GAME.current_round"),
            "{code}"
        );
        assert!(code.contains("['chosen_card']"), "{code}");
    }

    #[test]
    fn rank_keeps_legacy_aliases_and_safe_custom_rank_ids() {
        for (rank, id) in [
            ("A", "14"),
            ("Ace", "14"),
            ("K", "13"),
            ("King", "13"),
            ("2", "2"),
        ] {
            let code = rank_variable(
                &condition(
                    "rank_variable",
                    json!({"variableName": "chosen", "rank": rank}),
                ),
                &context(),
            )
            .unwrap()
            .to_string();
            assert!(code.contains(&format!("local expected = {id}")), "{code}");
        }
        let code = rank_variable(
            &condition(
                "rank_variable",
                json!({"variable": "chosen", "specific_rank": "mod_star"}),
            ),
            &context(),
        )
        .unwrap()
        .to_string();
        assert!(code.contains("SMODS and SMODS.Ranks"), "{code}");
        assert!(code.contains("SMODS.Ranks['mod_star'].id"), "{code}");
        assert!(
            !code.contains("or 0"),
            "an unknown rank must not match ID zero: {code}"
        );
    }

    #[test]
    fn suit_reads_current_and_legacy_selectors_and_quotes_names() {
        let current = suit_variable(
            &condition(
                "suit_variable",
                json!({
                    "variable_name": "chosen", "specific_suit": "mod_Moons", "suit": "Spades"
                }),
            ),
            &context(),
        )
        .unwrap()
        .to_string();
        assert!(current.contains(".suit == 'mod_Moons'"), "{current}");
        assert!(!current.contains("Spades"), "{current}");
        assert!(
            current.contains("G and G.GAME and G.GAME.current_round"),
            "{current}"
        );
        let legacy = suit_variable(
            &condition(
                "suit_variable",
                json!({"variable_name": "chosen", "suit": "Hearts"}),
            ),
            &context(),
        )
        .unwrap()
        .to_string();
        assert!(legacy.contains(".suit == 'Hearts'"), "{legacy}");
    }

    #[test]
    fn rank_and_suit_parameter_names_match_the_live_catalog() {
        let catalog: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../src/mod_engine/catalog/conditions.json"
        )))
        .unwrap();
        for (kind, parameter) in [
            ("rank_variable", "specific_rank"),
            ("suit_variable", "specific_suit"),
        ] {
            let entry = catalog
                .as_array()
                .unwrap()
                .iter()
                .find(|item| item["id"] == kind)
                .unwrap();
            let selector = entry["params"]
                .as_array()
                .unwrap()
                .iter()
                .find(|item| item["id"] == parameter)
                .unwrap();
            let condition = condition(
                kind,
                json!({"variable_name": "chosen", parameter: selector["default"]}),
            );
            let code = if kind == "rank_variable" {
                rank_variable(&condition, &context())
            } else {
                suit_variable(&condition, &context())
            }
            .unwrap()
            .to_string();
            assert!(!code.contains("invalid condition"), "{code}");
        }
    }

    #[test]
    fn poker_hand_comparisons_use_specific_and_most_or_least_played_catalog_options() {
        let specific = poker_hand_variable(&condition("pokerhand_variable", json!({"variable_name": "chosen", "check_type": "specific", "specific_pokerhand": "Flush", "hand": "Pair"})), &context()).unwrap().to_string();
        assert!(specific.contains("== 'Flush'"), "{specific}");
        assert!(!specific.contains("Pair"), "{specific}");
        for (mode, comparison) in [("most_played", "> tally"), ("least_played", "< tally")] {
            let code = poker_hand_variable(
                &condition(
                    "pokerhand_variable",
                    json!({"variable_name": "chosen", "check_type": mode}),
                ),
                &context(),
            )
            .unwrap()
            .to_string();
            assert!(code.contains(comparison), "{code}");
            assert!(code.contains("hand.visible"), "{code}");
            assert!(
                code.contains("if current == nil then return false end"),
                "{code}"
            );
        }
        let invalid = poker_hand_variable(
            &condition(
                "pokerhand_variable",
                json!({"variable_name": "chosen", "check_type": "unknown"}),
            ),
            &context(),
        )
        .unwrap()
        .to_string();
        assert!(invalid.contains("invalid condition"), "{invalid}");
    }
}
