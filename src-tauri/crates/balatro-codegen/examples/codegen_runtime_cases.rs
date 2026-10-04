//! Emit actual compiler output for the Lua 5.1 regression runner.
#[allow(dead_code)]
#[path = "../../../src/mod_engine/export.rs"]
mod export;

use balatro_codegen::compiler::conditions::compile_condition;
use balatro_codegen::compiler::context::CompileContext;
use balatro_codegen::compiler::effects::utils::value_to_lua_str;
use balatro_codegen::compiler::values::{
    game_var_lua_code, resolve_condition_value, resolve_value,
};
use balatro_codegen::types::{
    ConditionDef, ConsumableDef, DeckDef, EditionDef, EffectDef, EnhancementDef, JokerDef, ObjectType, ParamValue, SealDef, UserVariableDef, VoucherDef,
};
use balatro_codegen::{compile_consumable, compile_deck, compile_edition, compile_enhancement, compile_joker, compile_seal, compile_voucher, Emitter};
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

fn deck(rules: Value) -> DeckDef {
    serde_json::from_value(json!({
        "key":"runtime_test", "name":"Runtime Test", "description":["Test"],
        "atlas":"CustomDecks", "pos":{"x":0,"y":0}, "rules":rules
    }))
    .unwrap()
}

fn scoring_case(
    cases: &mut Vec<Value>,
    name: &str,
    rules: Value,
    prepare: &str,
    invoke: &str,
    verify: &str,
) {
    let mut definition = joker(rules);
    definition.user_variables = serde_json::from_value(json!([
        {"name":"counter", "var_type":"number", "initial_value":0}
    ]))
    .unwrap();
    cases.push(json!({"kind":"scoring", "name":name,
        "code":Emitter::new().emit_chunk(&compile_joker(&definition,"mod")),
        "prepare":format!("G.GAME.dollars=0;G.play={{cards={{}}}};G.hand={{cards={{}}}};{prepare}"),
        "invoke":invoke,"verify":verify}));
}

fn append_scoring_group_cases(cases: &mut Vec<Value>) {
    for count in [1, 3, 5] {
        scoring_case(cases,&format!("scoring_loop_mixed_effects_{count}"),json!([{
            "id":"mixed", "trigger":"hand_played",
            "effects":[
                {"effect_type":"add_chips","params":{"value":7}},
                {"effect_type":"add_mult","params":{"value":5}},
                {"effect_type":"set_dollars","params":{"value":2,"operation":"add"}}
            ],
            "loop_groups":[{"id":"repeat","count":count,"effects":[
                {"effect_type":"apply_x_mult","params":{"value":2}},
                {"effect_type":"add_chips","params":{"value":4}},
                {"effect_type":"set_dollars","params":{"value":3,"operation":"add"}},
                {"effect_type":"modify_internal_variable","params":{"variable_name":"counter","operation":"increment","value":1}}
            ]}]
        }]),"","resolve_joker({joker_main=true})",
            &format!("assert(hand_chips=={});assert(mult=={});assert(G.GAME.dollars=={});assert(actor.ability.extra.counter=={count});assert(message_count('x_mult')=={count});assert(message_count('chips')=={});assert(message_count('dollars')=={})",
                7+4*count,6*(1<<count),2+3*count,count+1,count+1));
    }
    scoring_case(cases,"scoring_loop_multiple_groups",json!([{
        "id":"multiple", "trigger":"hand_played",
        "effects":[{"effect_type":"add_chips","params":{"value":1}}],
        "loop_groups":[
            {"id":"first","count":2,"effects":[
                {"effect_type":"apply_x_mult","params":{"value":2}},
                {"effect_type":"set_dollars","params":{"value":1}}
            ]},
            {"id":"second","count":2,"effects":[
                {"effect_type":"apply_x_mult","params":{"value":3}},
                {"effect_type":"add_chips","params":{"value":2}}
            ]}
        ]
    }]),"","resolve_joker({joker_main=true})",
        "assert(hand_chips==5);assert(mult==36);assert(G.GAME.dollars==2);assert(message_count('x_mult')==4)");
    scoring_case(cases,"scoring_loop_captures_iteration_values",json!([{
        "id":"dynamic", "trigger":"hand_played",
        "loop_groups":[{"id":"repeat","count":3,"effects":[
            {"effect_type":"modify_internal_variable","params":{"variable_name":"counter","operation":"increment","value":1}},
            {"effect_type":"apply_x_mult","params":{"value":{"value":"counter","valueType":"userVariable"}}},
            {"effect_type":"add_chips","params":{"value":{"value":"counter","valueType":"userVariable"}}},
            {"effect_type":"set_dollars","params":{"value":{"value":"counter","valueType":"userVariable"}}}
        ]}]
    }]),"","resolve_joker({joker_main=true})",
        "assert(hand_chips==6);assert(mult==6);assert(G.GAME.dollars==6);assert(actor.ability.extra.counter==3)");
    scoring_case(cases,"scoring_loop_xchips_and_xmult",json!([{
        "id":"multiply", "trigger":"hand_played",
        "effects":[{"effect_type":"add_chips","params":{"value":5}}],
        "loop_groups":[{"id":"repeat","count":3,"effects":[
            {"effect_type":"apply_x_chips","params":{"value":2}},
            {"effect_type":"apply_x_mult","params":{"value":3}},
            {"effect_type":"set_dollars","params":{"value":1}}
        ]}]
    }]),"","resolve_joker({joker_main=true})",
        "assert(hand_chips==40);assert(mult==27);assert(G.GAME.dollars==3);assert(message_count('x_chips')==3 and message_count('x_mult')==3)");

    for chance_succeeds in [true,false] {
        scoring_case(cases,&format!("scoring_loop_chance_sibling_{chance_succeeds}"),json!([{
            "id":"chance_and_repeat", "trigger":"hand_played",
            "effects":[{"effect_type":"add_chips","params":{"value":2}}],
            "random_groups":[{"id":"chance","chance_numerator":1,"chance_denominator":2,"effects":[
                {"effect_type":"apply_x_mult","params":{"value":5}},
                {"effect_type":"set_dollars","params":{"value":7}}
            ]}],
            "loop_groups":[{"id":"repeat","count":2,"effects":[
                {"effect_type":"apply_x_mult","params":{"value":2}},
                {"effect_type":"add_chips","params":{"value":1}}
            ]}]
        }]),&format!("SMODS.pseudorandom_probability=function() return {chance_succeeds} end"),
            "resolve_joker({joker_main=true})",
            &format!("assert(hand_chips==4);assert(mult=={});assert(G.GAME.dollars=={});assert(message_count('x_mult')=={})",
                if chance_succeeds {20}else{4},if chance_succeeds {7}else{0},if chance_succeeds {3}else{2}));
    }

    let conditional_rules=json!([
        {"id":"conditioned_loop","trigger":"hand_played",
            "condition_groups":[{"conditions":[{"condition_type":"player_money","params":{"operator":"greater_than","value":10}}]}],
            "loop_groups":[{"id":"repeat","count":3,"effects":[
                {"effect_type":"apply_x_mult","params":{"value":2}},
                {"effect_type":"modify_internal_variable","params":{"variable_name":"counter","operation":"increment","value":1}}
            ]}]},
        {"id":"fallback","trigger":"hand_played","effects":[
            {"effect_type":"add_chips","params":{"value":11}},
            {"effect_type":"add_mult","params":{"value":7}},
            {"effect_type":"set_dollars","params":{"value":13}}
        ]}
    ]);
    scoring_case(cases,"scoring_loop_condition_fallback",conditional_rules.clone(),"",
        "resolve_joker({joker_main=true})",
        "assert(hand_chips==11);assert(mult==8);assert(G.GAME.dollars==13);assert(actor.ability.extra.counter==0)");
    scoring_case(cases,"scoring_loop_condition_match",conditional_rules,"G.GAME.dollars=20",
        "resolve_joker({joker_main=true})",
        "assert(hand_chips==0);assert(mult==8);assert(G.GAME.dollars==20);assert(actor.ability.extra.counter==3)");

    scoring_case(cases,"scoring_loop_side_effect_only_sibling",json!([{
        "id":"side_effects", "trigger":"hand_played",
        "effects":[{"effect_type":"add_chips","params":{"value":9}}],
        "loop_groups":[{"id":"repeat","count":4,"effects":[
            {"effect_type":"modify_internal_variable","params":{"variable_name":"counter","operation":"increment","value":1}}
        ]}]
    }]),"","resolve_joker({joker_main=true})",
        "assert(hand_chips==9);assert(mult==1);assert(actor.ability.extra.counter==4)");
    scoring_case(cases,"scoring_chance_failure_falls_through_to_sibling",json!([
        {"id":"chance","trigger":"hand_played","random_groups":[{
            "id":"chance_only","chance_numerator":1,"chance_denominator":2,
            "effects":[{"effect_type":"apply_x_mult","params":{"value":4}}]
        }]},
        {"id":"fallback","trigger":"hand_played","effects":[
            {"effect_type":"add_chips","params":{"value":12}},
            {"effect_type":"set_dollars","params":{"value":3}}
        ]}
    ]),"SMODS.pseudorandom_probability=function() return false end",
        "resolve_joker({joker_main=true})",
        "assert(hand_chips==12 and mult==1 and G.GAME.dollars==3)");
    scoring_case(cases,"scoring_side_effect_loop_falls_through_to_sibling",json!([
        {"id":"mutate","trigger":"hand_played","loop_groups":[{
            "id":"side_effect_only","count":4,
            "effects":[{"effect_type":"play_sound","params":{"sound":"card1"}}]
        }]},
        {"id":"fallback","trigger":"hand_played","effects":[
            {"effect_type":"add_chips","params":{"value":12}},
            {"effect_type":"set_dollars","params":{"value":4}}
        ]}
    ]),"played_sounds=0;function play_sound(sound) assert(sound=='card1');played_sounds=played_sounds+1 end",
        "resolve_joker({joker_main=true})",
        "assert(played_sounds==4);assert(hand_chips==12 and mult==1 and G.GAME.dollars==4)");
}

fn append_game_variable_description_and_loop_cases(cases: &mut Vec<Value>) {
    for value_type in ["raw", "gameVariable", "game_var"] {
        let reference = "GAMEVAR:joker_count|2|1";
        let value = if value_type == "raw" {
            json!(reference)
        } else {
            json!({"value":reference,"valueType":value_type})
        };
        let mut definition = joker(json!([{
            "id":"score","trigger":"hand_played","effects":[{
                "id":"mult","effect_type":"add_mult","params":{"value":value}
            }]
        }]));
        definition.description_variables = Some(serde_json::from_value(json!([
            {"kind":"game","id":"joker_count","multiplier":2,"startsFrom":1},
            {"kind":"config","name":"mult0","effect_id":"mult","fallback":value},
            {"kind":"game","id":"hand_level","multiplier":2,"startsFrom":1},
            {"kind":"config","name":"unused","fallback":{
                "value":"GAMEVAR:hand_level|2|1","valueType":"gameVariable"
            }}
        ])).unwrap());
        cases.push(json!({"kind":"description_game","name":format!("description_game_scaled_{value_type}"),
            "ids":["joker_count","joker_count","hand_level","hand_level"],
            "multiplier":2,"starts_from":1,
            "code":Emitter::new().emit_chunk(&compile_joker(&definition,"mod"))}));

        let xmult_value = if value_type == "raw" {
            json!("GAMEVAR:joker_count|0|2")
        } else {
            json!({"value":"GAMEVAR:joker_count|0|2","valueType":value_type})
        };
        scoring_case(cases,&format!("scoring_loop_game_variable_{value_type}"),json!([{
            "id":"scaled_loop","trigger":"hand_played",
            "effects":[{"effect_type":"add_mult","params":{"value":4}}],
            "loop_groups":[{"id":"repeat","count":value,"effects":[
                {"effect_type":"add_chips","params":{"value":value}},
                {"effect_type":"apply_x_mult","params":{"value":xmult_value}},
                {"effect_type":"set_dollars","params":{"value":value}},
                {"effect_type":"modify_internal_variable","params":{"variable_name":"counter","operation":"increment","value":1}}
            ]}]
        }]),"","resolve_joker({joker_main=true});assert(hand_chips==49 and mult==640 and G.GAME.dollars==49 and actor.ability.extra.counter==7);G.jokers.cards={owned_jokers[1]};resolve_joker({joker_main=true})",
            "assert(hand_chips==58 and mult==5152 and G.GAME.dollars==58 and actor.ability.extra.counter==10);assert(message_count('x_mult')==10)");
    }

    for (name, count, prepare, iterations) in [
        ("zero_jokers",json!({"value":"GAMEVAR:joker_count|1|0","valueType":"gameVariable"}),"G.jokers.cards={}",0),
        ("negative",json!({"value":"GAMEVAR:joker_count|-1|1","valueType":"gameVariable"}),"",0),
        ("fractional",json!({"value":"GAMEVAR:joker_count|0.5|0","valueType":"gameVariable"}),"",1),
        ("legacy_catalog_id",json!({"value":"joker_count","valueType":"gameVariable"}),"",3),
        ("invalid_reference",json!({"value":"GAMEVAR:missing|2|1","valueType":"gameVariable"}),"",0),
        ("malformed_reference",json!("GAMEVAR:joker_count|NaN|0"),"",0),
        ("numeric_string",json!("3"),"",3),
    ] {
        scoring_case(cases,&format!("scoring_loop_game_variable_count_{name}"),json!([{
            "id":"dynamic_count","trigger":"hand_played",
            "effects":[{"effect_type":"add_chips","params":{"value":9}}],
            "loop_groups":[{"id":"repeat","count":count,"effects":[
                {"effect_type":"add_chips","params":{"value":2}},
                {"effect_type":"set_dollars","params":{"value":1}},
                {"effect_type":"modify_internal_variable","params":{"variable_name":"counter","operation":"increment","value":1}}
            ]}]
        }]),prepare,"resolve_joker({joker_main=true})",
            &format!("assert(hand_chips=={} and mult==1 and G.GAME.dollars=={iterations} and actor.ability.extra.counter=={iterations})",9+2*iterations));
    }

    scoring_case(cases,"scoring_loop_game_variable_rechecks_each_iteration",json!([{
        "id":"dynamic_amount","trigger":"hand_played",
        "loop_groups":[{"id":"repeat","count":3,"effects":[
            {"effect_type":"level_up_hand","params":{"hand_selection":"Pair","value":1}},
            {"effect_type":"add_chips","params":{"value":{"value":"GAMEVAR:pair_level|1|0","valueType":"gameVariable"}}},
            {"effect_type":"apply_x_mult","params":{"value":{"value":"GAMEVAR:pair_level|1|0","valueType":"gameVariable"}}},
            {"effect_type":"set_dollars","params":{"value":{"value":"GAMEVAR:pair_level|2|1","valueType":"gameVariable"}}}
        ]}]
    }]),"G.GAME.hands.Pair.level=2;SMODS.smart_level_up_hand=function(card,hand,instant,amount) G.GAME.hands[hand].level=G.GAME.hands[hand].level+amount end",
        "resolve_joker({joker_main=true})",
        "assert(G.GAME.hands.Pair.level==5);assert(hand_chips==12 and mult==60 and G.GAME.dollars==27)");
}

fn append_retrigger_scoring_cases(cases: &mut Vec<Value>) {
    let scoring_effects=json!([
        {"effect_type":"add_chips","params":{"value":10}},
        {"effect_type":"apply_x_mult","params":{"value":2}},
        {"effect_type":"set_dollars","params":{"value":3}},
        {"effect_type":"permanent_bonus","params":{"bonus_type":"perma_bonus","value":5}},
        {"effect_type":"modify_internal_variable","params":{"variable_name":"counter","operation":"increment","value":1}}
    ]);
    let prepare="played_card={ability={perma_bonus=0},config={center={key='c_base'}}}";
    let discovery_checks="assert(hand_chips==0 and mult==1 and G.GAME.dollars==0);assert(played_card.ability.perma_bonus==0 and actor.ability.extra.counter==0);assert(#status_messages==0 and #repetition_warnings==0)";
    let scoring_checks="assert(hand_chips==30);assert(mult==8);assert(G.GAME.dollars==9);assert(played_card.ability.perma_bonus==15);assert(actor.ability.extra.counter==3);assert(message_count('x_mult')==3)";
    for (name,effect_type,params) in [
        ("canonical_parameter","retrigger",json!({"repetitions":2,"value":1})),
        ("legacy_value","retrigger",json!({"value":2})),
        ("playing_card_alias","retrigger_playing_card",json!({"repetitions":2})),
        ("cards_alias","retrigger_cards",json!({"repetitions":2})),
    ] {
        let mut effects=scoring_effects.as_array().unwrap().clone();
        effects.insert(0,json!({"effect_type":effect_type,"params":params}));
        scoring_case(cases,&format!("scoring_retrigger_mixed_{name}"),json!([{
            "id":"mixed", "trigger":"card_scored", "retrigger":true,"effects":effects
        }]),prepare,
            &format!("local reps=collect_repetitions({{cardarea=G.play,other_card=played_card}});assert(#reps==2);{discovery_checks};for i=0,#reps do resolve_joker({{individual=true,cardarea=G.play,other_card=played_card}}) end"),
            scoring_checks);
    }
    scoring_case(cases,"scoring_retrigger_separate_rule",json!([
        {"id":"score","trigger":"card_scored","effects":scoring_effects.clone()},
        {"id":"repeat","trigger":"card_scored","retrigger":true,"effects":[
            {"effect_type":"retrigger","params":{"repetitions":2}}
        ]}
    ]),prepare,
        &format!("local reps=collect_repetitions({{cardarea=G.play,other_card=played_card}});assert(#reps==2);{discovery_checks};for i=0,#reps do resolve_joker({{individual=true,cardarea=G.play,other_card=played_card}}) end"),
        scoring_checks);
    scoring_case(cases,"scoring_retrigger_stale_flag_without_retrigger_effect",json!([{
        "id":"score","trigger":"card_scored","retrigger":true,"effects":scoring_effects.clone()
    }]),prepare,
        &format!("local reps=collect_repetitions({{cardarea=G.play,other_card=played_card}});assert(#reps==0);{discovery_checks};resolve_joker({{individual=true,cardarea=G.play,other_card=played_card}})"),
        "assert(hand_chips==10 and mult==2 and G.GAME.dollars==3);assert(played_card.ability.perma_bonus==5 and actor.ability.extra.counter==1)");

    let mut grouped_effects=scoring_effects.as_array().unwrap().clone();
    grouped_effects.push(json!({"effect_type":"retrigger","params":{"repetitions":2}}));
    for chance_succeeds in [true,false] {
        scoring_case(cases,&format!("scoring_retrigger_chance_group_{chance_succeeds}"),json!([{
            "id":"chance","trigger":"card_scored","retrigger":true,
            "random_groups":[{"id":"mixed","chance_numerator":1,"chance_denominator":2,"effects":grouped_effects.clone()}]
        }]),&format!("{prepare};SMODS.pseudorandom_probability=function() return {chance_succeeds} end"),
            &format!("local reps=collect_repetitions({{cardarea=G.play,other_card=played_card}});assert(#reps=={});{discovery_checks};for i=0,#reps do resolve_joker({{individual=true,cardarea=G.play,other_card=played_card}}) end",if chance_succeeds {2}else{0}),
            if chance_succeeds {scoring_checks}else{"assert(hand_chips==0 and mult==1 and G.GAME.dollars==0);assert(played_card.ability.perma_bonus==0 and actor.ability.extra.counter==0)"});
    }
    scoring_case(cases,"scoring_retrigger_loop_group",json!([{
        "id":"loop","trigger":"card_scored","retrigger":true,
        "loop_groups":[{"id":"mixed","count":2,"effects":grouped_effects}]
    }]),prepare,
        &format!("local reps=collect_repetitions({{cardarea=G.play,other_card=played_card}});assert(#reps==4);{discovery_checks};for i=0,#reps do resolve_joker({{individual=true,cardarea=G.play,other_card=played_card}}) end"),
        "assert(hand_chips==100);assert(mult==1024);assert(G.GAME.dollars==30);assert(played_card.ability.perma_bonus==50);assert(actor.ability.extra.counter==10);assert(message_count('x_mult')==10)");

    for (trigger,end_of_round) in [
        ("card_held_in_hand",false),
        ("card_held_in_hand_end_of_round",true),
    ] {
        let mut effects=scoring_effects.as_array().unwrap().clone();
        effects.push(json!({"effect_type":"retrigger","params":{"repetitions":2}}));
        let rules=json!([{"id":"held","trigger":trigger,"retrigger":true,"effects":effects}]);
        for blueprint in [false,true] {
            scoring_case(cases,&format!("scoring_retrigger_{trigger}_blueprint_{blueprint}"),rules.clone(),prepare,
                &format!("local reps=collect_repetitions({{cardarea=G.hand,other_card=played_card,end_of_round={end_of_round},card_effects={{{{chips=1}}}},blueprint={blueprint}}});assert(#reps==2);{discovery_checks};for i=0,#reps do resolve_joker({{individual=true,cardarea=G.hand,other_card=played_card,end_of_round={end_of_round},blueprint={blueprint}}}) end"),
                scoring_checks);
        }
        scoring_case(cases,&format!("scoring_retrigger_{trigger}_wrong_phase"),rules,prepare,
            &format!("local reps=collect_repetitions({{cardarea=G.hand,other_card=played_card,end_of_round={},card_effects={{{{chips=1}}}}}});assert(#reps==0);resolve_joker({{individual=true,cardarea=G.hand,other_card=played_card,end_of_round={}}});{discovery_checks}",!end_of_round,!end_of_round),
            "assert(actor.ability.extra.counter==0 and played_card.ability.perma_bonus==0)");
    }

    let mut incompatible=joker(json!([{"id":"mixed","trigger":"card_scored","retrigger":true,"effects":[
        {"effect_type":"retrigger","params":{"repetitions":2}},
        {"effect_type":"add_chips","params":{"value":10}},
        {"effect_type":"apply_x_mult","params":{"value":2}},
        {"effect_type":"set_dollars","params":{"value":3}}
    ]}]));
    incompatible.blueprint_compat=false;
    cases.push(json!({"kind":"scoring","name":"scoring_retrigger_incompatible_blueprint",
        "code":Emitter::new().emit_chunk(&compile_joker(&incompatible,"mod")),
        "prepare":"G.GAME.dollars=0;G.play={cards={}};played_card={ability={}}",
        "invoke":"local reps=collect_repetitions({cardarea=G.play,other_card=played_card,blueprint=true});assert(#reps==0);assert(not resolve_joker({individual=true,cardarea=G.play,other_card=played_card,blueprint=true}));assert(hand_chips==0 and mult==1 and G.GAME.dollars==0);reps=collect_repetitions({cardarea=G.play,other_card=played_card});assert(#reps==2);for i=0,#reps do resolve_joker({individual=true,cardarea=G.play,other_card=played_card}) end",
        "verify":"assert(hand_chips==30 and mult==8 and G.GAME.dollars==9)"}));

    let mut tooltip=joker(json!([{"id":"mixed","trigger":"card_scored","retrigger":true,
        "effects":[{"effect_type":"retrigger","params":{"repetitions":1}}],
        "random_groups":[{"id":"normal_chance","chance_numerator":1,"chance_denominator":2,
            "effects":[{"effect_type":"apply_x_mult","params":{"value":2}}]}]
    }]));
    tooltip.description_variables=Some(serde_json::from_value(json!([
        {"kind":"probability","group_id":"normal_chance","part":"numerator"},
        {"kind":"probability","group_id":"normal_chance","part":"denominator"}
    ])).unwrap());
    cases.push(json!({"kind":"scoring","name":"scoring_retrigger_normal_chance_live_tooltip",
        "code":Emitter::new().emit_chunk(&compile_joker(&tooltip,"mod")),
        "prepare":"G.GAME.dollars=0;G.play={cards={}};played_card={ability={}};actor.ability.extra.numerator_0=3;actor.ability.extra.odds_0=7;chance_calls=0;SMODS.pseudorandom_probability=function(card,key,n,d) assert(n==3 and d==7);chance_calls=chance_calls+1;return true end",
        "invoke":"local vars=test_definition:loc_vars({},actor).vars;assert(vars[1]==3 and vars[2]==7);local reps=collect_repetitions({cardarea=G.play,other_card=played_card});assert(#reps==1 and chance_calls==0);resolve_joker({individual=true,cardarea=G.play,other_card=played_card})",
        "verify":"assert(mult==2 and chance_calls==1);local vars=test_definition:loc_vars({},actor).vars;assert(vars[1]==3 and vars[2]==7)"}));
}

fn append_card_retrigger_cases(cases: &mut Vec<Value>) {
    for object in ["enhancement","seal","edition"] {
        for grouping in ["direct","chance","loop"] {
            let effects=json!([
                {"effect_type":"retrigger","params":{"repetitions":1}},
                {"effect_type":"add_chips","params":{"value":7}},
                {"effect_type":"apply_x_chips","params":{"value":2}},
                {"effect_type":"apply_x_mult","params":{"value":2}},
                {"effect_type":"set_dollars","params":{"value":2}},
                {"effect_type":"permanent_bonus","params":{"bonus_type":"perma_bonus","value":3}},
                {"effect_type":"modify_internal_variable","params":{"variable_name":"counter","operation":"increment","value":1}}
            ]);
            let mut rule=json!({"id":"mixed","trigger":"card_scored","retrigger":true,
                "condition_groups":[{"conditions":[{"condition_type":"player_money","params":{"operator":"greater_than","value":-1}}]}]});
            match grouping {
                "chance" => rule["random_groups"]=json!([{"id":"mixed","chance_numerator":1,"chance_denominator":1,"effects":effects}]),
                "loop" => rule["loop_groups"]=json!([{"id":"mixed","count":2,"effects":effects}]),
                _ => rule["effects"]=effects,
            }
            let input=json!({"key":"runtime_test","name":"Runtime Test","description":["Test"],
                "atlas":"CustomCards","pos":{"x":0,"y":0},"rules":[rule],
                "user_variables":[{"name":"counter","var_type":"number","initial_value":0}]});
            let chunk=match object {
                "seal" => compile_seal(&serde_json::from_value::<SealDef>(input).unwrap(),"mod"),
                "edition" => compile_edition(&serde_json::from_value::<EditionDef>(input).unwrap(),"mod"),
                _ => compile_enhancement(&serde_json::from_value::<EnhancementDef>(input).unwrap(),"mod"),
            };
            let variable_path=match object {
                "seal" => "actor.ability.seal.extra.counter",
                "edition" => "actor.edition.extra.counter",
                _ => "actor.ability.extra.counter",
            };
            let object_setup=match object {
                "seal" => "actor.ability.seal=copy_table(test_definition.config)",
                "edition" => "actor.edition=copy_table(test_definition.config)",
                _ => "actor.ability.extra=actor.ability.extra or {}",
            };
            let repetitions=if grouping=="loop" {2}else{1};
            let effect_count=if grouping=="loop" {6}else{2};
            cases.push(json!({"kind":"scoring","name":format!("scoring_retrigger_{object}_{grouping}"),
                "code":Emitter::new().emit_chunk(&chunk),
                "prepare":format!("G.GAME.dollars=0;G.play={{cards={{}}}};actor.ability.perma_bonus=0;{object_setup};SMODS.pseudorandom_probability=function() return true end;SMODS.get_card_areas=function() return {{}} end;eval_card=function(card,context) local effect=test_definition:calculate(card,context);return effect and {{[{object:?}]=effect}} or {{}},{{}} end;reset_score(1,1)"),
                "invoke":format!("local reps=collect_repetitions({{cardarea=G.play,other_card=actor}});assert(#reps=={repetitions});assert(#repetition_warnings==0);assert(hand_chips==1 and mult==1 and G.GAME.dollars==0);assert(actor.ability.perma_bonus==0 and {variable_path}==0);for i=0,#reps do resolve_joker({{playing_card=true,main_scoring=true,cardarea=G.play,other_card=actor}}) end"),
                "verify":format!("assert(hand_chips=={});assert(mult=={});assert(G.GAME.dollars=={});assert(actor.ability.perma_bonus=={});assert({variable_path}=={effect_count});assert(message_count('x_chips')=={effect_count} and message_count('x_mult')=={effect_count})",
                    15*(1<<effect_count)-14,1<<effect_count,2*effect_count,3*effect_count)}));
        }
    }
}

fn append_card_self_destruct_cases(cases: &mut Vec<Value>) {
    for object in ["seal", "enhancement", "edition"] {
        for scenario in ["direct", "chance", "chance_then_miss", "loop", "missed_chance", "missed_condition", "empty_loop", "legacy_alias", "retrigger", "held", "glass", "typed_glass", "legacy_glass", "typed_legacy_false"] {
            let destroy_type = if scenario == "legacy_alias" { "destroy_card" } else { "destroy_playing_card" };
            let trigger = if scenario == "held" { "card_held_in_hand" } else { "card_scored" };
            let destroy_params = match scenario {
                "glass" => json!({"set_glass_trigger":"y"}),
                "typed_glass" => json!({"set_glass_trigger":{"value":"y", "valueType":"text"}}),
                "legacy_glass" => json!({"setGlassTrigger":true}),
                "typed_legacy_false" => json!({"setGlassTrigger":{"value":false, "valueType":"text"}}),
                _ => json!({"set_glass_trigger":"n"}),
            };
            let mut effects = json!([
                {"effect_type":"apply_x_mult", "params":{"value":5}},
                {"effect_type":destroy_type, "params":destroy_params}
            ]);
            let mut rule = json!({"id":"self_destruct", "trigger":trigger, "destroy":true});
            match scenario {
                "chance" | "chance_then_miss" | "missed_chance" => {
                    rule["random_groups"] = json!([{"id":"destruction_chance", "chance_numerator":1, "chance_denominator":2, "effects":effects}]);
                }
                "loop" | "empty_loop" => {
                    let count = if scenario=="empty_loop" { json!({"value":"GAMEVAR:current_money|1|0", "valueType":"gameVariable"}) } else { json!(2) };
                    rule["loop_groups"] = json!([{"id":"destruction_loop", "count":count, "effects":effects}]);
                }
                "retrigger" => {
                    effects.as_array_mut().unwrap().insert(0, json!({"effect_type":"retrigger", "params":{"repetitions":1}}));
                    rule["retrigger"] = json!(true);
                    rule["effects"] = effects;
                }
                _ => rule["effects"] = effects,
            }
            if scenario == "missed_condition" {
                rule["condition_groups"] = json!([{"conditions":[{"condition_type":"player_money", "params":{"operator":"greater_than", "value":10}}]}]);
            }
            let input = json!({"key":"runtime_test", "name":"Runtime Test", "description":["Test"],
                "atlas":"CustomCards", "pos":{"x":0,"y":0}, "rules":[rule]});
            let chunk = match object {
                "seal" => compile_seal(&serde_json::from_value::<SealDef>(input).unwrap(), "mod"),
                "edition" => compile_edition(&serde_json::from_value::<EditionDef>(input).unwrap(), "mod"),
                _ => compile_enhancement(&serde_json::from_value::<EnhancementDef>(input).unwrap(), "mod"),
            };
            let object_setup = match object {
                "seal" => "actor.ability.seal=copy_table(test_definition.config or {});native_effect_key='seals'",
                "edition" => "actor.edition=copy_table(test_definition.config or {});native_effect_key='edition'",
                _ => "native_effect_key='enhancement'",
            };
            let succeeds = !matches!(scenario, "missed_chance" | "missed_condition" | "empty_loop");
            let expected_mult = if !succeeds { 1 } else if matches!(scenario, "loop" | "retrigger") { 25 } else { 5 };
            let area = if scenario == "held" { "G.hand" } else { "G.play" };
            let scoring_hand = if scenario == "held" { "nil" } else { "{actor}" };
            let probability = if scenario == "chance_then_miss" {
                "chance_calls=0;SMODS.pseudorandom_probability=function() chance_calls=chance_calls+1;return chance_calls==1 end".to_string()
            } else {
                format!("SMODS.pseudorandom_probability=function() return {} end",scenario!="missed_chance")
            };
            let score_call = format!("SMODS.score_card(actor,{{cardarea={area},full_hand={{actor}},scoring_hand={scoring_hand}}});");
            let score_calls = if scenario == "chance_then_miss" { format!("{score_call}{score_call}assert(chance_calls==2);") } else { score_call };
            let glass_check = if matches!(scenario,"glass"|"typed_glass"|"legacy_glass") { "assert(actor.glass_trigger==true);" } else { "assert(not actor.glass_trigger);" };
            cases.push(json!({"kind":"scoring", "name":format!("card_self_destruct_{object}_{scenario}"),
                "card_destruction_runtime":true, "code":Emitter::new().emit_chunk(&chunk),
                "prepare":format!("G.GAME.dollars=0;G.play={{cards={{}}}};G.hand={{cards={{}}}};{object_setup};actor.config={{center={{key='c_base'}}}};actor.dissolve_calls=0;function actor:start_dissolve() self.dissolve_calls=self.dissolve_calls+1 end;innocent={{ability={{}},config={{center={{key='c_base'}}}}}};{area}.cards={{actor,innocent}};{probability};reset_score(0,1)"),
                "invoke":format!("{score_calls}assert(mult=={expected_mult}, 'scoring must finish before destruction');assert(not actor.destroyed and not actor.shattered and #event_queue==0);{glass_check}local cards_destroyed={{}};SMODS.calculate_destroying_cards({{cardarea={area},full_hand={{actor}},scoring_hand={scoring_hand}}},cards_destroyed,{scoring_hand})"),
                "verify":format!("assert(#cards_destroyed=={});{}assert(not innocent.should_destroy and not innocent.getting_sliced);run_events();assert(actor.dissolve_calls==0, 'Steamodded must own card removal')",if succeeds {1}else{0},if succeeds {"assert(cards_destroyed[1]==actor and actor.getting_sliced and (actor.destroyed or actor.shattered));"}else{"assert(not actor.should_destroy and not actor.getting_sliced);"})}));
        }
        let input = json!({"key":"runtime_test", "name":"Runtime Test", "description":["Test"],
            "atlas":"CustomCards", "pos":{"x":0,"y":0}, "rules":[{
                "id":"discard_destruction", "trigger":"card_discarded", "destroy":true,
                "effects":[{"effect_type":"destroy_playing_card", "params":{}}]
            }]});
        let chunk = match object {
            "seal" => compile_seal(&serde_json::from_value::<SealDef>(input).unwrap(), "mod"),
            "edition" => compile_edition(&serde_json::from_value::<EditionDef>(input).unwrap(), "mod"),
            _ => compile_enhancement(&serde_json::from_value::<EnhancementDef>(input).unwrap(), "mod"),
        };
        cases.push(json!({"kind":"scoring", "name":format!("card_self_destruct_{object}_discard"),
            "card_destruction_runtime":true, "code":Emitter::new().emit_chunk(&chunk),
            "prepare":format!("G.play={{cards={{}}}};G.hand={{cards={{actor}}}};actor.config={{center={{key='c_base'}}}};native_effect_key={:?}", if object=="seal" {"seals"} else {object}),
            "invoke":"local effects={};SMODS.calculate_context({discard=true,other_card=actor,full_hand={actor}},effects);local flags=SMODS.trigger_effects(effects,actor)",
            "verify":"assert(flags.remove, 'discard destruction must return native remove flag');assert(not actor.should_destroy and #event_queue==0)"}));
    }
}

fn deck_settings_case(
    cases: &mut Vec<Value>,
    name: &str,
    rules: Value,
    prepare: &str,
    invoke: &str,
    verify: &str,
) {
    cases.push(json!({"kind":"deck_settings", "name":format!("deck_settings_{name}"),
        "code":Emitter::new().emit_chunk(&compile_deck(&deck(rules), "mod")),
        "prepare":prepare, "invoke":invoke, "verify":verify}));
}

fn append_deck_settings_cases(cases: &mut Vec<Value>) {
    // Exercise actual generated Back.apply through the subsequent start_run
    // overwrites, rather than evaluating an isolated effect or returned table.
    for (effect, value, initial, field) in [
        ("edit_interest_cap", 10, 25, "G.GAME.interest_cap"),
        ("edit_hands", 2, 4, "G.GAME.round_resets.hands"),
        ("edit_discards", 1, 3, "G.GAME.round_resets.discards"),
        ("edit_hands_money", 2, 1, "G.GAME.modifiers.money_per_hand"),
        ("edit_consumable_slots", 1, 2, "G.consumeables.config.card_limit"),
    ] {
        for operation in ["add", "subtract", "set"] {
            let expected = match operation {
                "subtract" => initial - value,
                "set" => value,
                _ => initial + value,
            };
            let mut verify = format!("assert({field}=={expected})");
            match effect {
                "edit_hands" => verify.push_str(&format!(";assert(G.GAME.current_round.hands_left=={expected});G.GAME.current_round.hands_left=G.GAME.round_resets.hands;assert(G.GAME.current_round.hands_left=={expected})")),
                "edit_discards" => verify.push_str(&format!(";assert(G.GAME.current_round.discards_left=={expected});G.GAME.current_round.discards_left=G.GAME.round_resets.discards;assert(G.GAME.current_round.discards_left=={expected})")),
                "edit_interest_cap" => verify.push_str(&format!(";assert(interest_payout()=={})",expected/5)),
                "edit_hands_money" => verify.push_str(&format!(";assert(hand_payout()=={})",4*expected)),
                _ => {},
            }
            deck_settings_case(cases,&format!("{effect}_{operation}"),json!([{
                "id":"setting", "trigger":"card_used", "effects":[{
                    "effect_type":effect,"params":{"operation":operation,"value":value,"duration":"permanent"}
                }]
            }]),"","test_definition:apply(actor);finish_deck_startup();run_events()",&verify);
        }
    }
    for (operation, value, expected) in [("multiply",2,50),("divide",5,5)] {
        deck_settings_case(cases,&format!("interest_{operation}"),json!([{
            "id":"interest", "trigger":"card_used", "effects":[{
                "effect_type":"edit_interest_cap","params":{"operation":operation,"value":value}
            }]
        }]),"","test_definition:apply(actor);finish_deck_startup();run_events()",
            &format!("assert(G.GAME.interest_cap=={expected});assert(interest_payout()=={})",expected/5));
    }
    deck_settings_case(cases,"all_effects_same_rule",json!([{
        "id":"all", "trigger":"card_used", "effects":[
            {"effect_type":"edit_hands","params":{"operation":"add","value":2}},
            {"effect_type":"edit_interest_cap","params":{"operation":"set","value":40}},
            {"effect_type":"edit_discards","params":{"operation":"subtract","value":1}},
            {"effect_type":"edit_hands_money","params":{"operation":"set","value":4}},
            {"effect_type":"edit_consumable_slots","params":{"operation":"add","value":3}}
        ]
    }]),"old_consumables={cards={},config={card_limit=20}};G.consumeables=old_consumables",
        "test_definition:apply(actor);finish_deck_startup();run_events()",
        "assert(G.GAME.round_resets.hands==6 and G.GAME.current_round.hands_left==6);assert(G.GAME.round_resets.discards==2 and G.GAME.current_round.discards_left==2);assert(G.consumeables.config.card_limit==5);assert(old_consumables.config.card_limit==20);assert(hand_payout()==24);assert(interest_payout()==8)");
    deck_settings_case(cases,"all_matching_startup_rules",json!([
        {"id":"slots","trigger":"card_used","effects":[{"effect_type":"edit_consumable_slots","params":{"operation":"add","value":1}}]},
        {"id":"hands","trigger":"card_used","effects":[{"effect_type":"edit_hands","params":{"operation":"add","value":2}}]},
        {"id":"interest","trigger":"card_used","effects":[{"effect_type":"edit_interest_cap","params":{"operation":"add","value":10}}]},
        {"id":"money","trigger":"card_used","effects":[{"effect_type":"edit_hands_money","params":{"operation":"add","value":2}}]}
    ]),"","test_definition:apply(actor);finish_deck_startup();run_events()",
        "assert(G.consumeables.config.card_limit==3);assert(G.GAME.round_resets.hands==6 and G.GAME.current_round.hands_left==6);assert(interest_payout()==7);assert(hand_payout()==18)");
    deck_settings_case(cases,"conditional_and_unconditional_rules",json!([
        {"id":"true_first","trigger":"card_used",
            "condition_groups":[{"conditions":[{"condition_type":"player_money","params":{"operator":"greater_than","value":50}}]}],
            "effects":[{"effect_type":"edit_hands","params":{"operation":"add","value":1}}]},
        {"id":"false","trigger":"card_used",
            "condition_groups":[{"conditions":[{"condition_type":"player_money","params":{"operator":"greater_than","value":200}}]}],
            "effects":[{"effect_type":"edit_interest_cap","params":{"operation":"set","value":100}}]},
        {"id":"true_later","trigger":"card_used",
            "condition_groups":[{"conditions":[{"condition_type":"player_money","params":{"operator":"greater_than","value":25}}]}],
            "effects":[{"effect_type":"edit_consumable_slots","params":{"operation":"add","value":2}}]},
        {"id":"unconditional","trigger":"card_used",
            "effects":[{"effect_type":"edit_hands_money","params":{"operation":"set","value":3}}]}
    ]),"","test_definition:apply(actor);finish_deck_startup();run_events()",
        "assert(G.GAME.round_resets.hands==5 and G.GAME.current_round.hands_left==5);assert(G.consumeables.config.card_limit==4);assert(interest_payout()==5);assert(hand_payout()==15)");
    deck_settings_case(cases,"reordered_effects",json!([{
        "id":"reordered", "trigger":"card_used", "effects":[
            {"effect_type":"edit_consumable_slots","params":{"operation":"add","value":3}},
            {"effect_type":"edit_hands_money","params":{"operation":"set","value":4}},
            {"effect_type":"edit_discards","params":{"operation":"subtract","value":1}},
            {"effect_type":"edit_interest_cap","params":{"operation":"set","value":40}},
            {"effect_type":"edit_hands","params":{"operation":"add","value":2}}
        ]
    }]),"","test_definition:apply(actor);finish_deck_startup();run_events()",
        "assert(G.GAME.round_resets.hands==6 and G.GAME.current_round.hands_left==6);assert(G.GAME.round_resets.discards==2 and G.GAME.current_round.discards_left==2);assert(G.consumeables.config.card_limit==5);assert(hand_payout()==24);assert(interest_payout()==8)");
    let mut variable_definition=deck(json!([{
        "id":"variables", "trigger":"card_used", "effects":[
            {"effect_type":"edit_hands","params":{"operation":"add","value":{"value":"bonus","valueType":"userVariable"}}},
            {"effect_type":"edit_consumable_slots","params":{"operation":"add","value":{"value":"bonus","valueType":"userVariable"}}},
            {"effect_type":"edit_hands_money","params":{"operation":"set","value":{"value":"GAMEVAR:current_money|0.01|1","valueType":"gameVariable"}}}
        ]
    }]));
    variable_definition.user_variables=serde_json::from_value(json!([
        {"name":"bonus","var_type":"number","initial_value":2}
    ])).unwrap();
    cases.push(json!({"kind":"deck_settings", "name":"deck_settings_user_and_computed_values",
        "code":Emitter::new().emit_chunk(&compile_deck(&variable_definition,"mod")),
        "prepare":"", "invoke":"test_definition:apply(actor);finish_deck_startup();run_events()",
        "verify":"assert(test_definition.config.extra.bonus==2);assert(G.GAME.round_resets.hands==6 and G.GAME.current_round.hands_left==6);assert(G.consumeables.config.card_limit==4);assert(hand_payout()==12)"}));
    deck_settings_case(cases,"dependent_operations_across_rules",json!([
        {"id":"set","trigger":"card_used","effects":[
            {"effect_type":"edit_consumable_slots","params":{"operation":"set","value":4}},
            {"effect_type":"edit_hands","params":{"operation":"set","value":6}},
            {"effect_type":"edit_interest_cap","params":{"operation":"set","value":40}},
            {"effect_type":"edit_hands_money","params":{"operation":"set","value":3}}
        ]},
        {"id":"modify","trigger":"card_used","effects":[
            {"effect_type":"edit_consumable_slots","params":{"operation":"add","value":3}},
            {"effect_type":"edit_hands","params":{"operation":"subtract","value":2}},
            {"effect_type":"edit_interest_cap","params":{"operation":"add","value":10}},
            {"effect_type":"edit_hands_money","params":{"operation":"subtract","value":1}}
        ]},
        {"id":"modify_again","trigger":"card_used","effects":[
            {"effect_type":"edit_consumable_slots","params":{"operation":"subtract","value":2}},
            {"effect_type":"edit_hands","params":{"operation":"add","value":1}},
            {"effect_type":"edit_interest_cap","params":{"operation":"multiply","value":2}},
            {"effect_type":"edit_hands_money","params":{"operation":"add","value":2}}
        ]}
    ]),"","test_definition:apply(actor);finish_deck_startup();run_events()",
        "assert(G.GAME.round_resets.hands==5 and G.GAME.current_round.hands_left==5);assert(G.consumeables.config.card_limit==5);assert(interest_payout()==20);assert(hand_payout()==20)");
    deck_settings_case(cases,"grouped_startup_effects",json!([{
        "id":"grouped", "trigger":"card_used",
        "effects":[{"effect_type":"edit_interest_cap","params":{"operation":"add","value":10}}],
        "random_groups":[{"id":"chance","chance_numerator":1,"chance_denominator":2,"effects":[
            {"effect_type":"edit_discards","params":{"operation":"add","value":1}}
        ]}],
        "loop_groups":[{"id":"repeat","count":2,"effects":[
            {"effect_type":"edit_hands","params":{"operation":"add","value":1}},
            {"effect_type":"edit_consumable_slots","params":{"operation":"add","value":1}},
            {"effect_type":"edit_hands_money","params":{"operation":"add","value":1}}
        ]}]
    }]),"","test_definition:apply(actor);finish_deck_startup();run_events()",
        "assert(G.GAME.round_resets.hands==6 and G.GAME.current_round.hands_left==6);assert(G.GAME.round_resets.discards==4 and G.GAME.current_round.discards_left==4);assert(G.consumeables.config.card_limit==4);assert(hand_payout()==18);assert(interest_payout()==7)");
    deck_settings_case(cases,"stake_discard_penalty",json!([{
        "id":"discard", "trigger":"card_used", "effects":[{"effect_type":"edit_discards","params":{"operation":"add","value":2}}]
    }]),"G.GAME.starting_params.discards=2","test_definition:apply(actor);finish_deck_startup();run_events()",
        "assert(G.GAME.round_resets.discards==4 and G.GAME.current_round.discards_left==4)");
    deck_settings_case(cases,"consumable_slots_subtract_clamped",json!([{
        "id":"slots", "trigger":"card_used", "effects":[{"effect_type":"edit_consumable_slots","params":{"operation":"subtract","value":5}}]
    }]),"","test_definition:apply(actor);finish_deck_startup();run_events()",
        "assert(G.consumeables.config.card_limit==0)");
    deck_settings_case(cases,"runtime_counter_effects",json!([{
        "id":"runtime", "trigger":"hand_played", "effects":[
            {"effect_type":"edit_hands","params":{"operation":"add","value":2}},
            {"effect_type":"edit_discards","params":{"operation":"subtract","value":1}},
            {"effect_type":"edit_consumable_slots","params":{"operation":"set","value":5}}
        ]
    }]),"finish_deck_startup()",
        "local effect=test_definition:calculate(actor,{main_eval=true});assert(effect);SMODS.calculate_effect(effect,actor);run_events()",
        "assert(G.GAME.round_resets.hands==6 and G.GAME.current_round.hands_left==6);assert(G.GAME.round_resets.discards==2 and G.GAME.current_round.discards_left==2);assert(G.consumeables.config.card_limit==5)");
}

fn append_joker_creation_cases(cases: &mut Vec<Value>) {
    let effect = json!({"effect_type":"create_joker", "params":{
        "joker_type":"specific", "joker_key":"j_joker"
    }});
    let rules = json!([{"id":"create", "trigger":"card_used", "effects":[effect.clone()]}]);
    let code = Emitter::new().emit_chunk(&compile_deck(&deck(rules.clone()), "mod"));
    // Back:apply runs before the new run's Joker area exists. Replace the area
    // only after invoking the generated callback, then execute queued events.
    for (name, prepare, initialize, expected, buffer) in [
        ("missing_area", "G.jokers=nil", "G.jokers=joker_area(5,0)", 1, 0),
        ("stale_full_area", "old_area=joker_area(5,5);G.jokers=old_area", "G.jokers=joker_area(5,0)", 1, 0),
        ("full_new_area", "G.jokers=nil", "G.jokers=joker_area(2,2)", 0, 0),
        ("zero_slots", "G.jokers=nil", "G.jokers=joker_area(0,0)", 0, 0),
        ("pending_buffer_full", "G.jokers=nil;G.GAME.joker_buffer=1", "G.jokers=joker_area(1,0)", 0, 1),
        ("pending_buffer_room", "G.jokers=nil;G.GAME.joker_buffer=1", "G.jokers=joker_area(3,1)", 1, 1),
        ("missing_buffer", "G.jokers=nil;G.GAME.joker_buffer=nil", "G.jokers=joker_area(5,0)", 1, 0),
    ] {
        cases.push(json!({"kind":"joker_creation", "name":format!("deck_create_joker_{name}"),
            "code":code, "prepare":prepare,
            "invoke":format!("test_definition:apply(actor);assert(#created_cards==0);assert(#event_queue==1);{initialize};run_events()"),
            "verify":format!("assert(#created_cards=={expected});assert((G.GAME.joker_buffer or 0)=={buffer});if created_cards[1] then assert(created_cards[1].params.key=='j_joker');assert(created_cards[1].params.set=='Joker');assert(created_cards[1].buffer_at_add=={}) end;if old_area then assert(#old_area.cards==5) end",buffer+1)}));
    }
    let mut multiple_rules = rules.clone();
    multiple_rules[0]["effects"] = json!([effect.clone(), effect.clone(), effect.clone(), effect.clone()]);
    cases.push(json!({"kind":"joker_creation", "name":"deck_create_joker_multiple_effects",
        "code":Emitter::new().emit_chunk(&compile_deck(&deck(multiple_rules), "mod")),
        "prepare":"G.jokers=nil", "invoke":"test_definition:apply(actor);assert(#event_queue==4);G.jokers=joker_area(2,0);run_events()",
        "verify":"assert(#created_cards==2 and #G.jokers.cards==2);assert(G.GAME.joker_buffer==0);assert(created_cards[1].buffer_at_add==1 and created_cards[2].buffer_at_add==1)"}));
    cases.push(json!({"kind":"joker_creation", "name":"deck_create_joker_repeated_callbacks",
        "code":code, "prepare":"G.jokers=nil",
        "invoke":"test_definition:apply(actor);test_definition:apply(actor);test_definition:apply(actor);assert(#event_queue==3);G.jokers=joker_area(2,0);run_events()",
        "verify":"assert(#created_cards==2 and #G.jokers.cards==2);assert(G.GAME.joker_buffer==0)"}));

    for (name, params) in [
        ("ignore_slots_checkbox", json!({"ignoreSlots":true})),
        ("ignore_slots_selector", json!({"ignore_slots":"ignore"})),
        ("negative_edition", json!({"edition":"negative"})),
    ] {
        let mut bypass_rules = rules.clone();
        for (key, value) in params.as_object().unwrap() {
            bypass_rules[0]["effects"][0]["params"][key] = value.clone();
        }
        cases.push(json!({"kind":"joker_creation", "name":format!("deck_create_joker_{name}"),
            "code":Emitter::new().emit_chunk(&compile_deck(&deck(bypass_rules), "mod")),
            "prepare":"G.jokers=nil;G.GAME.joker_buffer=2",
            "invoke":"test_definition:apply(actor);assert(#event_queue==1);G.jokers=joker_area(0,0);run_events()",
            "verify":format!("assert(#created_cards==1);assert(G.GAME.joker_buffer==2);assert(created_cards[1].buffer_at_add==2);{}",
                if name=="negative_edition" {"assert(created_cards[1].params.edition=='e_negative')"} else {"assert(created_cards[1].params.edition==nil)"})}));
    }

    let calculate_rules = json!([{"id":"create", "trigger":"hand_played", "effects":[effect]}]);
    for object in ["deck", "joker"] {
        let chunk = if object == "deck" {
            compile_deck(&deck(calculate_rules.clone()), "mod")
        } else {
            compile_joker(&joker(calculate_rules.clone()), "mod")
        };
        let context = if object == "deck" {"{main_eval=true}"} else {"{joker_main=true}"};
        cases.push(json!({"kind":"joker_creation", "name":format!("create_joker_{object}_calculate_reserves_slots"),
            "code":Emitter::new().emit_chunk(&chunk), "prepare":"G.jokers=joker_area(1,0)",
            "invoke":format!("test_definition:calculate(actor,{context});test_definition:calculate(actor,{context});assert(#event_queue==1);assert(G.GAME.joker_buffer==1);assert(#created_cards==0);run_events()"),
            "verify":"assert(#created_cards==1 and #G.jokers.cards==1);assert(G.GAME.joker_buffer==0);assert(created_cards[1].buffer_at_add==1)"}));
    }
}

/// Exercise registration and the real generated callback, rather than duplicating
/// the compiler's Lua snippets in the runner.
fn rule_option_case(
    cases: &mut Vec<Value>,
    name: &str,
    object: &str,
    effect: &str,
    params: Value,
    prepare: &str,
    verify: &str,
) {
    let trigger = if object == "voucher_passive" || object == "joker_passive" {
        "passive"
    } else if object == "joker" {
        "hand_played"
    } else {
        "card_used"
    };
    let rules = json!([{"id":"rule_options", "trigger":trigger,
        "effects":[{"id":"option_effect", "effect_type":effect,"params":params}]}]);
    let variables = json!([
        {"name":"chosen", "var_type":"key", "initial_value":"j_first"},
        {"name":"source", "var_type":"key", "initial_value":"j_second"},
        {"name":"rankvar", "var_type":"rank", "initial_value":"A"},
        {"name":"suitvar", "var_type":"suit", "initial_value":"Spades"},
        {"name":"pokerhandvar", "var_type":"poker_hand", "initial_value":"Pair"},
        {"name":"rank_source", "var_type":"rank", "initial_value":"K"},
        {"name":"suit_source", "var_type":"suit", "initial_value":"Hearts"},
        {"name":"amount", "var_type":"number", "initial_value":3},
        {"name":"hand_source", "var_type":"poker_hand", "initial_value":"Pair"}
    ]);
    let mut definition = joker(rules.clone());
    definition.user_variables = serde_json::from_value(variables.clone()).unwrap();
    let chunk = match object {
        "consumable" => {
            let definition: ConsumableDef = serde_json::from_value(json!({
                "key":"runtime_test","name":"Runtime Test","description":["Test"],
                "set":"Tarot","atlas":"CustomConsumables","pos":{"x":0,"y":0},
                "user_variables":variables,"rules":rules
            }))
            .unwrap();
            compile_consumable(&definition, "mod")
        }
        "voucher" | "voucher_passive" => {
            let definition: VoucherDef = serde_json::from_value(json!({
                "key":"runtime_test","name":"Runtime Test","description":["Test"],
                "atlas":"CustomVouchers","pos":{"x":0,"y":0},
                "user_variables":variables,"rules":rules
            }))
            .unwrap();
            compile_voucher(&definition, "mod")
        }
        _ => compile_joker(&definition, "mod"),
    };
    let invoke = match object {
        "consumable" => "test_definition:use(actor,nil,nil)",
        "voucher" | "voucher_passive" => "test_definition:redeem(actor)",
        _ => "test_definition:calculate(actor,{joker_main=true,other_card=observed_card,other_joker=observed_joker})",
    };
    cases.push(json!({"kind":"rule_options","name":name,"object":object,
        "code":Emitter::new().emit_chunk(&chunk),"prepare":prepare,"invoke":invoke,"verify":verify}));
}

fn append_rule_option_cases(cases: &mut Vec<Value>) {
    for tag in ["negative", "d_six", "top_up"] {
        rule_option_case(
            cases,
            &format!("key_tag_picker_{tag}"),
            "joker",
            "change_key_variable",
            json!({"variable_name":"chosen","key_type":"tag","tag_change_type":"specific","specific_tag":tag}),
            &format!("G.P_TAGS={{['tag_{tag}']={{key='tag_{tag}'}}}};G.P_CENTERS['tag_{tag}']=nil"),
            &format!("assert(actor.ability.extra.chosen=='tag_{tag}',actor.ability.extra.chosen)"),
        );
    }
    rule_option_case(
        cases,
        "key_tag_custom_registry",
        "joker",
        "change_key_variable",
        json!({"variable_name":"chosen","key_type":"tag","tag_change_type":"specific","specific_tag":"registered"}),
        "G.P_TAGS={tag_registered={key='tag_registered'}};G.P_CENTERS.tag_registered=nil",
        "assert(actor.ability.extra.chosen=='tag_registered',actor.ability.extra.chosen)",
    );
    rule_option_case(cases,"key_tag_random_string_pool","joker","change_key_variable",
        json!({"variable_name":"chosen","key_type":"tag","tag_change_type":"random"}),
        "G.P_CENTER_POOLS.Tag={'tag_negative'};G.P_TAGS={tag_negative={key='tag_negative'}};G.P_CENTERS.tag_negative=nil",
        "assert(actor.ability.extra.chosen=='tag_negative',actor.ability.extra.chosen)");
    for (mode, amount, held) in [
        ("literal", json!(3), true),
        (
            "scoped",
            json!({"value":"amount","valueType":"userVariable"}),
            true,
        ),
        ("absent", json!(3), false),
        ("debuffed", json!(3), true),
    ] {
        let prepare=format!("actor.config={{center={{key='j_mod_runtime_test'}}}};actor.debuff={};G.jokers.cards={}",
            mode=="debuffed",if held {"{actor}"}else{"{}"});
        rule_option_case(
            cases,
            &format!("discount_joker_passive_{mode}"),
            "joker_passive",
            "discount_items",
            json!({"discount_type":"jokers","discount_method":"flat_reduction","discount_amount":amount}),
            &prepare,
            if held && mode != "debuffed" {
                "assert(shop_cards[6].cost==17);assert(shop_cards[1].cost==20);shop_cards[6]:set_cost();assert(shop_cards[6].cost==17);G.jokers.cards={};test_definition:remove_from_deck(actor);run_events();assert(shop_cards[6].cost==20)"
            } else {
                "assert(shop_cards[6].cost==20)"
            },
        );
        cases.last_mut().unwrap()["invoke"] =
            json!("test_definition:add_to_deck(actor);run_events()");
    }
    let condition_definition = joker(json!([{"id":"discount_condition","trigger":"passive",
        "condition_groups":[{"conditions":[{"condition_type":"player_money","params":{"operator":"greater_than","value":10}}]}],
        "effects":[{"effect_type":"discount_items","params":{"discount_type":"jokers","discount_method":"flat_reduction","discount_amount":3}}]}]));
    cases.push(json!({"kind":"rule_options","name":"discount_joker_condition",
        "code":Emitter::new().emit_chunk(&compile_joker(&condition_definition,"mod")),
        "prepare":"actor.config={center={key='j_mod_runtime_test'}};G.jokers.cards={actor};G.GAME.dollars=5",
        "invoke":"test_definition:add_to_deck(actor);run_events()",
        "verify":"assert(shop_cards[6].cost==20);G.GAME.dollars=15;shop_cards[6]:set_cost();assert(shop_cards[6].cost==17);G.GAME.dollars=0;shop_cards[6]:set_cost();assert(shop_cards[6].cost==20)"}));
    rule_option_case(cases,"discount_default_options","voucher","discount_items",json!({}),"",
        "assert(shop_cards[1].cost==0 and shop_cards[9].cost==0);assert(shop_cards[2].cost==20 and shop_cards[6].cost==20)");
    let first:VoucherDef=serde_json::from_value(json!({"key":"first","name":"First","description":["Test"],"atlas":"CustomVouchers","pos":{"x":0,"y":0},
        "rules":[{"id":"one","trigger":"card_used","effects":[{"effect_type":"discount_items","params":{"discount_type":"jokers","discount_method":"flat_reduction","discount_amount":3}}]}]})).unwrap();
    cases.push(json!({"kind":"rule_options","name":"discount_resume_saved_run",
        "setup":"G.GAME.jf_item_discounts={{item_type='jokers',method='flat_reduction',amount=3}}",
        "code":Emitter::new().emit_chunk(&compile_voucher(&first,"mod")),
        "invoke":"shop_cards[6]:set_cost()",
        "verify":"assert(shop_cards[6].cost==17);assert(#G.GAME.jf_item_discounts==1);local future=option_card('future','Joker');future:set_cost();assert(future.cost==17);shop_cards[6]:set_cost();assert(shop_cards[6].cost==17)"}));
    rule_option_case(cases,"discount_preserves_sell_value_api","voucher","discount_items",
        json!({"discount_type":"jokers","discount_method":"flat_reduction","discount_amount":3}),
        "shop_cards[6].ability.extra_value=7",
        "assert(shop_cards[6].cost==17);assert(shop_cards[6].sell_cost==15);assert(shop_cards[6].sell_update_calls>0)");
    let mut stacking = joker(json!([{"id":"stacking","trigger":"passive",
        "condition_groups":[{"conditions":[{"condition_type":"internal_variable","params":{"variable_name":"enabled","operator":"equals","value":1}}]}],
        "effects":[{"effect_type":"discount_items","params":{"discount_type":"jokers","discount_method":"flat_reduction","discount_amount":{"value":"amount","valueType":"userVariable"}}}]}]));
    stacking.user_variables = serde_json::from_value(json!([
        {"name":"amount","var_type":"number","initial_value":3},
        {"name":"enabled","var_type":"number","initial_value":1}
    ]))
    .unwrap();
    cases.push(json!({"kind":"rule_options","name":"discount_joker_stacking_and_conditions",
        "code":Emitter::new().emit_chunk(&compile_joker(&stacking,"mod")),
        "prepare":"actor.config={center={key='j_mod_runtime_test'}};actor.ability.extra.enabled=0;local other_extra={};for k,v in pairs(actor.ability.extra) do other_extra[k]=v end;other_extra.enabled=1;other_extra.amount=5;other_actor={config=actor.config,ability={extra=other_extra}};G.jokers.cards={actor,other_actor}",
        "invoke":"test_definition:add_to_deck(actor);run_events()",
        "verify":"assert(shop_cards[6].cost==15);actor.ability.extra.enabled=1;shop_cards[6]:set_cost();assert(shop_cards[6].cost==12);shop_cards[6]:set_cost();assert(shop_cards[6].cost==12);actor.debuff=true;shop_cards[6]:set_cost();assert(shop_cards[6].cost==15);other_actor.ability.extra.enabled=0;shop_cards[6]:set_cost();assert(shop_cards[6].cost==20)"}));
    let mut updating = joker(json!([{"id":"live_discount","trigger":"passive",
        "condition_groups":[{"conditions":[{"condition_type":"player_money","params":{"operator":"greater_than","value":10}}]}],
        "effects":[{"effect_type":"discount_items","params":{"discount_type":"jokers","discount_method":"flat_reduction","discount_amount":{"value":"amount","valueType":"userVariable"}}}]}]));
    updating.user_variables = serde_json::from_value(json!([
        {"name":"amount","var_type":"number","initial_value":3}
    ]))
    .unwrap();
    cases.push(json!({"kind":"rule_options","name":"discount_joker_live_update",
        "code":Emitter::new().emit_chunk(&compile_joker(&updating,"mod")),
        "prepare":"actor.config={center={key='j_mod_runtime_test'}};actor.area=G.jokers;G.jokers.cards={actor};G.GAME.dollars=15",
        "invoke":"test_definition:update(actor,0.016)",
        "verify":"assert(shop_cards[6].cost==17);local calls=shop_cards[6].cost_calls;test_definition:update(actor,0.016);assert(shop_cards[6].cost_calls==calls);actor.ability.extra.amount=7;test_definition:update(actor,0.016);assert(shop_cards[6].cost==13);G.GAME.dollars=0;test_definition:update(actor,0.016);assert(shop_cards[6].cost==20);G.GAME.dollars=15;test_definition:update(actor,0.016);assert(shop_cards[6].cost==13);actor.debuff=true;test_definition:update(actor,0.016);assert(shop_cards[6].cost==20);actor.debuff=false;test_definition:update(actor,0.016);assert(shop_cards[6].cost==13)"}));
    let second:VoucherDef=serde_json::from_value(json!({"key":"second","name":"Second","description":["Test"],"atlas":"CustomVouchers","pos":{"x":0,"y":0},
        "rules":[{"id":"two","trigger":"card_used","effects":[{"effect_type":"discount_items","params":{"discount_type":"jokers","discount_method":"percentage_reduction","discount_amount":25}}]}]})).unwrap();
    cases.push(json!({"kind":"rule_options","name":"discount_multiple_vouchers_install_once",
        "code":format!("{}\nfirst_definition=test_definition\n{}",Emitter::new().emit_chunk(&compile_voucher(&first,"mod")),Emitter::new().emit_chunk(&compile_voucher(&second,"mod"))),
        "prepare":"first_actor={ability=first_definition.config or {extra={}}};first_definition:redeem(first_actor);first_cost_hook=Card.set_cost;assert(shop_cards[6].cost==17)",
        "invoke":"test_definition:redeem(actor)",
        "verify":"assert(Card.set_cost==first_cost_hook);assert(#G.GAME.jf_item_discounts==2);assert(shop_cards[6].cost==12);shop_cards[6]:set_cost();assert(shop_cards[6].cost==12);assert(shop_cards[1].cost==20)"}));
    let mut mixed = first.clone();
    mixed.rules.extend(second.rules.clone());
    mixed.rules[1].trigger = "passive".into();
    cases.push(json!({"kind":"rule_options","name":"discount_mixed_voucher_rules",
        "code":Emitter::new().emit_chunk(&compile_voucher(&mixed,"mod")),
        "invoke":"test_definition:redeem(actor)",
        "verify":"assert(#G.GAME.jf_item_discounts==2);assert(shop_cards[6].cost==12);assert(shop_cards[1].cost==20);shop_cards[6]:set_cost();assert(shop_cards[6].cost==12)"}));
    let round_variables = "G.GAME.current_round.rankvar_card={rank='Ace',id=14};G.GAME.current_round.suitvar_card={suit='Spades'};G.GAME.current_round.pokerhandvar_hand='High Card';G.GAME.current_round.rank_source_card={rank='King',id=13};G.GAME.current_round.suit_source_card={suit='Hearts'}";
    for (effect,variable,pool_key,checkboxes,expected) in [
        ("change_rank_variable","rankvar","rank_pool",json!([false,false,false,false,false,false,false,false,false,false,false,true,false]),"assert(G.GAME.current_round.rankvar_card.rank=='King' and G.GAME.current_round.rankvar_card.id==13)"),
        ("change_suit_variable","suitvar","suit_pool",json!([false,true,false,false]),"assert(G.GAME.current_round.suitvar_card.suit=='Hearts')"),
        ("change_pokerhand_variable","pokerhandvar","pokerhand_pool",json!([false,true,false,false,false,false,false,false,false,false,false,false]),"assert(G.GAME.current_round.pokerhandvar_hand=='Pair')"),
    ] {
        for legacy in [false,true] {
            let mut params=json!({"variable_name":variable,"change_type":"pool"});
            params[pool_key]=if legacy {json!(serde_json::to_string(&checkboxes).unwrap())}else{json!({"value":checkboxes,"valueType":"checkbox"})};
            rule_option_case(cases,&format!("round_pool_{variable}_legacy_{legacy}"),"joker",effect,params,round_variables,expected);
        }
        let mut params=json!({"variable_name":variable,"change_type":"pool"});
        let empty=vec![false;checkboxes.as_array().unwrap().len()];
        params[pool_key]=json!({"value":empty,"valueType":"checkbox"});
        rule_option_case(cases,&format!("round_pool_empty_{variable}"),"joker",effect,params,round_variables,
            "assert(G.GAME.current_round.rankvar_card.id==14);assert(G.GAME.current_round.suitvar_card.suit=='Spades');assert(G.GAME.current_round.pokerhandvar_hand=='High Card')");
    }
    for (effect,variable,param,source,expected) in [
        ("change_rank_variable","rankvar","specific_rank","rank_source","assert(G.GAME.current_round.rankvar_card.id==13 and G.GAME.current_round.rankvar_card.rank=='King')"),
        ("change_suit_variable","suitvar","specific_suit","suit_source","assert(G.GAME.current_round.suitvar_card.suit=='Hearts')"),
    ] {
        let mut params=json!({"variable_name":variable,"change_type":"specific"});
        params[param]=json!({"value":source,"valueType":"userVariable"});
        rule_option_case(cases,&format!("round_copy_{variable}"),"joker",effect,params,round_variables,expected);
        rule_option_case(cases,&format!("round_missing_context_{variable}"),"joker",effect,
            json!({"variable_name":variable,"change_type":"held_card"}),&format!("{round_variables};observed_card=nil"),
            "assert(G.GAME.current_round.rankvar_card.id==14);assert(G.GAME.current_round.suitvar_card.suit=='Spades')");
    }
    rule_option_case(
        cases,
        "round_hand_typed_copy",
        "joker",
        "change_pokerhand_variable",
        json!({"variable_name":"pokerhandvar","change_type":"specific","specific_pokerhand":{"value":"hand_source","valueType":"userVariable"}}),
        &format!("{round_variables};G.GAME.current_round.hand_source_hand='Pair'"),
        "assert(G.GAME.current_round.pokerhandvar_hand=='Pair')",
    );
    rule_option_case(
        cases,
        "round_hand_most_played",
        "joker",
        "change_pokerhand_variable",
        json!({"variable_name":"pokerhandvar","change_type":"most_played"}),
        round_variables,
        "assert(G.GAME.current_round.pokerhandvar_hand=='Pair')",
    );
    rule_option_case(
        cases,
        "round_hand_least_played",
        "joker",
        "change_pokerhand_variable",
        json!({"variable_name":"pokerhandvar","change_type":"least_played"}),
        "G.GAME.current_round.pokerhandvar_hand='Pair'",
        "assert(G.GAME.current_round.pokerhandvar_hand=='High Card')",
    );
    // The assertions describe editor behavior independently of the generated
    // category predicates, including future cards and price refreshes.
    for (discount_type, affected) in [
        ("planet", "planet,celestial"), ("tarot", "tarot,arcana"),
        ("spectral", "spectral,spectral_pack"),
        ("standard", "enhanced,plain,standard_pack"),
        ("jokers", "joker"), ("vouchers", "voucher"),
        ("all_consumables", "planet,tarot,spectral,custom"),
        ("all_cards", "planet,tarot,spectral,enhanced,plain,joker,arcana,celestial,spectral_pack,standard_pack,buffoon,custom"),
        ("all_shop_items", "planet,tarot,spectral,enhanced,plain,joker,voucher,arcana,celestial,spectral_pack,standard_pack,buffoon,custom"),
    ] {
        for (method,amount,price) in [("flat_reduction",3,17),("percentage_reduction",25,15),("make_free",1,0)] {
            let names: Vec<&str> = affected.split(',').collect();
            let verify = format!(
                "local affected={{}};for _,key in ipairs({}) do affected[key]=true end;\
                for _,c in ipairs(shop_cards) do local expected=affected[c.config.center.key] and {price} or 20;\
                assert(c.cost==expected,c.config.center.key..':'..tostring(c.cost));\
                c:set_cost();c:set_cost();assert(c.cost==expected,'repeat '..c.config.center.key);\
                assert(c.sell_cost==math.max(1,math.floor(expected/2)));\
                local future=option_card(c.config.center.key,c.ability.set);\
                future.config.center=c.config.center;future:set_cost();\
                assert(future.cost==expected,'future '..c.config.center.key) end",
                format!("{{{}}}",names.iter().map(|name|serde_json::to_string(name).unwrap()).collect::<Vec<_>>().join(","))
            );
            rule_option_case(cases,&format!("discount_{discount_type}_{method}"),"voucher","discount_items",
                json!({"discount_type":discount_type,"discount_method":method,"discount_amount":amount}),"",&verify);
        }
    }
    rule_option_case(cases,"discount_voucher_passive","voucher_passive","discount_items",
        json!({"discount_type":"jokers","discount_method":"flat_reduction","discount_amount":3}),"",
        "assert(shop_cards[6].cost==17);assert(shop_cards[1].cost==20);shop_cards[6]:set_cost();assert(shop_cards[6].cost==17)");
    rule_option_case(cases,"discount_typed_amount","voucher","discount_items",
        json!({"discount_type":"jokers","discount_method":"flat_reduction","discount_amount":{"value":"GAMEVAR:current_money|2|1","valueType":"gameVariable"}}),
        "G.GAME.dollars=2","assert(shop_cards[6].cost==15);assert(shop_cards[1].cost==20);G.GAME.dollars=10;shop_cards[6]:set_cost();assert(shop_cards[6].cost==15)");
    rule_option_case(cases,"discount_legacy_global","voucher","discount_items",
        json!({"operation":"set","value":25}),"",
        "assert(G.GAME.discount_percent==25);for _,c in ipairs(shop_cards) do assert(c.cost==15) end");
    for (key_type, specific, expected) in [
        ("joker", "j_second", "j_second"),
        ("consumable", "c_second", "c_second"),
        ("enhancement", "bonus", "m_bonus"),
        ("seal", "Gold", "Gold"),
        ("edition", "foil", "e_foil"),
        ("booster", "p_second", "p_second"),
        ("voucher", "v_second", "v_second"),
        ("tag", "tag_second", "tag_second"),
        ("joker", "custom", "j_mod_custom"),
        ("edition", "sparkle", "e_mod_sparkle"),
        ("consumable", "c_other_custom", "c_other_custom"),
    ] {
        let mut params = json!({"variable_name":"chosen","key_type":key_type});
        params[format!("{key_type}_change_type")] = json!("specific");
        params[format!("specific_{key_type}")] = json!(specific);
        rule_option_case(
            cases,
            &format!("key_specific_{key_type}_{specific}"),
            "joker",
            "change_key_variable",
            params,
            "",
            &format!(
                "assert(actor.ability.extra.chosen=={},actor.ability.extra.chosen)",
                serde_json::to_string(expected).unwrap()
            ),
        );
    }
    for (key_type, random_type, extra, expected) in [
        ("joker", "all", json!({}), "j_first"),
        ("joker", "unlocked", json!({}), "j_second"),
        ("joker", "locked", json!({}), "j_first"),
        (
            "joker",
            "rarity",
            json!({"joker_rarity":"rare"}),
            "j_second",
        ),
        (
            "joker",
            "pool",
            json!({"joker_pool":"chosen_pool"}),
            "j_second",
        ),
        ("joker", "owned", json!({}), "j_first"),
        (
            "consumable",
            "set",
            json!({"consumable_set":"Planet"}),
            "c_second",
        ),
        ("consumable", "owned", json!({}), "c_first"),
        (
            "booster",
            "category",
            json!({"booster_category":"Celestial"}),
            "p_second",
        ),
        (
            "booster",
            "size",
            json!({"booster_size_extra":5,"booster_size_choose":2}),
            "p_second",
        ),
        ("voucher", "possible", json!({}), "v_second"),
    ] {
        let mut params = json!({"variable_name":"chosen","key_type":key_type});
        params[format!("{key_type}_change_type")] = json!("random");
        params[format!("{key_type}_random_type")] = json!(random_type);
        params
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        rule_option_case(
            cases,
            &format!("key_random_{key_type}_{random_type}"),
            "joker",
            "change_key_variable",
            params,
            "",
            &format!(
                "assert(actor.ability.extra.chosen=={},actor.ability.extra.chosen)",
                serde_json::to_string(expected).unwrap()
            ),
        );
    }
    for (count, initial, expected) in [
        (1, "j_first", "j_second"),
        (2, "j_second", "j_first"),
        (-1, "j_first", "j_third"),
        (1, "j_missing", "j_missing"),
    ] {
        rule_option_case(
            cases,
            &format!("key_increment_{count}_{initial}"),
            "joker",
            "change_key_variable",
            json!({"variable_name":"chosen","key_type":"joker","joker_change_type":"increment","joker_increment_count":count}),
            &format!(
                "actor.ability.extra.chosen={}",
                serde_json::to_string(initial).unwrap()
            ),
            &format!(
                "assert(actor.ability.extra.chosen=={},actor.ability.extra.chosen)",
                serde_json::to_string(expected).unwrap()
            ),
        );
    }
    for type_tag in ["userVariable", "user_var"] {
        rule_option_case(
            cases,
            &format!("key_copy_{type_tag}"),
            "joker",
            "change_key_variable",
            json!({"variable_name":"chosen","key_type":"joker","joker_change_type":{"value":"source","valueType":type_tag}}),
            "",
            "assert(actor.ability.extra.chosen=='j_second',actor.ability.extra.chosen)",
        );
    }
    rule_option_case(
        cases,
        "key_legacy_specific",
        "joker",
        "change_key_variable",
        json!({"variable_name":"chosen","key_type":"joker","change_type":"specific","specific_key":"j_third"}),
        "",
        "assert(actor.ability.extra.chosen=='j_third',actor.ability.extra.chosen)",
    );
    rule_option_case(
        cases,
        "key_empty_pool",
        "joker",
        "change_key_variable",
        json!({"variable_name":"chosen","key_type":"joker","joker_change_type":"random"}),
        "G.P_CENTER_POOLS.Joker={}",
        "assert(actor.ability.extra.chosen=='j_first',actor.ability.extra.chosen)",
    );
    for token in ["scored_card", "evaled_joker", "selected_joker"] {
        let expected = if token == "scored_card" {
            "m_bonus"
        } else {
            "j_second"
        };
        rule_option_case(
            cases,
            &format!("key_context_{token}"),
            "joker",
            "change_key_variable",
            json!({"variable_name":"chosen","key_type":"joker","joker_change_type":token}),
            "",
            &format!(
                "assert(actor.ability.extra.chosen=={},actor.ability.extra.chosen)",
                serde_json::to_string(expected).unwrap()
            ),
        );
        rule_option_case(
            cases,
            &format!("key_context_missing_{token}"),
            "joker",
            "change_key_variable",
            json!({"variable_name":"chosen","key_type":"joker","joker_change_type":token}),
            "observed_card=nil;observed_joker=nil;G.jokers.highlighted={}",
            "assert(actor.ability.extra.chosen=='j_first',actor.ability.extra.chosen)",
        );
    }
    for (set,key,prepare,expected) in [
        ("any","random","","c_first"),
        ("planet","random","","c_second"),
        ("Tarot","c_mod_custom","","c_mod_custom"),
        ("tarot","the_fool","owned_consumables[1].config.center.key='c_fool'","c_fool"),
        ("Spectral","random_set:Spectral","","c_third"),
        ("customset","random","owned_consumables[4].ability.set='customset';owned_consumables[4].config.center.set='customset'","c_mod_custom"),
        ("planet","c_first","",""),
        ("tarot","c_missing","",""),
        ("any","random","G.consumeables=nil",""),
    ] {
        rule_option_case(cases,&format!("destroy_{set}_{key}"),"joker","destroy_consumable",
            json!({"consumable_type":set,"specific_card":key}),prepare,
            &format!("assert(#destroyed=={});{}",if expected.is_empty(){0}else{1},
                if expected.is_empty(){String::new()}else{format!("assert(destroyed[1]=={})",serde_json::to_string(expected).unwrap())}));
    }
    for target in ["random", "selected_joker", "evaled_joker"] {
        let expected = if target == "random" { 1 } else { 2 };
        rule_option_case(cases,&format!("edit_target_{target}"),"consumable","edit_joker",
            json!({"target":target,"edition":"foil","sticker":"rental"}),
            "context={other_joker=owned_jokers[2]}",
            &format!("assert(owned_jokers[{expected}].edition=='e_foil');assert(owned_jokers[{expected}].ability.rental);for i,c in ipairs(owned_jokers) do assert((c.edition_calls or 0)==(i=={expected} and 1 or 0)) end"));
    }
    for (edition, expected) in [
        ("sparkle", "e_mod_sparkle"),
        ("e_other_custom", "e_other_custom"),
        ("random", "e_holo"),
    ] {
        rule_option_case(
            cases,
            &format!("edit_edition_{edition}"),
            "consumable",
            "edit_joker",
            json!({"target":"random","edition":edition}),
            "",
            &format!(
                "assert(owned_jokers[1].edition=={})",
                serde_json::to_string(expected).unwrap()
            ),
        );
    }
    rule_option_case(cases,"edit_remove_stickers","consumable","edit_joker",
        json!({"target":"selected_joker","edition":"remove","sticker":"remove"}),
        "owned_jokers[2].edition='e_foil';owned_jokers[2].ability.eternal=true;owned_jokers[2].ability.rental=true;owned_jokers[2].ability.perishable=true",
        "assert(owned_jokers[2].edition==nil);assert(not owned_jokers[2].ability.eternal);assert(not owned_jokers[2].ability.rental);assert(not owned_jokers[2].ability.perishable)");
    rule_option_case(
        cases,
        "edit_missing_selection",
        "consumable",
        "edit_joker",
        json!({"target":"selected_joker","edition":"foil","sticker":"rental"}),
        "G.jokers.highlighted={}",
        "for _,c in ipairs(owned_jokers) do assert(c.edition_calls==nil) end",
    );
    rule_option_case(
        cases,
        "edit_empty_area",
        "consumable",
        "edit_joker",
        json!({"target":"random","edition":"foil"}),
        "G.jokers=nil",
        "for _,c in ipairs(owned_jokers) do assert(c.edition_calls==nil) end",
    );
    rule_option_case(
        cases,
        "edit_legacy_selected",
        "consumable",
        "edit_joker",
        json!({"selection_method":"selected","edition":"foil"}),
        "",
        "assert(owned_jokers[2].edition=='e_foil')",
    );
    rule_option_case(
        cases,
        "edit_key_variable",
        "consumable",
        "edit_joker",
        json!({"selection_method":"keyvar","key_variable":"source","edition":"foil"}),
        "",
        "assert(owned_jokers[2].edition=='e_foil');assert(owned_jokers[1].edition==nil)",
    );
    rule_option_case(
        cases,
        "edit_specific_custom_key",
        "consumable",
        "edit_joker",
        json!({"selection_method":"specific","joker_key":"j_other_custom","edition":"foil"}),
        "owned_jokers[2].config.center.key='j_other_custom'",
        "assert(owned_jokers[2].edition=='e_foil')",
    );
    rule_option_case(cases,"edit_random_selects_once","consumable","edit_joker",
        json!({"target":"random","edition":"foil","sticker":"rental"}),
        "selection_calls=0;function pseudorandom_element(pool) selection_calls=selection_calls+1;return pool[selection_calls] end",
        "assert(selection_calls==1);assert(owned_jokers[1].edition=='e_foil');assert(owned_jokers[1].ability.rental);assert(owned_jokers[2].edition==nil)");
    rule_option_case(
        cases,
        "edit_typed_context",
        "consumable",
        "edit_joker",
        json!({"target":{"value":"selected_joker","valueType":"contextVariable"},"edition":"foil"}),
        "",
        "assert(owned_jokers[2].edition=='e_foil')",
    );
    rule_option_case(
        cases,
        "edit_removed_target",
        "consumable",
        "edit_joker",
        json!({"target":"selected_joker","edition":"foil"}),
        "owned_jokers[2].removed=true",
        "assert(owned_jokers[2].edition==nil)",
    );
    rule_option_case(
        cases,
        "destroy_legacy_params",
        "joker",
        "destroy_consumable",
        json!({"set":"planet","consumable_key":"c_second"}),
        "",
        "assert(#destroyed==1 and destroyed[1]=='c_second')",
    );
    rule_option_case(
        cases,
        "destroy_skips_removed",
        "joker",
        "destroy_consumable",
        json!({"consumable_type":"Tarot","specific_card":"random"}),
        "owned_consumables[1].removed=true",
        "assert(#destroyed==1 and destroyed[1]=='c_mod_custom')",
    );
    for (kind, params, prepare, expected) in [
        (
            "rank",
            json!({"variable_name":"rankvar","specific_rank":"A"}),
            "{rank='Ace',id=14}",
            true,
        ),
        (
            "rank",
            json!({"variable_name":"rankvar","specific_rank":"A"}),
            "{rank='King',id=13}",
            false,
        ),
        (
            "rank",
            json!({"variable_name":"rankvar","specific_rank":"A"}),
            "{rank='Ace'}",
            true,
        ),
        (
            "rank",
            json!({"variable_name":"rankvar","rank":"King"}),
            "{rank='King',id=13}",
            true,
        ),
        (
            "suit",
            json!({"variable_name":"suitvar","specific_suit":"Hearts"}),
            "{suit='Hearts'}",
            true,
        ),
        (
            "suit",
            json!({"variable_name":"suitvar","specific_suit":"Hearts"}),
            "{suit='Spades'}",
            false,
        ),
        (
            "suit",
            json!({"variable_name":"suitvar","suit":"Diamonds"}),
            "{suit='Diamonds'}",
            true,
        ),
        (
            "rank",
            json!({"variable_name":"rankvar","specific_rank":"A"}),
            "nil",
            false,
        ),
        (
            "suit",
            json!({"variable_name":"suitvar","specific_suit":"Hearts"}),
            "nil",
            false,
        ),
    ] {
        let definition = joker(json!([{"id":"compare","trigger":"hand_played",
            "condition_groups":[{"conditions":[{"condition_type":format!("{kind}_variable"),"params":params}]}],
            "effects":[{"effect_type":"add_mult","params":{"value":9}}]}]));
        cases.push(json!({"kind":"rule_options","name":format!("compare_{kind}_{prepare}_{params}"),
            "code":Emitter::new().emit_chunk(&compile_joker(&definition,"mod")),
            "prepare":format!("G.GAME.current_round.{kind}var_card={prepare}"),
            "invoke":"comparison_result=test_definition:calculate(actor,{joker_main=true})",
            "verify":if expected {"assert(comparison_result and comparison_result.mult==9)"}else{"assert(not comparison_result or comparison_result.mult==nil)"}}));
    }
    for (name,condition_type,params,prepare,expected) in [
        ("specific_match","pokerhand_variable",json!({"check_type":"specific","specific_pokerhand":"Pair"}),"G.GAME.current_round.chosen_hand='Pair'",true),
        ("specific_mismatch","pokerhand_variable",json!({"check_type":"specific","specific_pokerhand":"Pair"}),"G.GAME.current_round.chosen_hand='High Card'",false),
        ("most_match","pokerhand_variable",json!({"check_type":"most_played"}),"G.GAME.current_round.chosen_hand='Pair'",true),
        ("most_mismatch","pokerhand_variable",json!({"check_type":"most_played"}),"G.GAME.current_round.chosen_hand='High Card'",false),
        ("least_match","pokerhand_variable",json!({"check_type":"least_played"}),"G.GAME.current_round.chosen_hand='High Card'",true),
        ("least_mismatch","pokerhand_variable",json!({"check_type":"least_played"}),"G.GAME.current_round.chosen_hand='Pair'",false),
        ("most_hidden","pokerhand_variable",json!({"check_type":"most_played"}),"G.GAME.current_round.chosen_hand='Pair';for _,hand in pairs(G.GAME.hands) do hand.visible=false end",false),
        ("least_hidden","pokerhand_variable",json!({"check_type":"least_played"}),"G.GAME.current_round.chosen_hand='High Card';for _,hand in pairs(G.GAME.hands) do hand.visible=false end",false),
        ("most_tied_zero","pokerhand_variable",json!({"check_type":"most_played"}),"G.GAME.current_round.chosen_hand='High Card';for _,hand in pairs(G.GAME.hands) do hand.played=0 end",true),
        ("least_tied_zero","pokerhand_variable",json!({"check_type":"least_played"}),"G.GAME.current_round.chosen_hand='Pair';for _,hand in pairs(G.GAME.hands) do hand.played=0 end",false),
        ("specific_nil_game","pokerhand_variable",json!({"check_type":"specific","specific_pokerhand":"Pair"}),"G.GAME=nil",false),
        ("most_nil_game","pokerhand_variable",json!({"check_type":"most_played"}),"G.GAME=nil",false),
        ("least_nil_round","pokerhand_variable",json!({"check_type":"least_played"}),"G.GAME.current_round=nil",false),
        ("most_missing_variable","pokerhand_variable",json!({"check_type":"most_played"}),"G.GAME.current_round.chosen_hand=nil",false),
        ("legacy_specific_alias","poker_hand_variable",json!({"hand":"Pair"}),"G.GAME.current_round.chosen_hand='Pair'",true),
        ("unknown_comparison","pokerhand_variable",json!({"check_type":"future_unsupported_option"}),"G.GAME.current_round.chosen_hand='High Card'",false),
    ] {
        let mut params=params;
        params["variable_name"]=json!("chosen");
        let definition=joker(json!([{"id":"compare_hand","trigger":"hand_played",
            "condition_groups":[{"conditions":[{"condition_type":condition_type,"params":params}]}],
            "effects":[{"effect_type":"add_mult","params":{"value":9}}]}]));
        cases.push(json!({"kind":"rule_options","name":format!("compare_hand_{name}"),
            "code":Emitter::new().emit_chunk(&compile_joker(&definition,"mod")),
            "prepare":prepare,"invoke":"comparison_result=test_definition:calculate(actor,{joker_main=true})",
            "verify":if expected {"assert(comparison_result and comparison_result.mult==9)"}else{"assert(not comparison_result or comparison_result.mult==nil)"}}));
    }
}

fn joker_condition_case(
    cases: &mut Vec<Value>,
    name: &str,
    trigger: &str,
    conditions: Value,
    prepare: &str,
    context: &str,
    expected: bool,
) {
    let mut definition = joker(json!([{
        "id":"joker_condition", "trigger":trigger,
        "condition_groups":[{"conditions":conditions}],
        "effects":[{"effect_type":"add_mult","params":{"value":9}}]
    }]));
    definition.user_variables = serde_json::from_value(json!([
        {"name":"source", "var_type":"key", "initial_value":"j_second"},
        {"name":"global_source", "var_type":"key", "initial_value":"j_third", "is_global":true},
        {"name":"persistent_source", "var_type":"key", "initial_value":"j_first", "is_global":true, "is_persistent":true}
    ])).unwrap();
    cases.push(json!({"kind":"rule_options", "name":name,
        "code":Emitter::new().emit_chunk(&compile_joker(&definition,"mod")),
        "prepare":format!("G.GAME.jf_global_vars={{global_source='j_third'}};JF_GLOBALS={{persistent_source='j_first'}};{prepare}"),
        "invoke":format!("comparison_result=test_definition:calculate(actor,{context})"),
        "verify":if expected {"assert(comparison_result and comparison_result.mult==9)"} else {"assert(not comparison_result or comparison_result.mult==nil)"}
    }));
}

fn append_joker_selection_and_key_cases(cases: &mut Vec<Value>) {
    for (name, params, prepare, expected) in [
        ("any_multiple", json!({"check_key":"any"}), "G.jokers.highlighted={owned_jokers[1],owned_jokers[2]}", true),
        ("any_empty", json!({"check_key":"any"}), "G.jokers.highlighted={}", false),
        ("any_missing_area", json!({"check_key":"any"}), "G.jokers=nil", false),
        ("any_missing_highlighted", json!({"check_key":"any"}), "G.jokers.highlighted=nil", false),
        ("any_no_game", json!({"check_key":"any"}), "G=nil", false),
        ("key_second_highlighted", json!({"check_key":"key","joker_key":"second"}), "G.jokers.highlighted={owned_jokers[1],owned_jokers[2]}", true),
        ("key_third_highlighted", json!({"check_key":"key","joker_key":"j_third"}), "G.jokers.highlighted={owned_jokers[1],owned_jokers[2],owned_jokers[3]}", true),
        ("key_not_selected", json!({"check_key":"key","joker_key":"j_third"}), "G.jokers.highlighted={owned_jokers[1],owned_jokers[2]}", false),
        ("key_malformed_selection", json!({"check_key":"key","joker_key":"j_second"}), "G.jokers.highlighted={{},{config={}},owned_jokers[2]}", true),
        ("key_missing_area", json!({"check_key":"key","joker_key":"j_second"}), "G.jokers=nil", false),
        ("legacy_key_alias", json!({"mode":"key","jokerKey":"second"}), "G.jokers.highlighted={owned_jokers[1],owned_jokers[2]}", true),
        ("rarity_second_highlighted", json!({"check_key":"rarity","rarity":"rare"}), "owned_jokers[1].config.center.rarity=1;owned_jokers[2].config.center.rarity=3;G.jokers.highlighted={owned_jokers[1],owned_jokers[2]}", true),
        ("rarity_not_selected", json!({"check_key":"rarity","rarity":"rare"}), "owned_jokers[1].config.center.rarity=1;G.jokers.highlighted={owned_jokers[1]}", false),
    ] {
        joker_condition_case(cases,&format!("joker_selected_{name}"),"hand_played",
            json!([{"condition_type":"joker_selected","params":params}]),prepare,"{joker_main=true}",expected);
    }
    for (name, prepare, expected) in [
        ("both", "G.jokers.highlighted={owned_jokers[1],owned_jokers[2]}", true),
        ("one_missing", "G.jokers.highlighted={owned_jokers[2]}", false),
    ] {
        joker_condition_case(cases,&format!("joker_selected_two_keys_{name}"),"hand_played",
            json!([
                {"condition_type":"joker_selected","params":{"check_key":"key","joker_key":"j_first"}},
                {"condition_type":"joker_selected","params":{"check_key":"key","joker_key":"j_second"}}
            ]),prepare,"{joker_main=true}",expected);
    }
    for (name, prepare, expected) in [
        ("match", "G.jokers.highlighted={owned_jokers[1],owned_jokers[2]}", false),
        ("missing", "G.jokers.highlighted={owned_jokers[1]}", true),
    ] {
        joker_condition_case(cases,&format!("joker_selected_negated_{name}"),"hand_played",
            json!([{"condition_type":"joker_selected","negate":true,"params":{"check_key":"key","joker_key":"j_second"}}]),
            prepare,"{joker_main=true}",expected);
    }

    for (name, trigger, params, prepare, context, negate, expected) in [
        ("triggered_match", "joker_triggered", json!({"joker_key":"second"}), "", "{post_trigger=true,other_card=owned_jokers[2]}", false, true),
        ("triggered_mismatch", "joker_triggered", json!({"joker_key":"j_second"}), "", "{post_trigger=true,other_card=owned_jokers[1]}", false, false),
        ("triggered_target_priority", "joker_triggered", json!({"joker_key":"j_second"}), "", "{post_trigger=true,other_card=owned_jokers[2],other_joker=owned_jokers[1]}", false, true),
        ("evaluated_match", "joker_evaluated", json!({"joker_key":"j_second"}), "", "{other_joker=owned_jokers[2]}", false, true),
        ("evaluated_mismatch", "joker_evaluated", json!({"joker_key":"j_second"}), "", "{other_joker=owned_jokers[1]}", false, false),
        ("evaluated_target_priority", "joker_evaluated", json!({"joker_key":"j_second"}), "", "{other_joker=owned_jokers[2],other_card=owned_jokers[1]}", false, true),
        ("triggered_missing", "joker_triggered", json!({"joker_key":"j_second"}), "", "{post_trigger=true}", false, false),
        ("triggered_malformed", "joker_triggered", json!({"joker_key":"j_second"}), "", "{post_trigger=true,other_card={config={}}}", false, false),
        ("triggered_self", "joker_triggered", json!({"joker_key":"j_second"}), "actor.config={center={key='j_second',set='Joker'}}", "{post_trigger=true,other_card=actor}", false, false),
        ("triggered_consumable", "joker_triggered", json!({"joker_key":"j_second"}), "owned_consumables[1].config.center.key='j_second'", "{post_trigger=true,other_card=owned_consumables[1]}", false, false),
        ("triggered_nonpost_context", "joker_triggered", json!({"joker_key":"j_second"}), "", "{other_card=owned_jokers[2]}", false, false),
        ("triggered_negated_match", "joker_triggered", json!({"joker_key":"j_second"}), "", "{post_trigger=true,other_card=owned_jokers[2]}", true, false),
        ("triggered_negated_mismatch", "joker_triggered", json!({"joker_key":"j_second"}), "", "{post_trigger=true,other_card=owned_jokers[1]}", true, true),
        ("local_key_variable", "joker_triggered", json!({"type":"variable","key_variable":"source"}), "", "{post_trigger=true,other_card=owned_jokers[2]}", false, true),
        ("global_key_variable", "joker_triggered", json!({"type":"variable","key_variable":"global_source"}), "", "{post_trigger=true,other_card=owned_jokers[3]}", false, true),
        ("persistent_key_variable", "joker_triggered", json!({"type":"variable","key_variable":"persistent_source"}), "", "{post_trigger=true,other_card=owned_jokers[1]}", false, true),
        ("missing_key_variable", "joker_triggered", json!({"type":"variable","key_variable":"unknown"}), "", "{post_trigger=true,other_card=owned_jokers[2]}", false, false),
        ("legacy_key_alias", "joker_triggered", json!({"selection_method":"key","jokerKey":"second"}), "", "{post_trigger=true,other_card=owned_jokers[2]}", false, true),
    ] {
        joker_condition_case(cases,&format!("joker_key_{name}"),trigger,
            json!([{"condition_type":"joker_key","negate":negate,"params":params}]),prepare,context,expected);
    }
    let definition = joker(json!([{
        "id":"observed", "trigger":"joker_triggered",
        "condition_groups":[{"conditions":[{"condition_type":"joker_key","params":{"joker_key":"j_second"}}]}],
        "effects":[{"effect_type":"add_mult","params":{"value":9}}]
    }]));
    let entry: export::BatchJokerEntry = serde_json::from_value(json!({
        "jokerData":{"objectKey":"runtime_test","name":"Runtime Test","description":"Test","cost":4,"rarity":"common",
            "rules":[{"id":"observed","trigger":"joker_triggered"}]},
        "pos":{"x":0,"y":0},"fileName":"runtime_test.lua"
    })).unwrap();
    let main = export::build_main_lua(&[entry], &[], &[], &[], &[], &[], &[], "mod", &[],
        false, false, false, false, false, false, false, &[]);
    cases.push(json!({"kind":"rule_options","name":"joker_key_steamodded_post_trigger_dispatch","post_trigger_runtime":true,
        "setup":"package.preload.nativefs=function() return {} end;SMODS.current_mod={can_load=true,optional_features={existing_feature=true}};SMODS.Atlas=function() end;SMODS.ObjectType=function() end;SMODS.load_file=function() return function() end end;",
        "code":format!("{main}\n{}",Emitter::new().emit_chunk(&compile_joker(&definition,"mod"))),
        "invoke":r#"
SMODS.mod_list={SMODS.current_mod};SMODS.get_optional_features()
assert(SMODS.optional_features.post_trigger==true)
assert(SMODS.optional_features.existing_feature==true)
actor.config={center={key='j_mod_runtime_test',set='Joker'}};actor.area=G.jokers
for _,observed in ipairs(owned_jokers) do
 observed.area=G.jokers
 function observed:calculate_joker(context) return {mult=1} end
end
evaluate_observed_joker(owned_jokers[2],{joker_main=true})
assert(#post_contexts==1 and post_contexts[1].other_card==owned_jokers[2])
assert(last_post_result and last_post_result.mult==9)
evaluate_observed_joker(owned_jokers[1],{joker_main=true})
assert(#post_contexts==2 and not last_post_result)
evaluate_observed_joker(owned_jokers[2],{post_trigger=true})
evaluate_observed_joker(owned_jokers[2],{retrigger_joker_check=true})
assert(#post_contexts==2)
SMODS.optional_features.post_trigger=false
evaluate_observed_joker(owned_jokers[2],{joker_main=true})
"#,
        "verify":"assert(#post_contexts==2)"
    }));
}

fn joker_selection_size_case(
    cases: &mut Vec<Value>,
    name: &str,
    object: &str,
    rules: Value,
    prepare: &str,
    invoke: &str,
    verify: &str,
) {
    let variables = json!([{"name":"amount","var_type":"number","initial_value":3}]);
    let chunk = match object {
        "deck" => {
            let mut definition = deck(rules);
            definition.user_variables = serde_json::from_value(variables).unwrap();
            compile_deck(&definition,"mod")
        }
        "voucher" => {
            let definition: VoucherDef = serde_json::from_value(json!({
                "key":"runtime_test","name":"Runtime Test","description":["Test"],
                "atlas":"CustomVouchers","pos":{"x":0,"y":0},
                "user_variables":variables,"rules":rules
            })).unwrap();
            compile_voucher(&definition,"mod")
        }
        "consumable" => {
            let definition: ConsumableDef = serde_json::from_value(json!({
                "key":"runtime_test","name":"Runtime Test","description":["Test"],
                "set":"Tarot","atlas":"CustomConsumables","pos":{"x":0,"y":0},
                "user_variables":variables,"rules":rules
            })).unwrap();
            compile_consumable(&definition,"mod")
        }
        _ => {
            let mut definition = joker(rules);
            definition.user_variables = serde_json::from_value(variables).unwrap();
            compile_joker(&definition,"mod")
        }
    };
    cases.push(json!({"kind":"rule_options","card_selection":true,"name":name,
        "code":Emitter::new().emit_chunk(&chunk),"prepare":format!("actor.ID=100;actor.sort_id=100;{prepare}"),"invoke":invoke,"verify":verify
    }));
}

fn append_joker_selection_size_cases(cases: &mut Vec<Value>) {
    for object in ["joker", "consumable", "voucher", "deck"] {
        for (operation, initial, value, expected) in [
            ("add", 1, 2.0, 3),
            ("subtract", 3, 2.0, 1),
            ("set", 1, 3.0, 3),
            ("set", 3, 1.9, 1),
            ("subtract", 3, 8.0, 1),
        ] {
            let trigger = if object == "joker" {"hand_played"} else {"card_used"};
            let rules = json!([{"id":"selection_limit","trigger":trigger,
                "effects":[{"effect_type":"edit_joker_size","params":{"operation":operation,"value":value}}]
            }]);
            let prepare = if object == "deck" {
                "actor={effect={center=test_definition,config=copy_table(test_definition.config or {})}};G.jokers=nil".to_string()
            } else {
                format!("G.jokers=make_joker_selection_area({initial});select_jokers()")
            };
            let invoke = match object {
                "deck" => format!("test_definition:apply(actor);assert(G.jokers==nil);G.jokers=make_joker_selection_area({initial});select_jokers();run_events()"),
                "voucher" => "test_definition:redeem(actor);run_events()".to_string(),
                "consumable" => "test_definition:use(actor,nil,nil);run_events()".to_string(),
                _ => "test_definition:calculate(actor,{joker_main=true});run_events()".to_string(),
            };
            joker_selection_size_case(cases,&format!("joker_selection_size_{object}_{operation}_{value}"),object,
                rules,&prepare,&invoke,
                &format!("assert(G.jokers.config.highlighted_limit=={expected});assert(#G.jokers.highlighted<=G.jokers.config.highlighted_limit);G.jokers.highlighted={{}};select_jokers();assert(#G.jokers.highlighted=={expected});assert(G.jokers.highlighted[#G.jokers.highlighted]==owned_jokers[3])"));
        }
    }
    joker_selection_size_case(cases,"joker_selection_size_deck_stale_area","deck",json!([{
        "id":"selection_limit","trigger":"card_used","effects":[{"effect_type":"edit_joker_size","params":{"operation":"add","value":2}}]
    }]),"actor={effect={center=test_definition,config=copy_table(test_definition.config or {})}};old_area=make_joker_selection_area(8);G.jokers=old_area",
        "test_definition:apply(actor);assert(old_area.config.highlighted_limit==8);G.jokers=make_joker_selection_area(1);run_events();select_jokers()",
        "assert(old_area.config.highlighted_limit==8);assert(G.jokers.config.highlighted_limit==3 and #G.jokers.highlighted==3)");

    for (name, operation, value, initial, applied, after_add) in [
        ("add", "add", json!(2), 1, 3, ""),
        ("subtract_clamped", "subtract", json!(8), 3, 1, ""),
        ("set", "set", json!(3), 1, 3, ""),
        ("typed_number", "add", json!({"value":2,"valueType":"number"}), 1, 3, ""),
        ("user_variable_changed", "add", json!({"value":"amount","valueType":"userVariable"}), 1, 4, "actor.ability.extra.amount=8"),
        ("game_variable_changed", "add", json!({"value":"GAMEVAR:joker_count|1|0","valueType":"gameVariable"}), 1, 4, "G.jokers.cards={owned_jokers[1]}"),
    ] {
        joker_selection_size_case(cases,&format!("joker_selection_size_passive_{name}"),"joker",json!([{
            "id":"selection_limit","trigger":"passive","effects":[{"effect_type":"edit_joker_size","params":{"operation":operation,"value":value}}]
        }]),&format!("G.jokers=make_joker_selection_area({initial});select_jokers()"),
            &format!("test_definition:add_to_deck(actor,false);assert(G.jokers.config.highlighted_limit=={applied});test_definition:add_to_deck(actor,false);assert(G.jokers.config.highlighted_limit=={applied});assert(#G.jokers.highlighted<=G.jokers.config.highlighted_limit)\n{after_add}\ntest_definition:remove_from_deck(actor,false);test_definition:remove_from_deck(actor,false)"),
            &format!("assert(G.jokers.config.highlighted_limit=={initial});assert(#G.jokers.highlighted<=G.jokers.config.highlighted_limit)"));
    }
    joker_selection_size_case(cases,"joker_selection_size_passive_multiple_effects","joker",json!([{
        "id":"selection_limit","trigger":"passive","effects":[
            {"effect_type":"edit_joker_size","params":{"operation":"add","value":1}},
            {"effect_type":"edit_joker_size","params":{"operation":"add","value":2}}
        ]
    }]),"G.jokers=make_joker_selection_area(1)",
        "test_definition:add_to_deck(actor,false);assert(G.jokers.config.highlighted_limit==4);select_jokers();test_definition:remove_from_deck(actor,false)",
        "assert(G.jokers.config.highlighted_limit==1 and #G.jokers.highlighted==1)");
    joker_selection_size_case(cases,"joker_selection_size_passive_opposing_sets","joker",json!([{
        "id":"selection_limit","trigger":"passive","effects":[
            {"effect_type":"edit_joker_size","params":{"operation":"set","value":3}},
            {"effect_type":"edit_joker_size","params":{"operation":"set","value":1}}
        ]
    }]),"G.jokers=make_joker_selection_area(1)",
        "test_definition:add_to_deck(actor,false);assert(G.jokers.config.highlighted_limit==1);test_definition:remove_from_deck(actor,false)",
        "assert(G.jokers.config.highlighted_limit==1)");

    let copy_rules = json!([{
        "id":"selection_limit","trigger":"passive","effects":[
            {"effect_type":"edit_joker_size","params":{"operation":"add","value":2}}
        ]
    }]);
    for (name, removal) in [
        ("source_first", "test_definition:remove_from_deck(actor,false);assert(G.jokers.config.highlighted_limit==3);test_definition:remove_from_deck(clone,false)"),
        ("copy_first", "test_definition:remove_from_deck(clone,false);assert(G.jokers.config.highlighted_limit==3);test_definition:remove_from_deck(actor,false)"),
    ] {
        joker_selection_size_case(cases,&format!("joker_selection_size_passive_copy_{name}"),"joker",copy_rules.clone(),
            "G.jokers=make_joker_selection_area(1);actor.config={center={key='j_mod_runtime_test',set='Joker'},card={}}",
            &format!("test_definition:add_to_deck(actor,false);assert(G.jokers.config.highlighted_limit==3);clone=copy_card(actor,clone_card_shell(200,actor.sort_id));assert(clone.ID~=actor.ID and clone.sort_id==actor.sort_id);test_definition:remove_from_deck(clone,false);assert(G.jokers.config.highlighted_limit==3);test_definition:add_to_deck(clone,false);assert(G.jokers.config.highlighted_limit==5);{removal}"),
            "assert(G.jokers.config.highlighted_limit==1)");
    }
    joker_selection_size_case(cases,"joker_selection_size_passive_saved_id_reload","joker",copy_rules,
        "G.jokers=make_joker_selection_area(1)",
        "test_definition:add_to_deck(actor,false);assert(G.jokers.config.highlighted_limit==3);restored={ID=200,sort_id=300,unique_val__saved_ID=actor.ID,ability=copy_table(actor.ability)};test_definition:add_to_deck(restored,false);assert(G.jokers.config.highlighted_limit==3);test_definition:remove_from_deck(restored,false)",
        "assert(G.jokers.config.highlighted_limit==1)");

    joker_selection_size_case(cases,"joker_selection_consumable_two_keys","consumable",json!([{
        "id":"selected_jokers","trigger":"card_used",
        "condition_groups":[{"conditions":[
            {"condition_type":"joker_selected","params":{"check_key":"key","joker_key":"j_first"}},
            {"condition_type":"joker_selected","params":{"check_key":"key","joker_key":"j_second"}}
        ]}],
        "effects":[{"effect_type":"modify_internal_variable","params":{"variable_name":"amount","operation":"increment","value":1}}]
    }]),"G.jokers=make_joker_selection_area(2);G.jokers:add_to_highlighted(owned_jokers[1],true);G.jokers:add_to_highlighted(owned_jokers[2],true)",
        "assert(test_definition:can_use(actor));test_definition:use(actor,nil,nil);assert(actor.ability.extra.amount==4);G.jokers:remove_from_highlighted(owned_jokers[1]);assert(not test_definition:can_use(actor));test_definition:use(actor,nil,nil)",
        "assert(actor.ability.extra.amount==4)");
}

fn append_flag_and_variable_cases(cases: &mut Vec<Value>) {
    let flag = "1 active-flag!";
    let mut writer = joker(json!([{
        "id":"set", "trigger":"hand_played", "effects":[{
            "effect_type":"emit_flag", "params":{"flag_name":flag,"change":"true","display_message":"n"}
        }]
    }, {
        "id":"following_effect", "trigger":"hand_played", "effects":[{"effect_type":"add_mult","params":{"value":10}}]
    }]));
    writer.appearance = Some(serde_json::from_value(json!({"appear_flags":[flag,"not blocked"]})).unwrap());
    let writer_code = Emitter::new().emit_chunk(&compile_joker(&writer, "mod"));
    let reader = joker(json!([{
        "id":"read", "trigger":"hand_played", "condition_groups":[{"conditions":[{
            "condition_type":"check_flag", "params":{"flag_name":flag}
        }]}], "effects":[{"effect_type":"add_mult","params":{"value":7}}]
    }]));
    let reader_code = Emitter::new().emit_chunk(&compile_joker(&reader, "mod"));
    cases.push(json!({"kind":"rule_options", "name":"flag_set_check_across_jokers_and_rounds",
        "code":format!("{writer_code}\nwriter=test_definition\n{reader_code}\nreader=test_definition"),
        "prepare":"writer_card={ability=writer.config};reader_card={ability=reader.config};G.GAME.pool_flags=nil",
        "invoke":"assert(not writer:in_pool({}));assert(not reader:calculate(reader_card,{joker_main=true}));local result=writer:calculate(writer_card,{joker_main=true});assert(result.mult==10);assert(writer:in_pool({}));assert(reader:calculate(reader_card,{joker_main=true}).mult==7)",
        "verify":"assert(#event_queue==0 and #status_messages==0);G.GAME.current_round={};assert(reader:calculate(reader_card,{joker_main=true}).mult==7);G.GAME.pool_flags.mod_blocked=true;assert(not writer:in_pool({}));G.GAME.pool_flags.mod_1_active_flag_=false;assert(not reader:calculate(reader_card,{joker_main=true}))"}));

    let mut unset_reader = reader.clone();
    unset_reader.appearance = Some(serde_json::from_value(json!({"appear_flags":["not blocked"]})).unwrap());
    cases.push(json!({"kind":"rule_options", "name":"flag_unset_check_without_run",
        "setup":"G=nil",
        "code":Emitter::new().emit_chunk(&compile_joker(&unset_reader,"mod")),
        "invoke":"assert(not test_definition:calculate(actor,{joker_main=true}));assert(test_definition:in_pool({}))",
        "verify":"assert(#status_messages==0)"}));

    for (change, initial, expected) in [("false",true,false),("invert",true,false),("invert",false,true)] {
        rule_option_case(cases,&format!("flag_change_{change}_{initial}"),"joker","emit_flag",
            json!({"flag_name":"enabled","change":change}),
            &format!("G.GAME.pool_flags={{mod_enabled={initial}}}"),
            &format!("assert(G.GAME.pool_flags.mod_enabled=={expected});assert(#event_queue==0 and #status_messages==0)"));
    }

    let input:Value=serde_json::from_str(include_str!("../tests/lua-code-examples/consumables/user_variables_use.json")).unwrap();
    let mut consumable:ConsumableDef=serde_json::from_value(input["definition"].clone()).unwrap();
    consumable.rules.push(serde_json::from_value(json!({
        "id":"after_mutation","trigger":"card_used","condition_groups":[{"conditions":[{
            "condition_type":"internal_variable","params":{"variable_name":"amount","operator":"greater_than","value":5}
        }]}],"effects":[{"effect_type":"set_dollars","params":{"operation":"add","value":{"value":"amount","valueType":"user_var"}}}]
    })).unwrap());
    cases.push(json!({"kind":"rule_options","name":"consumable_variables_use_mutation_following_rule_and_tooltip",
        "code":Emitter::new().emit_chunk(&compile_consumable(&consumable,"mod")),
        "prepare":"G.GAME.dollars=0;test_definition:set_ability(actor,true);assert(test_definition:can_use(actor))",
        "invoke":"test_definition:use(actor,nil,nil)",
        "verify":"assert(actor.ability.extra.amount==6);assert(G.GAME.dollars==10);assert(test_definition:loc_vars({},actor).vars[1]==6);actor.ability.extra.label='Spent';assert(test_definition:can_use(actor));assert(test_definition:loc_vars({},actor).vars[3]=='Spent');actor.ability.extra.amount=0;assert(not test_definition:can_use(actor));G=nil;assert(test_definition:loc_vars({},nil).vars[1]==4)"}));

    let global_consumable:ConsumableDef=serde_json::from_value(json!({
        "key":"global_use","name":"Global Use","description":["#1#"],
        "set":"Tarot","atlas":"Consumables","pos":{"x":0,"y":0},
        "user_variables":[
            {"name":"shared_amount","var_type":"number","initial_value":3,"is_global":true},
            {"name":"unrelated","var_type":"number","initial_value":99,"is_global":true}
        ],
        "rules":[{"id":"change","trigger":"card_used","effects":[
            {"effect_type":"modify_internal_variable","params":{"variable_name":"shared_amount","value":2,"operation":"increment"}}
        ]},{"id":"use","trigger":"card_used","effects":[
            {"effect_type":"set_dollars","params":{"operation":"add","value":{"value":"shared_amount","valueType":"user_var"}}}
        ]}]
    })).unwrap();
    cases.push(json!({"kind":"rule_options","name":"consumable_global_use_and_legacy_tooltip",
        "code":Emitter::new().emit_chunk(&compile_consumable(&global_consumable,"mod")),
        "prepare":"G.GAME.dollars=0;G.GAME.jf_global_vars={shared_amount=12,unrelated=99}",
        "invoke":"assert(test_definition:can_use(actor));test_definition:use(actor,nil,nil)",
        "verify":"assert(G.GAME.jf_global_vars.shared_amount==14);assert(G.GAME.dollars==14);local tooltip=test_definition:loc_vars({},actor);assert(tooltip.vars[1]==14);assert(#tooltip.vars==2);G=nil;assert(test_definition:loc_vars({},nil).vars[1]==3)"}));

    let grouped_input:Value=serde_json::from_str(include_str!("../tests/lua-code-examples/consumables/grouped_variables_use.json")).unwrap();
    let grouped:ConsumableDef=serde_json::from_value(grouped_input["definition"].clone()).unwrap();
    cases.push(json!({"kind":"rule_options","name":"consumable_variables_chance_and_loop",
        "code":Emitter::new().emit_chunk(&compile_consumable(&grouped,"mod")),
        "prepare":"G.GAME.dollars=0;SMODS.pseudorandom_probability=function() return true end",
        "invoke":"test_definition:use(actor,nil,nil)",
        "verify":"assert(actor.ability.extra.amount==11);assert(G.GAME.dollars==11);assert(test_definition:loc_vars({},actor).vars[1]==11)"}));
}

fn global_variables(persistent: bool) -> Vec<UserVariableDef> {
    serde_json::from_value(json!([
        {"name":"global_counter","var_type":"number","initial_value":7,"is_global":true,"is_persistent":persistent},
        {"name":"global_text","var_type":"text","initial_value":"Ready","is_global":true,"is_persistent":persistent},
        {"name":"global_key","var_type":"key","initial_value":"j_joker","is_global":true,"is_persistent":persistent},
        {"name":"global_suit","var_type":"suit","initial_value":"Hearts","is_global":true,"is_persistent":persistent},
        {"name":"global_rank","var_type":"rank","initial_value":"A","is_global":true,"is_persistent":persistent},
        {"name":"global_hand","var_type":"poker_hand","initial_value":"Pair","is_global":true,"is_persistent":persistent}
    ]))
    .unwrap()
}

fn global_main(run_variables: &[UserVariableDef]) -> String {
    export::build_main_lua(
        &[], &[], &[], &[], &[], &[], &[], "mod", &[],
        false, false, false, false, false, true, false, run_variables,
    )
}

fn global_setup(persistent_variables: &[UserVariableDef]) -> String {
    format!(
        "package.preload.nativefs=function() return {{}} end\n\
SMODS.current_mod={{}}\n\
G.SETTINGS={{profile=1}}\n\
G.PROFILES={{[1]={{jf_global_vars={{persistent_counter=41}}}}}}\n\
G.save_progress=function() saves=(saves or 0)+1 end\n\
SMODS.load_file=function(path) assert(path=='globals.lua',path);return function()\n{}end end\n",
        export::build_globals_lua(persistent_variables)
    )
}

fn append_global_lifecycle_case(cases: &mut Vec<Value>) {
    let persistent_variables: Vec<UserVariableDef> = serde_json::from_value(json!([
        {"name":"persistent_counter","var_type":"number","initial_value":3,"is_global":true,"is_persistent":true},
        {"name":"persistent_text","var_type":"text","initial_value":"Initial","is_global":true,"is_persistent":true}
    ])).unwrap();
    let main = global_main(&global_variables(false));
    cases.push(json!({
        "kind":"rule_options", "name":"global_lifecycle_round_save_new_run", "object":"global",
        "setup":global_setup(&persistent_variables),
        "code":format!("local function load_mod()\n{main}\nend\nload_mod()\ntest_definition={{config={{extra={{}}}}}}"),
        "invoke":r#"
local reset=SMODS.current_mod.reset_game_globals
reset(true)
local values=G.GAME.jf_global_vars
assert(values.global_counter==7 and values.global_rank=='A' and values.global_hand=='Pair')
values.global_counter=23;values.global_text='Changed';values.global_key='j_mod_saved'
values.global_suit='Clubs';values.global_rank='King';values.global_hand='Flush'
reset(false);reset(nil)
assert(values.global_counter==23 and values.global_text=='Changed' and values.global_key=='j_mod_saved')
assert(values.global_suit=='Clubs' and values.global_rank=='King' and values.global_hand=='Flush')
G.GAME.current_round={}
reset(false)
assert(values.global_counter==23 and values.global_rank=='King')
local saved_values={}
for key,value in pairs(values) do saved_values[key]=value end
G.GAME={jf_global_vars=saved_values,current_round={}}
load_mod()
reset=SMODS.current_mod.reset_game_globals
reset(false)
assert(G.GAME.jf_global_vars.global_counter==23 and G.GAME.jf_global_vars.global_hand=='Flush')
G.GAME.jf_global_vars.global_text=nil
reset(false)
assert(G.GAME.jf_global_vars.global_text=='Ready' and G.GAME.jf_global_vars.global_counter==23)
assert(JF_GLOBALS.persistent_counter==41 and JF_GLOBALS.persistent_text=='Initial')
JF_GLOBALS.persistent_counter=67
JF_GLOBALS.persistent_text='Saved'
local saved_progress_calls=saves
G.GAME={current_round={}}
reset(true)
assert(G.GAME.jf_global_vars.global_counter==7 and G.GAME.jf_global_vars.global_hand=='Pair')
assert(JF_GLOBALS.persistent_counter==67 and JF_GLOBALS.persistent_text=='Saved')
load_mod()
assert(JF_GLOBALS.persistent_counter==67 and JF_GLOBALS.persistent_text=='Saved')
assert(saves==saved_progress_calls)
G.GAME.jf_global_vars.global_counter=100
SMODS.current_mod.reset_game_globals(true)
assert(G.GAME.jf_global_vars.global_counter==7)
"#,
        "verify":"assert(G.PROFILES[1].jf_global_vars.persistent_counter==67 and G.PROFILES[1].jf_global_vars.persistent_text=='Saved');assert(saves>=3)",
    }));
}

fn append_typed_global_cases(cases: &mut Vec<Value>) {
    for persistent in [false, true] {
        let definitions = global_variables(persistent);
        let persistent_variables = if persistent { definitions.clone() } else { Vec::new() };
        let run_variables = if persistent { Vec::new() } else { definitions.clone() };
        for (effect, condition, variable, parameter, selection, expected) in [
            ("change_rank_variable", "rank_variable", "global_rank", "specific_rank", "K", "King"),
            ("change_suit_variable", "suit_variable", "global_suit", "specific_suit", "Clubs", "Clubs"),
            ("change_pokerhand_variable", "poker_hand_variable", "global_hand", "specific_pokerhand", "Flush", "Flush"),
        ] {
            let mut params = json!({"variable_name":variable,"change_type":"specific"});
            params[parameter] = json!(selection);
            let joker: JokerDef = serde_json::from_value(json!({
                "key":"global_test", "name":"Global Test", "description":["#1#"],
                "cost":4,"rarity":"common","blueprint_compat":false,"eternal_compat":true,
                "perishable_compat":true,"unlocked":true,"discovered":true,
                "atlas":"CustomJokers","pos":{"x":0,"y":0},
                "user_variables":definitions,
                "description_variables":[{"kind":"user","name":variable}],
                "rules":[{"id":"typed_global_change","trigger":"hand_played","effects":[{"effect_type":effect,"params":params}]}]
            })).unwrap();
            let mut check_params = json!({"variable_name":variable});
            check_params[parameter] = json!(expected);
            let check: ConditionDef = serde_json::from_value(json!({
                "condition_type":condition,"params":check_params
            })).unwrap();
            let mut ctx = CompileContext::new(ObjectType::Joker,"mod".into(),"reader".into(),false);
            ctx.set_user_vars(definitions.clone());
            let condition_code = compile_condition(&check, ObjectType::Joker, &mut ctx).unwrap();
            let main = global_main(&run_variables);
            let code = format!("{main}\n{}", Emitter::new().emit_chunk(&compile_joker(&joker,"mod")));
            let path = if persistent { "JF_GLOBALS" } else { "G.GAME.jf_global_vars" };
            let reset = "if SMODS.current_mod.reset_game_globals then SMODS.current_mod.reset_game_globals(false) end";
            let initial = match variable { "global_rank" => "A", "global_suit" => "Hearts", _ => "Pair" };
            cases.push(json!({
                "kind":"rule_options", "name":format!("typed_global_{variable}_persistent_{persistent}"),
                "object":"joker", "setup":global_setup(&persistent_variables), "code":code,
                "prepare":format!("if SMODS.current_mod.reset_game_globals then SMODS.current_mod.reset_game_globals(true) end;G.GAME.hands.Flush={{visible=true,played=0}};G.GAME.current_round={{global_rank_card={{rank='2',id=2}},global_suit_card={{suit='Spades'}},global_hand_hand='High Card'}};if test_definition.set_ability then test_definition:set_ability(actor,true) end;assert({path}.{variable}=='{initial}')"),
                "invoke":"test_definition:calculate(actor,{joker_main=true})",
                "verify":format!("assert({path}.{variable}=='{expected}');assert({condition_code});local tooltip=test_definition:loc_vars({{}},actor);assert(tooltip.vars[1]=='{expected}',tostring(tooltip.vars[1]));{reset};G.GAME.current_round={{}};{reset};assert({path}.{variable}=='{expected}');assert({condition_code})"),
            }));
        }
    }
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
    append_scoring_group_cases(&mut cases);
    append_game_variable_description_and_loop_cases(&mut cases);
    append_retrigger_scoring_cases(&mut cases);
    append_card_retrigger_cases(&mut cases);
    append_card_self_destruct_cases(&mut cases);
    append_deck_settings_cases(&mut cases);
    append_joker_creation_cases(&mut cases);
    append_rule_option_cases(&mut cases);
    append_joker_selection_and_key_cases(&mut cases);
    append_joker_selection_size_cases(&mut cases);
    append_flag_and_variable_cases(&mut cases);
    append_global_lifecycle_case(&mut cases);
    append_typed_global_cases(&mut cases);
    std::fs::write(output, serde_json::to_vec(&cases).unwrap()).unwrap();
}
