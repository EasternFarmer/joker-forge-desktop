//! Emit actual compiler output for the Lua 5.1 regression runner.
use balatro_codegen::compiler::conditions::compile_condition;
use balatro_codegen::compiler::context::CompileContext;
use balatro_codegen::compiler::effects::utils::value_to_lua_str;
use balatro_codegen::compiler::values::{
    game_var_lua_code, resolve_condition_value, resolve_value,
};
use balatro_codegen::types::{
    ConditionDef, ConsumableDef, EffectDef, JokerDef, ObjectType, ParamValue,
};
use balatro_codegen::{compile_consumable, compile_joker, Emitter};
use serde_json::{json, Value};
use std::collections::HashMap;

fn joker(rules: Value) -> JokerDef {
    serde_json::from_value(json!({
        "key": "runtime_test", "name": "Runtime Test", "description": ["Test"],
        "cost": 4, "rarity": "common", "blueprint_compat": true, "eternal_compat": true,
        "perishable_compat": true, "unlocked": true, "discovered": true,
        "atlas": "CustomJokers", "pos": {"x": 0, "y": 0}, "rules": rules
    }))
    .unwrap()
}

fn main() {
    let source = include_str!("../../../../src/lib/content/game-vars.ts");
    let mut cases = Vec::new();
    let mut ids = Vec::new();
    let mut id = "";
    for line in source.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("id: \"") {
            id = rest.split('"').next().unwrap();
        }
        if line.starts_with("code: ") && !id.is_empty() {
            let code = game_var_lua_code(id).unwrap_or_else(|| panic!("Unmapped editor ID: {id}"));
            cases.push(json!({"name": id, "kind": "game", "code": code}));
            ids.push(id);
        }
    }
    for alias in [
        "deck_size",
        "full_deck_size",
        "hand_size",
        "remaining_hands",
        "remaining_discards",
        "player_money",
        "dollars",
        "ante_level",
        "blind_chips",
        "consumable_count",
        "times_hand_played",
    ] {
        cases.push(
            json!({"name": alias, "kind": "game", "code": game_var_lua_code(alias).unwrap()}),
        );
    }
    let mut tooltip = joker(json!([]));
    tooltip.description_variables = Some(
        serde_json::from_value(json!(ids
            .iter()
            .map(|id| json!({"kind": "game", "id": id}))
            .collect::<Vec<_>>()))
        .unwrap(),
    );
    cases.push(json!({"name": "catalog_tooltips", "kind": "tooltip", "ids": ids, "code": Emitter::new().emit_chunk(&compile_joker(&tooltip, "mod"))}));

    for edition in [
        "any",
        "none",
        "foil",
        "holo",
        "polychrome",
        "negative",
        "sparkle",
        "e_other_shiny",
        "e_quote'\\edition",
    ] {
        for negate in [false, true] {
            let condition: ConditionDef = serde_json::from_value(json!({
                "condition_type": "card_edition", "negate": negate, "params": {"edition": edition}
            }))
            .unwrap();
            let mut ctx =
                CompileContext::new(ObjectType::Joker, "mod".into(), "test".into(), false);
            let code = compile_condition(&condition, ObjectType::Joker, &mut ctx)
                .unwrap()
                .to_string();
            cases.push(json!({"name": edition, "kind": "edition", "negate": negate, "code": code}));
        }
    }
    for reference in [
        "GAMEVAR:cards_in_deck|2|3",
        "GAMEVAR:missing|2|3",
        "GAMEVAR:cards_in_deck",
        "GAMEVAR:cards_in_deck|NaN|0",
        "GAMEVAR:cards_in_deck|inf|0",
        "GAMEVAR:cards_in_deck|1|0|extra",
    ] {
        for typed in [false, true] {
            let value: ParamValue = serde_json::from_value(if typed {
                json!({"value": reference, "valueType": "gameVariable"})
            } else {
                json!(reference)
            })
            .unwrap();
            cases.push(json!({"name": reference, "kind": "value", "code": resolve_value(&value, ObjectType::Joker, None).to_string()}));
            let effect = EffectDef {
                id: "effect".into(),
                effect_type: "edit_dollars".into(),
                params: HashMap::from([("value".into(), value.clone())]),
            };
            let mut ctx =
                CompileContext::new(ObjectType::Joker, "mod".into(), "test".into(), false);
            let code = value_to_lua_str(&effect, "value", &mut ctx, "dollars");
            cases.push(json!({"name": reference, "kind": "effect_value", "code": code}));
            let code = resolve_condition_value(&effect.params, "value", &mut ctx, "player_money")
                .unwrap()
                .to_string();
            cases.push(json!({"name": reference, "kind": "condition_value", "code": code}));

            let mut definition = joker(json!([{
                "id": "score", "trigger": "hand_played", "effects": [{"id": "mult", "effect_type": "add_mult", "params": {"value": value}}]
            }]));
            definition.description_variables = Some(
                serde_json::from_value(
                    json!([{"kind": "config", "name": "unused", "fallback": value}]),
                )
                .unwrap(),
            );
            cases.push(json!({"name": reference, "kind": "joker", "code": Emitter::new().emit_chunk(&compile_joker(&definition, "mod"))}));
        }
    }
    for id in [
        "hand_level",
        "times_hand_played",
        "current_hand_played_count",
        "scored_card_count",
        "played_card_count",
        "cumulative_chips",
    ] {
        let planet: ConsumableDef = serde_json::from_value(json!({
            "key": "planet_test", "name": "Planet Test", "description": ["Test"],
            "set": "Planet", "cost": 3, "atlas": "CustomConsumables", "pos": {"x": 0, "y": 0},
            "rules": [{"id": "use", "trigger": "card_used", "effects": [{
                "id": "level", "effect_type": "level_up_hand", "params": {
                    "selection_mode": "specific", "hand": "Flush",
                    "amount": {"value": format!("GAMEVAR:{id}|2|1"), "valueType": "gameVariable"}
                }
            }]}]
        }))
        .unwrap();
        cases.push(json!({"name": id, "kind": "planet", "code": Emitter::new().emit_chunk(&compile_consumable(&planet, "mod"))}));
    }
    let output = std::env::args().nth(1).expect("Pass the output JSON path");
    std::fs::write(output, serde_json::to_vec(&cases).unwrap()).unwrap();
}
