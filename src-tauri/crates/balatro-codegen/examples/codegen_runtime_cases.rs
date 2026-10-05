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
use balatro_codegen::{compile_consumable, compile_deck, compile_edition, compile_enhancement, compile_joker, compile_rarity, compile_seal, compile_voucher, Emitter};
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

fn append_edition_shader_cases(cases: &mut Vec<Value>) {
    for (name, shader) in [
        ("missing", None), ("null", Some(json!(null))), ("boolean_false", Some(json!(false))),
        ("empty", Some(json!(""))), ("whitespace", Some(json!(" \t\n "))),
        ("string_false", Some(json!("false"))),
    ] {
        let mut input = json!({"objectKey":"runtime_test","name":"Runtime Test","description":"Test",
            "apply_to_float":true,"rules":[{"id":"score","trigger":"card_scored","effects":[{
                "id":"mult","type":"add_mult","params":{"value":7}
            }]}]});
        if let Some(shader) = shader { input["shader"] = shader; }
        let input: export::EditionDataInput = serde_json::from_value(input).unwrap();
        cases.push(json!({"kind":"edition_shader","name":format!("edition_shader_frontend_{name}"),
            "code":Emitter::new().emit_chunk(&compile_edition(&export::edition_data_to_def(&input),"mod")),
            "shader":false,"custom":false}));
    }
    for (name, shader, expected, custom) in [
        ("missing", None, json!(false), false),
        ("empty", Some(""), json!(false), false),
        ("whitespace", Some(" \t\n "), json!(false), false),
        ("string_false", Some("false"), json!(false), false),
        ("vanilla", Some("polychrome"), json!("polychrome"), false),
        ("custom", Some("shimmer"), json!("mod_shimmer"), true),
    ] {
        let mut input = json!({"key":"runtime_test","name":"Runtime Test","description":["Test"],
            "apply_to_float":true,"rules":[{"id":"score","trigger":"card_scored","effects":[{
                "id":"mult","effect_type":"add_mult","params":{"value":7}
            }]}]});
        if let Some(shader) = shader { input["shader"] = json!(shader); }
        let definition: EditionDef = serde_json::from_value(input).unwrap();
        cases.push(json!({"kind":"edition_shader","name":format!("edition_shader_direct_{name}"),
            "code":Emitter::new().emit_chunk(&compile_edition(&definition,"mod")),
            "shader":expected,"custom":custom}));
    }
}

fn append_blind_win_cases(cases: &mut Vec<Value>) {
    let emit = |trigger: &str, scope: &str, consumable: bool| {
        let rules = json!([{"id":"win_rule","trigger":trigger,"effects":[{
            "id":"win_effect","effect_type":"win_game","params":{"win_type":scope}
        }]}]);
        let chunk = if consumable {
            let definition: ConsumableDef = serde_json::from_value(json!({
                "key":"runtime_test","name":"Runtime Test","description":["Test"],
                "set":"Tarot","atlas":"CustomConsumables","pos":{"x":0,"y":0},"rules":rules
            })).unwrap();
            compile_consumable(&definition,"mod")
        } else {
            compile_joker(&joker(rules),"mod")
        };
        Emitter::new().emit_chunk(&chunk)
    };
    let score_code = emit("hand_played", "blind", false);
    let invoke = "test_definition:calculate(actor,{joker_main=true})";
    for (name, prepare, after) in [
        ("selecting_hand", "", ""),
        ("duplicate_requests", "", "for index=1,4 do test_definition:calculate(actor,{joker_main=true}) end"),
        ("preserve_higher_score", "G.GAME.chips=700", ""),
        ("scoring_release_with_hands", "G.STATE=G.STATES.HAND_PLAYED;G.STATE_COMPLETE=true;G.GAME.current_round.hands_played=1",
            "G.E_MANAGER:add_event(Event{trigger='after',delay=0.15,func=function() G.GAME.chips=20;G.STATE_COMPLETE=false;return true end});tick_blind_events(1);assert(round_end_count==0 and G.GAME.chips==0,'win must wait for scoring completion')"),
        ("scoring_release_last_hand", "G.STATE=G.STATES.HAND_PLAYED;G.STATE_COMPLETE=true;G.GAME.current_round.hands_played=1;G.GAME.current_round.hands_left=0",
            "G.E_MANAGER:add_event(Event{trigger='after',delay=0.15,func=function() G.GAME.chips=20;G.STATE_COMPLETE=false;return true end});tick_blind_events(1);assert(round_end_count==0 and G.GAME.chips==0,'win must wait for the last hand to finish')"),
        ("scoring_already_released", "G.STATE=G.STATES.HAND_PLAYED;G.STATE_COMPLETE=false;G.GAME.current_round.hands_played=1", ""),
        ("naturally_winning_hand", "G.STATE=G.STATES.HAND_PLAYED;G.STATE_COMPLETE=false;G.GAME.current_round.hands_played=1;G.GAME.chips=700", ""),
    ] {
        let extra_check = if name == "preserve_higher_score" || name == "naturally_winning_hand" {
            "assert(G.GAME.chips==700,'winning must preserve an already higher score')"
        } else { "" };
        cases.push(json!({"kind":"blind_win","name":format!("blind_win_{name}"),"code":score_code,
            "prepare":prepare,"invoke":format!("{invoke};{after}"),
            "verify":format!("assert_blind_won();{extra_check}")}));
    }
    for (name, trigger, prepare, invocation) in [
        ("blind_selected", "blind_selected", "G.STATE=G.STATES.BLIND_SELECT;G.STATE_COMPLETE=true",
            "test_definition:calculate(actor,{setting_blind=true,main_eval=true});G.E_MANAGER:add_event(Event{trigger='after',delay=0.1,func=function() G.STATE=G.STATES.DRAW_TO_HAND;G.STATE_COMPLETE=false;return true end})"),
        ("first_hand_drawn", "first_hand_drawn", "G.STATE=G.STATES.DRAW_TO_HAND;G.STATE_COMPLETE=false;G.jokers.cards={actor};function actor:calculate_joker(context) return test_definition:calculate(self,context) end;function actor:calculate_rental() end;function actor:calculate_perishable() end",
            "tick_blind_events(3);assert(drawn_hand_count==1,'native initial draw did not finish')"),
        ("discard_restore", "hand_discarded", "G.GAME.current_round.discards_used=1",
            "test_definition:calculate(actor,{pre_discard=true});G.STATE=G.STATES.DRAW_TO_HAND;G.STATE_COMPLETE=true;G.E_MANAGER:add_event(Event{trigger='after',delay=0.1,func=function() G.STATE_COMPLETE=false;return true end})"),
        ("after_hand_score_overwrite", "after_hand_played", "G.STATE=G.STATES.HAND_PLAYED;G.STATE_COMPLETE=true;G.GAME.current_round.hands_played=1",
            "test_definition:calculate(actor,{after=true});G.E_MANAGER:add_event(Event{trigger='after',delay=0.1,func=function() G.GAME.chips=10;G.STATE_COMPLETE=false;return true end})"),
    ] {
        cases.push(json!({"kind":"blind_win","name":format!("blind_win_{name}"),"code":emit(trigger,"blind",false),
            "prepare":prepare,"invoke":invocation,"verify":"assert_blind_won()"}));
    }
    cases.push(json!({"kind":"blind_win","name":"blind_win_consumable_restore","code":emit("card_used","blind",true),
        "prepare":"G.STATE=G.STATES.PLAY_TAROT;G.STATE_COMPLETE=true",
        "invoke":"test_definition:use(actor,nil,nil);G.E_MANAGER:add_event(Event{trigger='after',delay=0.1,func=function() G.STATE=G.STATES.SELECTING_HAND;G.STATE_COMPLETE=true;return true end});tick_blind_events(1);assert(round_end_count==0 and G.GAME.chips==0,'win must wait for consumable cleanup')",
        "verify":"assert_blind_won()"}));
    for (name, prepare) in [
        ("shop", "G.STATE=G.STATES.SHOP"),
        ("outside_run", "G.STAGE=2"),
        ("missing_game", "G.GAME=nil"),
        ("finished_blind", "G.GAME.blind.in_blind=false"),
    ] {
        cases.push(json!({"kind":"blind_win","name":format!("blind_win_ignored_{name}"),"code":score_code,
            "prepare":prepare,"invoke":invoke,
            "verify":"tick_blind_events(10);assert(round_end_count==0 and run_win_count==0);for _,queue in pairs(G.E_MANAGER.queues) do assert(#queue==0,'invalid win request must not leave an event') end;assert(not G.GAME or G.GAME.chips==0)"}));
    }
    for (name, mutation) in [
        ("new_run", "G.GAME=copy_table(G.GAME)"),
        ("reused_blind_new_round", "G.GAME.round=2"),
        ("new_blind_config", "G.GAME.blind.config.blind={}"),
        ("naturally_finished", "G.STATE=G.STATES.ROUND_EVAL;G.GAME.blind.in_blind=false"),
    ] {
        cases.push(json!({"kind":"blind_win","name":format!("blind_win_cancelled_{name}"),"code":score_code,
            "prepare":"G.STATE=G.STATES.PLAY_TAROT;G.STATE_COMPLETE=true",
            "invoke":format!("{invoke};{mutation};if G.STATE~=G.STATES.ROUND_EVAL then G.STATE=G.STATES.SELECTING_HAND end"),
            "verify":"tick_blind_events(10);assert(round_end_count==0 and G.GAME.chips==0,'stale request must not win a different round');for _,queue in pairs(G.E_MANAGER.queues) do assert(#queue==0,'cancelled request must not leave an event') end"}));
    }
    cases.push(json!({"kind":"blind_win","name":"blind_win_current_run_unchanged","code":emit("hand_played","run",false),
        "prepare":"G.STATE=G.STATES.SHOP","invoke":invoke,
        "verify":"tick_blind_events(10);assert(run_win_count==1 and G.GAME.won==true and round_end_count==0 and G.GAME.chips==0)"}));
}

fn append_rarity_shop_cases(cases: &mut Vec<Value>) {
    for (name, rarities, joker_rarity, prepare, verify) in [
        ("positive_weight", vec![("superrare", 0.05)], "superrare", "",
            "assert(SMODS.poll_rarity('Joker','shop')=='mod_superrare','custom rarity must be eligible for shop polling');local pool=get_current_pool('Joker',nil,false,'shop');assert(#pool==1 and pool[1]=='j_mod_runtime_test')"),
        ("already_prefixed_joker", vec![("superrare", 0.05)], "mod_superrare", "",
            "assert(SMODS.Centers.j_mod_runtime_test.rarity=='mod_superrare');local pool=get_current_pool('Joker',nil,false,'shop');assert(pool[1]=='j_mod_runtime_test')"),
        ("zero_weight", vec![("superrare", 0.0)], "superrare", "",
            "for i=0,99 do rarity_roll=i/100;assert(SMODS.poll_rarity('Joker','shop')~='mod_superrare') end;local pool=get_current_pool('Joker','mod_superrare',false,'forced');assert(pool[1]=='j_mod_runtime_test')"),
        ("vanilla_rates", vec![("superrare", 0.05)], "superrare", "",
            "rarity_roll=0;assert(SMODS.poll_rarity('Joker','shop')==1);rarity_roll=0.7;assert(SMODS.poll_rarity('Joker','shop')==2);rarity_roll=0.92;assert(SMODS.poll_rarity('Joker','shop')==3);local rates=SMODS.ObjectTypes.Joker.rarities;assert(rates[1].weight==0.7 and rates[2].weight==0.25 and rates[3].weight==0.05)"),
        ("run_weight_modifier", vec![("superrare", 0.05)], "superrare", "G.GAME.mod_superrare_mod=0",
            "assert(SMODS.poll_rarity('Joker','shop')==3);G.GAME.mod_superrare_mod=20;rarity_roll=0.6;assert(SMODS.poll_rarity('Joker','shop')=='mod_superrare')"),
        ("native_get_weight", vec![("superrare", 0.05)], "superrare", "SMODS.Rarities.mod_superrare.get_weight=function(self,weight,pool) assert(weight==0.05 and pool.key=='Joker');return 0 end",
            "assert(SMODS.poll_rarity('Joker','shop')==3);SMODS.Rarities.mod_superrare.get_weight=function() return 1 end;rarity_roll=0.6;assert(SMODS.poll_rarity('Joker','shop')=='mod_superrare')"),
        ("multiple_custom_rarities", vec![("superrare", 0.05), ("mythic", 0.15)], "superrare", "",
            "local total=0;for _,rarity in ipairs(SMODS.ObjectTypes.Joker.rarities) do total=total+rarity.weight end;assert(math.abs(total-1.2)<0.000001);local previous=0;local seen={};for _,rarity in ipairs(SMODS.ObjectTypes.Joker.rarities) do rarity_roll=(previous+rarity.weight/2)/total;local selected=SMODS.poll_rarity('Joker','shop');seen[selected]=true;previous=previous+rarity.weight end;assert(seen.mod_superrare and seen.mod_mythic);local pool=get_current_pool('Joker','mod_mythic',false,'shop');assert(pool[1]=='j_mod_mythic_test')"),
    ] {
        let mut code = String::new();
        for (key, weight) in &rarities {
            let input: export::RarityDataInput = serde_json::from_value(json!({
                "key":key,"name":key,"badge_colour":"AABBCC","default_weight":weight
            })).unwrap();
            code.push_str(&Emitter::new().emit_chunk(&compile_rarity(&export::rarity_data_to_def(&input), "mod")));
            code.push('\n');
        }
        for (key, rarity) in [("runtime_test", joker_rarity), ("mythic_test", "mythic")] {
            if key == "mythic_test" && rarities.len() == 1 { continue; }
            let input: export::JokerDataInput = serde_json::from_value(json!({
                "objectKey":key,"name":key,"description":"Test","cost":4,"rarity":rarity,
                "unlocked":true,"discovered":true,"rules":[]
            })).unwrap();
            let definition = export::joker_data_to_def(&input, "mod", export::AtlasPosInput { x: 0, y: 0 }, None);
            code.push_str(&Emitter::new().emit_chunk(&compile_joker(&definition, "mod")));
            code.push('\n');
        }
        cases.push(json!({"kind":"rarity_shop","name":format!("rarity_shop_{name}"),"code":code,
            "invoke":format!("inject_rarity_shop();{prepare}"),"verify":verify}));
    }
}

fn append_description_blank_line_cases(cases: &mut Vec<Value>) {
    for (name, description, translated, expected, translated_expected) in [
        ("markers", "First #1#[s][s][s]Last #2#", "Premier #1#[s][s][s]Dernier #2#",
            vec!["First #1#", " ", " ", "Last #2#"], vec!["Premier #1#", " ", " ", "Dernier #2#"]),
        ("newlines", "First #1#\n\n\nLast #2#", "Premier #1#\n\n\nDernier #2#",
            vec!["First #1#", " ", " ", "Last #2#"], vec!["Premier #1#", " ", " ", "Dernier #2#"]),
        ("windows_newlines", "First #1#\r\n\r\n\r\nLast #2#", "Premier #1#\r\n\r\n\r\nDernier #2#",
            vec!["First #1#", " ", " ", "Last #2#"], vec!["Premier #1#", " ", " ", "Dernier #2#"]),
        ("html_breaks", "First #1#<br/><br><br />Last #2#", "Premier #1#<br/><br><br />Dernier #2#",
            vec!["First #1#", " ", " ", "Last #2#"], vec!["Premier #1#", " ", " ", "Dernier #2#"]),
        ("edge_breaks", "[s]First #1#[s][s]Last #2#[s]", "[s]Premier #1#[s][s]Dernier #2#[s]",
            vec![" ", "First #1#", " ", "Last #2#", " "], vec![" ", "Premier #1#", " ", "Dernier #2#", " "]),
        ("whole_empty", "[s][s]", "\n\n", vec!["No description"], vec!["No description"]),
    ] {
        let entry: export::BatchJokerEntry = serde_json::from_value(json!({
            "jokerData":{
                "objectKey":"runtime_test","name":"Runtime Test","description":description,
                "cost":4,"rarity":"common",
                "localizations":[{"language":"fr","name":"Essai","description":translated}],
                "descriptionVariables":[
                    {"kind":"game","id":"current_money","multiplier":2,"startsFrom":3},
                    {"kind":"config","name":"mult0","effect_id":"money_mult","fallback":{
                        "value":"GAMEVAR:current_money|2|3","valueType":"gameVariable"
                    }}
                ],
                "rules":[{"id":"score","trigger":"hand_played","effects":[{
                    "id":"money_mult","type":"add_mult","params":{"value":{
                        "value":"GAMEVAR:current_money|2|3","valueType":"gameVariable"
                    }}
                }]}]
            },
            "pos":{"x":0,"y":0},"fileName":"runtime_test.lua"
        })).unwrap();
        let definition = export::joker_data_to_def(&entry.joker_data, "mod", entry.pos.clone(), None);
        let inline = Emitter::new().emit_chunk(&compile_joker(&definition, "mod"));
        let external = Emitter::new().emit_chunk(&balatro_codegen::compile_joker_with_options(&definition, "mod", false));
        let localizations = export::build_localization_lua_files(
            "mod", "en-us", &[entry], &[], &[], &[], &[], &[], &[], &[],
        );
        cases.push(json!({"kind":"description_layout","name":format!("description_blank_lines_{name}_inline"),
            "code":inline,"expected":expected}));
        for (locale, lines) in [("en-us", &expected), ("fr", &translated_expected)] {
            cases.push(json!({"kind":"description_layout","name":format!("description_blank_lines_{name}_{locale}"),
                "code":external,"localization":localizations[locale],"expected":lines}));
        }
    }
}

fn append_description_format_cases(cases: &mut Vec<Value>) {
    for (name, description, expected, parts) in [
        ("colour_markers", "{C:red}First #1#[s]Second #2#{}",
            vec!["{C:red}First #1#", "{C:red}Second #2#{}"],
            json!([[{"text":"First #1#","control":{"C":"red"}}], [{"text":"Second #2#","control":{"C":"red"}}]])),
        ("combined_effect_scale", "{C:blue,E:1,s:1.5}First #1#\nSecond #2#{}",
            vec!["{C:blue,E:1,s:1.5}First #1#", "{C:blue,E:1,s:1.5}Second #2#{}"],
            json!([[{"text":"First #1#","control":{"C":"blue","E":"1","s":"1.5"}}], [{"text":"Second #2#","control":{"C":"blue","E":"1","s":"1.5"}}]])),
        ("bump_effect", "{E:2}First #1#[s]Second #2#{}",
            vec!["{E:2}First #1#", "{E:2}Second #2#{}"],
            json!([[{"text":"First #1#","control":{"E":"2"}}], [{"text":"Second #2#","control":{"E":"2"}}]])),
        ("background", "{X:mult,C:white}First#1#[s]Second#2#{}",
            vec!["{X:mult,C:white}First#1#", "{X:mult,C:white}Second#2#{}"],
            json!([[{"text":"First#1#","control":{"X":"mult","C":"white"}}], [{"text":"Second#2#","control":{"X":"mult","C":"white"}}]])),
        ("dynamic_colours", "{V:1,B:2,s:0.75}First #1#[s]Second #2#{}",
            vec!["{V:1,B:2,s:0.75}First #1#", "{V:1,B:2,s:0.75}Second #2#{}"],
            json!([[{"text":"First #1#","control":{"V":"1","B":"2","s":"0.75"}}], [{"text":"Second #2#","control":{"V":"1","B":"2","s":"0.75"}}]])),
        ("reset_before_break", "{C:red}First #1#{}[s]Plain #2#",
            vec!["{C:red}First #1#{}", "Plain #2#"],
            json!([[{"text":"First #1#","control":{"C":"red"}}], [{"text":"Plain #2#","control":{}}]])),
        ("reset_midline", "{C:red}First #1#[s]Second{} Plain #2#[s]Last",
            vec!["{C:red}First #1#", "{C:red}Second{} Plain #2#", "Last"],
            json!([[{"text":"First #1#","control":{"C":"red"}}], [{"text":"Second","control":{"C":"red"}},{"text":" Plain #2#","control":{}}], [{"text":"Last","control":{}}]])),
        ("explicit_replacement", "{C:red,E:1}First #1#[s]{C:blue}Second #2#[s]Last{}",
            vec!["{C:red,E:1}First #1#", "{C:blue}Second #2#", "{C:blue}Last{}"],
            json!([[{"text":"First #1#","control":{"C":"red","E":"1"}}], [{"text":"Second #2#","control":{"C":"blue"}}], [{"text":"Last","control":{"C":"blue"}}]])),
        ("adjacent_tags", "{C:red}{E:1}First #1#[s]Second #2#{}",
            vec!["{C:red}{E:1}First #1#", "{E:1}Second #2#{}"],
            json!([[{"text":"First #1#","control":{"E":"1"}}], [{"text":"Second #2#","control":{"E":"1"}}]])),
        ("tag_only_rows", "{C:red}[s]First #1#[s]{}[s]Plain #2#",
            vec![" ", "{C:red}First #1#", " ", "Plain #2#"],
            json!([[{"text":" ","control":{}}], [{"text":"First #1#","control":{"C":"red"}}], [{"text":" ","control":{}}], [{"text":"Plain #2#","control":{}}]])),
        ("background_blank_gap", "{X:mult,C:white}First#1#[s][s]Second#2#{}",
            vec!["{X:mult,C:white}First#1#", " ", "{X:mult,C:white}Second#2#{}"],
            json!([[{"text":"First#1#","control":{"X":"mult","C":"white"}}], [{"text":" ","control":{}}], [{"text":"Second#2#","control":{"X":"mult","C":"white"}}]])),
        ("alternate_breaks", "{C:red}First #1#\r\nSecond #2#<BR />Last{}",
            vec!["{C:red}First #1#", "{C:red}Second #2#", "{C:red}Last{}"],
            json!([[{"text":"First #1#","control":{"C":"red"}}], [{"text":"Second #2#","control":{"C":"red"}}], [{"text":"Last","control":{"C":"red"}}]])),
    ] {
        let translated = description.replace("First", "Premier");
        let translated_expected: Vec<String> = expected.iter().map(|line| line.replace("First", "Premier")).collect();
        let mut translated_parts = parts.clone();
        for line in translated_parts.as_array_mut().unwrap() {
            for part in line.as_array_mut().unwrap() {
                part["text"] = json!(part["text"].as_str().unwrap().replace("First", "Premier"));
            }
        }
        let entry: export::BatchJokerEntry = serde_json::from_value(json!({
            "jokerData":{
                "objectKey":"runtime_test","name":"Runtime Test","description":description,
                "cost":4,"rarity":"common",
                "localizations":[{"language":"fr","name":"Essai","description":translated}],
                "descriptionVariables":[
                    {"kind":"game","id":"current_money","multiplier":2,"startsFrom":3},
                    {"kind":"config","name":"mult0","effect_id":"money_mult","fallback":{
                        "value":"GAMEVAR:current_money|2|3","valueType":"gameVariable"
                    }}
                ],
                "rules":[{"id":"score","trigger":"hand_played","effects":[{
                    "id":"money_mult","type":"add_mult","params":{"value":{
                        "value":"GAMEVAR:current_money|2|3","valueType":"gameVariable"
                    }}
                }]}]
            },
            "pos":{"x":0,"y":0},"fileName":"runtime_test.lua"
        })).unwrap();
        let definition = export::joker_data_to_def(&entry.joker_data, "mod", entry.pos.clone(), None);
        let inline = Emitter::new().emit_chunk(&compile_joker(&definition, "mod"));
        let external = Emitter::new().emit_chunk(&balatro_codegen::compile_joker_with_options(&definition, "mod", false));
        let localizations = export::build_localization_lua_files(
            "mod", "en-us", &[entry], &[], &[], &[], &[], &[], &[], &[],
        );
        cases.push(json!({"kind":"description_format","name":format!("description_format_{name}_inline"),
            "code":inline,"expected":expected,"parts":parts}));
        cases.push(json!({"kind":"description_format","name":format!("description_format_{name}_en-us"),
            "code":external,"localization":localizations["en-us"],"expected":expected,"parts":parts}));
        cases.push(json!({"kind":"description_format","name":format!("description_format_{name}_fr"),
            "code":external,"localization":localizations["fr"],"expected":translated_expected,"parts":translated_parts}));
    }
}

fn append_consumable_creation_message_cases(cases: &mut Vec<Value>) {
    for (name, object, source_set, create_set, key, count, loops, expected_key, expected_set) in [
        ("self_tarot", "consumable", "Tarot", "Tarot", "c_mod_runtime_test", 1, 1, "c_mod_runtime_test", "Tarot"),
        ("self_planet", "consumable", "Planet", "Planet", "c_mod_runtime_test", 1, 1, "c_mod_runtime_test", "Planet"),
        ("self_spectral", "consumable", "Spectral", "Spectral", "c_mod_runtime_test", 1, 1, "c_mod_runtime_test", "Spectral"),
        ("random_set", "consumable", "Tarot", "random", "random", 1, 1, "c_first", "Tarot"),
        ("custom_set_multiple", "consumable", "mod_Runes", "mod_Runes", "random", 2, 1, "c_mod_runtime_test", "mod_Runes"),
        ("self_in_loop", "consumable", "Tarot", "Tarot", "c_mod_runtime_test", 2, 2, "c_mod_runtime_test", "Tarot"),
        ("shared_joker_effect", "joker", "Tarot", "Tarot", "c_first", 1, 1, "c_first", "Tarot"),
        ("full_slots", "consumable", "Tarot", "Tarot", "c_mod_runtime_test", 1, 1, "c_mod_runtime_test", "Tarot"),
        ("chance_success", "consumable", "Tarot", "Tarot", "c_mod_runtime_test", 1, 1, "c_mod_runtime_test", "Tarot"),
        ("chance_miss", "consumable", "Tarot", "Tarot", "c_mod_runtime_test", 1, 1, "c_mod_runtime_test", "Tarot"),
    ] {
        let effect = json!({"id":"create","effect_type":"create_consumable","params":{
            "set":create_set,"specific_card":key,"count":count,"edition":"none","ignore_slots":"n"
        }});
        let mut rule = json!({"id":"creation","trigger":if object=="joker" {"hand_played"} else {"card_used"}});
        if name.starts_with("chance_") {
            rule["random_groups"] = json!([{"id":"chance","chance_numerator":1,"chance_denominator":2,"effects":[effect]}]);
        } else if loops > 1 {
            rule["loop_groups"] = json!([{"id":"repeat","count":loops,"effects":[effect]}]);
        } else {
            rule["effects"] = json!([effect]);
        }
        let rules = json!([rule]);
        let chunk = if object == "consumable" {
            let definition: ConsumableDef = serde_json::from_value(json!({
                "key":"runtime_test","name":"Runtime Test","description":["Test"],
                "set":source_set,"atlas":"CustomConsumables","pos":{"x":0,"y":0},"rules":rules
            })).unwrap();
            compile_consumable(&definition, "mod")
        } else {
            compile_joker(&joker(rules), "mod")
        };
        cases.push(json!({"kind":"rule_options","name":format!("consumable_creation_message_{name}"),
            "consumable_creation_message_runtime":true,"code":Emitter::new().emit_chunk(&chunk),
            "prepare":if object=="consumable" {format!("register_source_consumable('{source_set}');{}",match name {"full_slots"=>"G.consumeables.config.card_limit=1","chance_miss"=>"chance_roll=0.9",_=>""})} else {String::new()},
            "invoke":if object=="consumable" {"test_definition:use(actor,nil,nil)"} else {"local effect=test_definition:calculate(actor,{joker_main=true});if effect then SMODS.calculate_effect(effect,actor) end"},
            "verify":format!("assert_consumable_creation_message({},'{expected_key}','{expected_set}');{}",if name=="full_slots" || name=="chance_miss" {0} else {count*loops},if name.starts_with("chance_") {format!("assert(#SMODS.post_prob==1 and SMODS.post_prob[1].pseudorandom_result and SMODS.post_prob[1].result=={} and SMODS.post_prob[1].trigger_obj==actor,'silent creation must retain native probability result notification')",name=="chance_success")} else {String::new()})
        }));
    }
}

fn size_message_effect(effect_type: &str, operation: &str, mode: Option<&str>, message: Option<&str>) -> Value {
    let mut effect = json!({"id":effect_type,"type":effect_type,"params":{
        "operation":{"value":operation},"value":{"value":2}
    }});
    if let Some(mode) = mode { effect["messageMode"] = json!(mode); }
    if let Some(message) = message { effect["customMessage"] = json!(message); }
    effect
}

fn size_message_list(messages: &[&str]) -> String {
    // Expected bytes are independent of the compiler's Lua string escaping.
    format!("{{{}}}", messages.iter().map(|message|
        format!("string.char({})", message.bytes().map(|byte| byte.to_string()).collect::<Vec<_>>().join(","))
    ).collect::<Vec<_>>().join(","))
}

fn size_message_case(cases: &mut Vec<Value>, name: &str, object: &str, rules: Value, prepare: &str, verify: &str) {
    // Exercise the desktop export boundary, including top-level message fields.
    let chunk = if object == "consumable" {
        let input: export::ConsumableDataInput = serde_json::from_value(json!({
            "objectKey":"runtime_test","name":"Runtime Test","description":"Test",
            "set":"Tarot","rules":rules
        })).unwrap();
        compile_consumable(&export::consumable_data_to_def(&input, export::AtlasPosInput {x:0,y:0}, None), "mod")
    } else {
        let input: export::JokerDataInput = serde_json::from_value(json!({
            "objectKey":"runtime_test","name":"Runtime Test","description":"Test",
            "cost":4,"rarity":"common","rules":rules
        })).unwrap();
        compile_joker(&export::joker_data_to_def(&input, "mod", export::AtlasPosInput {x:0,y:0}, None), "mod")
    };
    let invoke = if object == "consumable" {
        "test_definition:use(actor,nil,nil);run_events()"
    } else {
        "resolve_joker({joker_main=true});run_events()"
    };
    cases.push(json!({"kind":"scoring","name":format!("size_message_{name}"),
        "size_message_runtime":true,"code":Emitter::new().emit_chunk(&chunk),
        "prepare":prepare,"invoke":invoke,"verify":verify}));
}

fn append_size_message_cases(cases: &mut Vec<Value>) {
    for (effect_type, label, stat, initial) in [
        ("edit_hand_size", "Hand Limit", "hand", 8),
        ("edit_play_size", "Play Size", "play", 5),
        ("edit_discard_size", "Discard Size", "discard", 5),
    ] {
        for operation in ["add", "subtract", "set"] {
            let expected = match operation { "subtract" => initial - 2, "set" => 2, _ => initial + 2 };
            let default_message = match operation {
                "subtract" => format!("-2 {label}"),
                "set" => format!("{label}  set to 2"),
                _ => format!("+2 {label}"),
            };
            for mode in ["default", "custom", "none"] {
                let message = format!("Changed {stat}");
                let effect = size_message_effect(effect_type, operation, Some(mode), Some(&message));
                let messages = match mode { "none" => size_message_list(&[]), "custom" => size_message_list(&[&message]), _ => size_message_list(&[&default_message]) };
                size_message_case(cases, &format!("{stat}_{operation}_{mode}"), "joker", json!([{
                    "id":"change_size","trigger":"hand_played","effects":[effect]
                }]), "", &format!("assert_size_change('{stat}',{expected},1,{});assert(hand_chips==0 and mult==1)", messages));
            }
        }
    }

    for (name, mode, message, expected_message) in [
        ("old_default", None, None, "+2 Hand Limit"),
        ("old_custom", None, Some("Extra room!"), "Extra room!"),
        ("empty_custom", Some("custom"), Some(""), "+2 Hand Limit"),
        ("whitespace_custom", Some("custom"), Some(" \t\n "), "+2 Hand Limit"),
        ("escaped_custom", Some("custom"), Some("Room \"for\" 'more'\\cards\nnext\tline"), "Room \"for\" 'more'\\cards\nnext\tline"),
    ] {
        size_message_case(cases,name,"joker",json!([{
            "id":"change_size","trigger":"hand_played","effects":[size_message_effect("edit_hand_size","add",mode,message)]
        }]),"",&format!("assert_size_change('hand',10,1,{})",size_message_list(&[expected_message])));
    }

    let mixed = json!([{
        "id":"mixed","trigger":"hand_played",
        "effects":[
            size_message_effect("edit_hand_size","add",Some("none"),Some("Hidden stale text")),
            {"id":"chips","type":"add_chips","params":{"value":{"value":7}}}
        ],
        "loops":[{"id":"repeat","repetitions":{"value":3},"effects":[
            size_message_effect("edit_play_size","add",Some("none"),None),
            size_message_effect("edit_discard_size","subtract",Some("custom"),Some("Less to discard"))
        ]}],
        "randomGroups":[{"id":"chance","chance_numerator":{"value":1},"chance_denominator":{"value":2},"effects":[
            size_message_effect("edit_hand_size","add",Some("custom"),Some("Lucky room!")),
            {"id":"money","type":"set_dollars","params":{"value":{"value":3},"operation":{"value":"add"}}}
        ]}]
    }]);
    for succeeds in [true,false] {
        let messages = if succeeds {size_message_list(&["Lucky room!","Less to discard","Less to discard","Less to discard"])} else {size_message_list(&["Less to discard","Less to discard","Less to discard"])};
        size_message_case(cases,&format!("mixed_loop_chance_{succeeds}"),"joker",mixed.clone(),
            &format!("SMODS.pseudorandom_probability=function() return {succeeds} end;G.GAME.starting_params.discard_limit=12"),
            &format!("assert(G.hand.config.card_limit=={} and size_change_calls.hand=={});assert(G.GAME.starting_params.play_limit==11 and size_change_calls.play==3);assert(G.GAME.starting_params.discard_limit==6 and size_change_calls.discard==3);assert_size_messages({messages});assert(hand_chips==7 and mult==1 and G.GAME.dollars=={});assert(message_count('chips')==1 and message_count('dollars')=={})",if succeeds {12}else{10},if succeeds {2}else{1},if succeeds {3}else{0},if succeeds {1}else{0}));
    }
    size_message_case(cases,"consumable_mixed_modes","consumable",json!([{
        "id":"use","trigger":"card_used","effects":[
            size_message_effect("edit_hand_size","add",Some("none"),Some("Hidden")),
            size_message_effect("edit_play_size","set",Some("custom"),Some("Play more!")),
            size_message_effect("edit_discard_size","subtract",Some("default"),Some("Ignored stale text"))
        ]
    }]),"","assert(G.hand.config.card_limit==10 and size_change_calls.hand==1);assert(G.GAME.starting_params.play_limit==2 and size_change_calls.play==1);assert(G.GAME.starting_params.discard_limit==3 and size_change_calls.discard==1);assert_size_messages({'Play more!','-2 Discard Size'})");
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

fn probability_counter(name: &str) -> Value {
    json!({"effect_type":"modify_internal_variable","params":{
        "variable_name":name,"operation":"increment","value":1
    }})
}

fn probability_definition(rules: Value) -> JokerDef {
    let mut definition=joker(rules);
    definition.user_variables=serde_json::from_value(json!([
        {"name":"seen","var_type":"number","initial_value":0},
        {"name":"chance_count","var_type":"number","initial_value":0},
        {"name":"loop_count","var_type":"number","initial_value":0}
    ])).unwrap();
    definition
}

fn probability_result_case(
    cases: &mut Vec<Value>,name: &str,definitions: &[JokerDef],prepare: &str,invoke: &str,verify: &str,
) {
    let mut code=String::from("probability_definitions={}\n");
    for definition in definitions {
        code.push_str(&Emitter::new().emit_chunk(&compile_joker(definition,"mod")));
        code.push_str("\nprobability_definitions[#probability_definitions+1]=test_definition\n");
    }
    code.push_str("test_definition=probability_definitions[1]\n");
    cases.push(json!({"kind":"rule_options","name":format!("probability_result_{name}"),
        "probability_result_runtime":true,"code":code,
        "prepare":format!("actor=probability_card(test_definition);G.jokers.cards={{actor}};{prepare}"),
        "invoke":invoke,"verify":verify}));
}

fn append_probability_result_cases(cases: &mut Vec<Value>) {
    let chance=json!({"id":"chance","chance_numerator":1,"chance_denominator":2,
        "effects":[probability_counter("chance_count"),{"effect_type":"set_dollars","params":{"value":3}}]
    });
    let simple=probability_definition(json!([{"id":"result","trigger":"probability_result",
        "effects":[probability_counter("seen")],"random_groups":[chance.clone()]
    }]));
    probability_result_case(cases,"chance_recursion",std::slice::from_ref(&simple),
        "set_probability_rolls({0,0})","assert(external_probability())",
        "assert(G.GAME.dollars==3);assert(actor.ability.extra.seen==1 and actor.ability.extra.chance_count==1);assert_probability_dispatch_complete(1,2)");

    for external_success in [true,false] {
        for chance_success in [true,false] {
            let definition=probability_definition(json!([{"id":"result","trigger":"probability_result",
                "effects":[probability_counter("seen")],"random_groups":[chance.clone()],
                "loop_groups":[{"id":"loop","count":2,"effects":[
                    probability_counter("loop_count"),{"effect_type":"set_dollars","params":{"value":1}}
                ]}]
            }]));
            probability_result_case(cases,&format!("outer_{external_success}_chance_{chance_success}"),&[definition],
                &format!("set_probability_rolls({{{},{}}})",if external_success {0.0}else{0.9},if chance_success {0.0}else{0.9}),
                &format!("assert(external_probability()=={external_success})"),
                &format!("assert(G.GAME.dollars=={});assert(actor.ability.extra.seen==1);assert(actor.ability.extra.chance_count=={} and actor.ability.extra.loop_count==2);assert(probability_counts.modifier==2 and probability_counts.fixed==2);assert_probability_dispatch_complete(1,2)",if chance_success {5}else{2},if chance_success {1}else{0}));
        }
    }

    let observer=probability_definition(json!([{"id":"observer","trigger":"probability_result",
        "effects":[probability_counter("seen"),{"effect_type":"set_dollars","params":{"value":2}}]
    }]));
    probability_result_case(cases,"ordinary_listener_preserved",&[simple.clone(),observer],
        "observer_card=probability_card(probability_definitions[2]);G.jokers.cards={actor,observer_card};set_probability_rolls({0,0})",
        "assert(external_probability())",
        "assert(G.GAME.dollars==5);assert(actor.ability.extra.seen==1 and observer_card.ability.extra.seen==1);assert_probability_dispatch_complete(1,2)");

    let mut second_listener=simple.clone();second_listener.key="second_listener".into();
    probability_result_case(cases,"multiple_listener_definitions",&[simple.clone(),second_listener],
        "second_card=probability_card(probability_definitions[2]);G.jokers.cards={actor,second_card};set_probability_rolls({0,0,0})",
        "assert(external_probability())",
        "assert(G.GAME.dollars==6);assert(actor.ability.extra.seen==1 and second_card.ability.extra.seen==1);assert(actor.ability.extra.chance_count==1 and second_card.ability.extra.chance_count==1);assert_probability_dispatch_complete(1,3)");

    probability_result_case(cases,"multiple_copies_and_native_blueprint",std::slice::from_ref(&simple),
        "second_card=probability_card(test_definition);blueprint_card=probability_blueprint(actor);G.jokers.cards={actor,second_card,blueprint_card};set_probability_rolls({0,0,0,0})",
        "assert(external_probability())",
        "assert(G.GAME.dollars==9);assert(actor.ability.extra.seen==2 and second_card.ability.extra.seen==1);assert(actor.ability.extra.chance_count==2 and second_card.ability.extra.chance_count==1);assert_probability_dispatch_complete(1,4)");

    let mut incompatible=simple.clone();incompatible.blueprint_compat=false;
    probability_result_case(cases,"blueprint_disabled",&[incompatible],
        "G.jokers.cards={actor,probability_blueprint(actor)};set_probability_rolls({0,0})",
        "assert(external_probability())",
        "assert(G.GAME.dollars==3);assert(actor.ability.extra.seen==1 and actor.ability.extra.chance_count==1);assert_probability_dispatch_complete(1,2)");

    for first_success in [true,false] {
        let definition=probability_definition(json!([{"id":"result","trigger":"probability_result",
            "effects":[probability_counter("seen")],"random_groups":[chance.clone(),{
                "id":"second_chance","chance_numerator":1,"chance_denominator":2,
                "effects":[probability_counter("chance_count"),{"effect_type":"set_dollars","params":{"value":5}}]
            }]
        }]));
        probability_result_case(cases,&format!("multiple_groups_first_{first_success}"),&[definition],
            &format!("set_probability_rolls({{0,{},0}})",if first_success {0.0}else{0.9}),
            "assert(external_probability())",
            &format!("assert(G.GAME.dollars=={});assert(actor.ability.extra.seen==1 and actor.ability.extra.chance_count=={});assert_probability_dispatch_complete(1,3)",if first_success {8}else{5},if first_success {2}else{1}));
    }

    for normal_success in [true,false] {
        let definition=probability_definition(json!([
            {"id":"normal","trigger":"hand_played","random_groups":[{
                "id":"normal_chance","chance_numerator":1,"chance_denominator":2,
                "effects":[{"effect_type":"set_dollars","params":{"value":7}}]
            }]},
            {"id":"result","trigger":"probability_result","effects":[probability_counter("seen")],"random_groups":[chance.clone()]}
        ]));
        probability_result_case(cases,&format!("normal_trigger_still_notifies_{normal_success}"),&[definition],
            &format!("set_probability_rolls({{{},0}})",if normal_success {0.0}else{0.9}),
            "SMODS.calculate_context({joker_main=true})",
            &format!("assert(G.GAME.dollars=={});assert(actor.ability.extra.seen==1 and actor.ability.extra.chance_count==1);assert_probability_dispatch_complete(1,2)",if normal_success {10}else{3}));
    }

    probability_result_case(cases,"queued_original_results_preserved",std::slice::from_ref(&simple),
        "set_probability_rolls({0,0.9,0,0})",
        "assert(SMODS.pseudorandom_probability(actor,'first',1,2,'first',true));assert(not SMODS.pseudorandom_probability(actor,'second',1,2,'second',true));assert(#SMODS.post_prob==2);SMODS.trigger_effects({},actor)",
        "assert(G.GAME.dollars==6);assert(actor.ability.extra.seen==2 and actor.ability.extra.chance_count==2);assert(probability_contexts[1].identifier=='first' and probability_contexts[2].identifier=='second');assert(probability_contexts[1].result==true and probability_contexts[2].result==false);assert_probability_dispatch_complete(2,4)");

    for source in ["joker_modifier","normal_multiplier"] {
        let mut rules=json!([{"id":"result","trigger":"probability_result","effects":[probability_counter("seen")],
            "random_groups":[{"id":"modified_chance","chance_numerator":1,"chance_denominator":4,
                "effects":[probability_counter("chance_count"),{"effect_type":"set_dollars","params":{"value":3}}]
            }]
        }]);
        if source=="joker_modifier" {
            rules.as_array_mut().unwrap().push(json!({"id":"modify","trigger":"change_probability",
                "effects":[{"effect_type":"mod_probability","params":{"part":"numerator","operation":"multiply","value":2}}]
            }));
        }
        probability_result_case(cases,&format!("respects_{source}"),&[probability_definition(rules)],
            if source=="normal_multiplier" {"G.GAME.probabilities.normal=2;set_probability_rolls({0.75,0.4})"}else{"set_probability_rolls({0.75,0.4})"},
            "assert(external_probability())",
            "assert(G.GAME.dollars==3);assert(actor.ability.extra.seen==1 and actor.ability.extra.chance_count==1);assert(probability_contexts[1].numerator==2);assert(probability_counts.modifier==2 and probability_counts.fixed==2);assert_probability_dispatch_complete(1,2)");
    }

    probability_result_case(cases,"respects_other_joker_modifier",&[
        probability_definition(json!([{"id":"result","trigger":"probability_result",
            "random_groups":[{"id":"modified_chance","chance_numerator":1,"chance_denominator":4,
                "effects":[probability_counter("chance_count"),{"effect_type":"set_dollars","params":{"value":3}}]
            }]
        }])),
        probability_definition(json!([{"id":"modifier","trigger":"change_probability","effects":[{
            "effect_type":"mod_probability","params":{"part":"numerator","operation":"multiply","value":2}
        }]}]))
    ],"modifier_card=probability_card(probability_definitions[2]);G.jokers.cards={actor,modifier_card};set_probability_rolls({0.75,0.4})",
        "assert(external_probability())",
        "assert(G.GAME.dollars==3 and actor.ability.extra.chance_count==1);assert(probability_contexts[1].numerator==2);assert(probability_counts.modifier==2 and probability_counts.fixed==2);assert_probability_dispatch_complete(1,2)");

    for status in ["succeeded","failed"] {
        for outer_success in [true,false] {
            let matches=(status=="succeeded")==outer_success;
            let definition=probability_definition(json!([{"id":"conditioned","trigger":"probability_result",
                "condition_groups":[{"conditions":[{"condition_type":"probability_succeeded","params":{"status":status}}]}],
                "effects":[probability_counter("seen")],"random_groups":[chance.clone()]
            }]));
            probability_result_case(cases,&format!("condition_{status}_{outer_success}"),&[definition],
                &format!("set_probability_rolls({{{},0}})",if outer_success {0.0}else{0.9}),
                &format!("assert(external_probability()=={outer_success})"),
                &format!("assert(G.GAME.dollars=={});assert(actor.ability.extra.seen=={} and actor.ability.extra.chance_count=={});assert_probability_dispatch_complete(1,{})",if matches {3}else{0},if matches {1}else{0},if matches {1}else{0},if matches {2}else{1}));
        }
        let definition=probability_definition(json!([{"id":"conditioned","trigger":"probability_result",
            "condition_groups":[{"conditions":[{"condition_type":"probability_succeeded","params":{"status":status}}]}],
            "effects":[probability_counter("seen")]
        }]));
        probability_result_case(cases,&format!("missing_result_is_neither_{status}"),&[definition],
            "set_probability_rolls({})","SMODS.calculate_context({pseudorandom_result=true})",
            "assert(actor.ability.extra.seen==0);assert_probability_dispatch_complete(1,0)");
    }

    probability_result_case(cases,"flat_condition_fields",&[probability_definition(json!([{
        "id":"conditioned","trigger":"probability_result","condition_groups":[{"conditions":[
            {"condition_type":"probability_succeeded","params":{"status":"succeeded"}},
            {"condition_type":"probability_identifier","params":{"mode":"custom","card_key":"external"}},
            {"condition_type":"probability_part_compare","params":{"part":"numerator","operator":"equals","value":1}},
            {"condition_type":"probability_part_compare","params":{"part":"denominator","operator":"equals","value":4}}
        ]}],"effects":[probability_counter("seen")],"random_groups":[chance.clone()]
    }]))],"set_probability_rolls({0,0})","assert(external_probability(1,4))",
        "assert(G.GAME.dollars==3);assert(actor.ability.extra.seen==1 and actor.ability.extra.chance_count==1);assert_probability_dispatch_complete(1,2)");

    probability_result_case(cases,"modifier_flat_compare",&[probability_definition(json!([
        {"id":"modify","trigger":"change_probability","condition_groups":[{"conditions":[{
            "condition_type":"probability_part_compare","params":{"part":"numerator","operator":"equals","value":1}
        }]}],"effects":[{"effect_type":"mod_probability","params":{"part":"numerator","operation":"multiply","value":2}}]},
        {"id":"result","trigger":"probability_result","condition_groups":[{"conditions":[{
            "condition_type":"probability_part_compare","params":{"part":"numerator","operator":"equals","value":2}
        }]}],"effects":[probability_counter("seen")],"random_groups":[chance]}
    ]))],"set_probability_rolls({0.75,0.75})","assert(external_probability())",
        "assert(G.GAME.dollars==3);assert(actor.ability.extra.seen==1 and actor.ability.extra.chance_count==1);assert_probability_dispatch_complete(1,2)");
}

fn probability_group_condition(group_id: &str, status: &str) -> Value {
    json!({"condition_type":"probability_succeeded","params":{
        "source":"chance_group","group_id":group_id,"status":status
    }})
}

fn probability_failure_chain() -> JokerDef {
    probability_definition(json!([
        {"id":"initial","trigger":"hand_drawn","random_groups":[{
            "id":"first","chance_numerator":1,"chance_denominator":4,
            "effects":[probability_counter("chance_count"),{"effect_type":"set_dollars","params":{"value":5}}]
        }]},
        {"id":"retry","trigger":"probability_result","condition_groups":[{
            "conditions":[probability_group_condition("first","failed")]
        }],"random_groups":[{
            "id":"second","chance_numerator":1,"chance_denominator":5,
            "effects":[probability_counter("loop_count"),{"effect_type":"set_dollars","params":{"value":10}}]
        }]},
        {"id":"fallback","trigger":"probability_result","condition_groups":[{
            "conditions":[probability_group_condition("second","failed")]
        }],"effects":[probability_counter("seen"),{"effect_type":"set_dollars","params":{"value":1000}}]}
    ]))
}

fn append_probability_chain_cases(cases: &mut Vec<Value>) {
    for first_success in [true,false] {
        for second_success in [true,false] {
            let result_count=if first_success {1}else{2};
            let fallback_count=usize::from(!first_success && !second_success);
            let second_count=usize::from(!first_success && second_success);
            let dollars=if first_success {5}else if second_success {10}else{1000};
            probability_result_case(cases,&format!("chain_first_{first_success}_second_{second_success}"),
                &[probability_failure_chain()],
                &format!("set_probability_rolls({{{},{}}})",if first_success {0.0}else{0.9},if second_success {0.0}else{0.9}),
                "SMODS.calculate_context({hand_drawn=true})",
                &format!("assert(G.GAME.dollars=={dollars});assert(actor.ability.extra.seen=={fallback_count} and actor.ability.extra.chance_count=={} and actor.ability.extra.loop_count=={second_count});assert(probability_contexts[1].jf_probability_group_id=='first' and probability_contexts[1].jf_probability_owner==actor);if {result_count}==2 then assert(probability_contexts[2].jf_probability_group_id=='second' and probability_contexts[2].jf_probability_owner==actor and probability_contexts[2].denominator==5) end;assert_probability_dispatch_complete({result_count},{result_count})",usize::from(first_success)));
        }
    }

    probability_result_case(cases,"chain_success_branch",&[probability_definition(json!([
        {"id":"initial","trigger":"hand_drawn","random_groups":[{
            "id":"first","chance_numerator":1,"chance_denominator":2,"effects":[]
        }]},
        {"id":"retry","trigger":"probability_result","condition_groups":[{
            "conditions":[probability_group_condition("first","succeeded")]
        }],"random_groups":[{
            "id":"second","chance_numerator":1,"chance_denominator":5,"effects":[]
        }]},
        {"id":"reward","trigger":"probability_result","condition_groups":[{
            "conditions":[probability_group_condition("second","succeeded")]
        }],"effects":[probability_counter("seen"),{"effect_type":"set_dollars","params":{"value":1000}}]}
    ]))],"set_probability_rolls({0,0})","SMODS.calculate_context({hand_drawn=true})",
        "assert(G.GAME.dollars==1000 and actor.ability.extra.seen==1);assert_probability_dispatch_complete(2,2)");

    probability_result_case(cases,"chain_independent_original_events",&[probability_failure_chain()],
        "set_probability_rolls({0.9,0.9,0.9,0.9})",
        "SMODS.calculate_context({hand_drawn=true});SMODS.calculate_context({hand_drawn=true})",
        "assert(G.GAME.dollars==2000 and actor.ability.extra.seen==2);assert(probability_contexts[1].jf_probability_chain~=probability_contexts[3].jf_probability_chain);assert_probability_dispatch_complete(4,4)");

    probability_result_case(cases,"chain_cycle_stops_before_repeated_roll",&[probability_definition(json!([
        {"id":"initial","trigger":"hand_drawn","random_groups":[{
            "id":"initial_chance","chance_numerator":1,"chance_denominator":2,"effects":[]
        }]},
        {"id":"stage_a","trigger":"probability_result","condition_groups":[
            {"logic_operator":"or","conditions":[probability_group_condition("initial_chance","succeeded")]},
            {"conditions":[probability_group_condition("stage_b_chance","succeeded")]}
        ],"effects":[probability_counter("seen")],"random_groups":[{
            "id":"stage_a_chance","chance_numerator":1,"chance_denominator":2,"effects":[]
        }]},
        {"id":"stage_b","trigger":"probability_result","condition_groups":[{
            "conditions":[probability_group_condition("stage_a_chance","succeeded")]
        }],"random_groups":[{
            "id":"stage_b_chance","chance_numerator":1,"chance_denominator":2,"effects":[]
        }]}
    ]))],"set_probability_rolls({0,0,0})","SMODS.calculate_context({hand_drawn=true})",
        "assert(actor.ability.extra.seen==2);assert(probability_contexts[1].jf_probability_group_id=='initial_chance' and probability_contexts[2].jf_probability_group_id=='stage_a_chance' and probability_contexts[3].jf_probability_group_id=='stage_b_chance');assert_probability_dispatch_complete(3,3)");

    for source_status in ["succeeded","failed"] {
        probability_result_case(cases,&format!("chain_empty_source_{source_status}"),&[probability_definition(json!([
            {"id":"initial","trigger":"hand_drawn","random_groups":[{
                "id":"empty","chance_numerator":1,"chance_denominator":4,"effects":[]
            }]},
            {"id":"result","trigger":"probability_result","condition_groups":[{
                "conditions":[probability_group_condition("empty",source_status)]
            }],"effects":[probability_counter("seen")]}
        ]))],&format!("set_probability_rolls({{{}}})",if source_status=="succeeded" {0.0}else{0.9}),
            "SMODS.calculate_context({hand_drawn=true})",
            "assert(actor.ability.extra.seen==1);assert(probability_contexts[1].jf_probability_group_id=='empty');assert_probability_dispatch_complete(1,1)");
    }

    for source in ["joker_modifier","normal_multiplier"] {
        let mut rules=json!([
            {"id":"initial","trigger":"hand_drawn","random_groups":[{
                "id":"modified","chance_numerator":1,"chance_denominator":4,"effects":[]
            }]},
            {"id":"result","trigger":"probability_result","condition_groups":[{"conditions":[
                probability_group_condition("modified","succeeded"),
                {"condition_type":"probability_part_compare","params":{"part":"numerator","operator":"equals","value":2}},
                {"condition_type":"probability_part_compare","params":{"part":"denominator","operator":"equals","value":4}}
            ]}],"effects":[probability_counter("seen")]}
        ]);
        if source=="joker_modifier" {
            rules.as_array_mut().unwrap().push(json!({"id":"modify","trigger":"change_probability",
                "effects":[{"effect_type":"mod_probability","params":{"part":"numerator","operation":"multiply","value":2}}]
            }));
        }
        probability_result_case(cases,&format!("chain_modified_odds_{source}"),&[probability_definition(rules)],
            if source=="normal_multiplier" {"G.GAME.probabilities.normal=2;set_probability_rolls({0.4})"}else{"set_probability_rolls({0.4})"},
            "SMODS.calculate_context({hand_drawn=true})",
            "assert(actor.ability.extra.seen==1);assert(probability_contexts[1].numerator==2 and probability_contexts[1].denominator==4);assert(probability_counts.modifier==1 and probability_counts.fixed==1);assert_probability_dispatch_complete(1,1)");
    }

    for source in ["user_variable","game_variable"] {
        let denominator=if source=="user_variable" {json!({"value":"Draws","valueType":"userVariable"})}
            else {json!({"value":"GAMEVAR:joker_count|1|0","valueType":"gameVariable"})};
        let mut definition=probability_definition(json!([
            {"id":"initial","trigger":"hand_drawn","random_groups":[{
                "id":"dynamic","chance_numerator":1,"chance_denominator":denominator,"effects":[]
            }]},
            {"id":"result","trigger":"probability_result","condition_groups":[{
                "conditions":[probability_group_condition("dynamic","failed")]
            }],"effects":[probability_counter("seen")]}
        ]));
        if source=="user_variable" {
            definition.user_variables.push(serde_json::from_value(json!({"name":"Draws","var_type":"number","initial_value":4})).unwrap());
        }
        let invoke=if source=="user_variable" {
            "actor.ability.extra.Draws=4;SMODS.calculate_context({hand_drawn=true});actor.ability.extra.Draws=8;SMODS.calculate_context({hand_drawn=true})"
        }else{
            "SMODS.calculate_context({hand_drawn=true});G.jokers.cards={actor,probability_blueprint(actor)};test_definition.blueprint_compat=false;SMODS.calculate_context({hand_drawn=true})"
        };
        let verify=if source=="user_variable" {
            "assert(actor.ability.extra.seen==2);assert(probability_contexts[1].denominator==4 and probability_contexts[2].denominator==8);assert_probability_dispatch_complete(2,2)"
        }else{
            "assert(actor.ability.extra.seen==1);assert(probability_contexts[1].denominator==1 and probability_contexts[2].denominator==2);assert_probability_dispatch_complete(2,2)"
        };
        probability_result_case(cases,&format!("chain_dynamic_denominator_{source}"),&[definition],
            "set_probability_rolls({0.9,0.9})",invoke,verify);
    }

    probability_result_case(cases,"chain_copies_and_native_blueprint_are_isolated",&[probability_failure_chain()],
        "second_card=probability_card(test_definition);blueprint_card=probability_blueprint(actor);G.jokers.cards={actor,second_card,blueprint_card};set_probability_rolls({0.9,0.9,0.9,0.9,0.9,0.9})",
        "SMODS.calculate_context({hand_drawn=true})",
        "assert(G.GAME.dollars==3000 and actor.ability.extra.seen==2 and second_card.ability.extra.seen==1);local owners={};for _,context in ipairs(probability_contexts) do local owner=context.jf_probability_owner;owners[owner]=(owners[owner] or 0)+1 end;assert(owners[actor]==2 and owners[second_card]==2 and owners[blueprint_card]==2);assert_probability_dispatch_complete(6,6)");

    let mut other=probability_failure_chain();other.key="another_definition".into();
    probability_result_case(cases,"chain_other_definition_same_group_id_is_isolated",&[probability_failure_chain(),other],
        "second_card=probability_card(probability_definitions[2]);G.jokers.cards={actor,second_card};set_probability_rolls({0.9,0.9,0.9,0.9})",
        "SMODS.calculate_context({hand_drawn=true})",
        "assert(G.GAME.dollars==2000 and actor.ability.extra.seen==1 and second_card.ability.extra.seen==1);assert_probability_dispatch_complete(4,4)");

    probability_result_case(cases,"chain_source_rejects_external_and_missing_results",&[probability_failure_chain()],
        "set_probability_rolls({0.9})",
        "assert(not external_probability());SMODS.calculate_context({pseudorandom_result=true,jf_probability_group_id='first',jf_probability_owner=actor})",
        "assert(G.GAME.dollars==0 and actor.ability.extra.seen==0 and actor.ability.extra.chance_count==0 and actor.ability.extra.loop_count==0);assert_probability_dispatch_complete(2,1)");

    let observer=probability_definition(json!([{"id":"observer","trigger":"probability_result",
        "condition_groups":[{"conditions":[{"condition_type":"probability_succeeded","params":{"source":"any","status":"failed"}}]}],
        "effects":[probability_counter("seen")]
    }]));
    probability_result_case(cases,"chain_any_source_observes_named_results",&[probability_failure_chain(),observer],
        "observer_card=probability_card(probability_definitions[2]);G.jokers.cards={actor,observer_card};set_probability_rolls({0.9,0.9})",
        "SMODS.calculate_context({hand_drawn=true})",
        "assert(G.GAME.dollars==1000 and actor.ability.extra.seen==1 and observer_card.ability.extra.seen==2);assert_probability_dispatch_complete(2,2)");

    let mut depth_rules=vec![json!({"id":"initial","trigger":"hand_drawn","random_groups":[{
        "id":"depth_0","chance_numerator":1,"chance_denominator":2,"effects":[]
    }]})];
    for index in 1..=18 {
        depth_rules.push(json!({"id":format!("depth_rule_{index}"),"trigger":"probability_result",
            "condition_groups":[{"conditions":[probability_group_condition(&format!("depth_{}",index-1),"succeeded")]}],
            "random_groups":[{"id":format!("depth_{index}"),"chance_numerator":1,"chance_denominator":2,"effects":[]}]
        }));
    }
    depth_rules.push(json!({"id":"unreachable","trigger":"probability_result",
        "condition_groups":[{"conditions":[probability_group_condition("depth_18","succeeded")]}],
        "effects":[probability_counter("seen")]
    }));
    probability_result_case(cases,"chain_depth_is_bounded",&[probability_definition(json!(depth_rules))],
        "set_probability_rolls({})","SMODS.calculate_context({hand_drawn=true})",
        "assert(actor.ability.extra.seen==0);assert(probability_roll_count>=16 and probability_roll_count<=17);for _,context in ipairs(probability_contexts) do assert(context.jf_probability_depth<=16) end;assert_probability_dispatch_complete(probability_roll_count,probability_roll_count)");

    let mut fanout_rules=vec![json!({"id":"initial","trigger":"hand_drawn","random_groups":[{
        "id":"branch_0_a","chance_numerator":1,"chance_denominator":2,"effects":[]
    }]})];
    for level in 1..=8 {
        fanout_rules.push(json!({"id":format!("branch_rule_{level}"),"trigger":"probability_result",
            "condition_groups":[
                {"logic_operator":"or","conditions":[probability_group_condition(&format!("branch_{}_a",level-1),"succeeded")]},
                {"conditions":[probability_group_condition(&format!("branch_{}_b",level-1),"succeeded")]}
            ],"random_groups":[
                {"id":format!("branch_{level}_a"),"chance_numerator":1,"chance_denominator":2,"effects":[]},
                {"id":format!("branch_{level}_b"),"chance_numerator":1,"chance_denominator":2,"effects":[]}
            ]}));
    }
    fanout_rules.push(json!({"id":"last_branches","trigger":"probability_result","condition_groups":[
        {"logic_operator":"or","conditions":[probability_group_condition("branch_8_a","succeeded")]},
        {"conditions":[probability_group_condition("branch_8_b","succeeded")]}
    ],"effects":[probability_counter("seen")]}));
    probability_result_case(cases,"chain_fanout_budget_is_bounded",&[probability_definition(json!(fanout_rules))],
        "set_probability_rolls({})","SMODS.calculate_context({hand_drawn=true})",
        "assert(probability_roll_count>=128 and probability_roll_count<=129);for _,context in ipairs(probability_contexts) do assert(context.jf_probability_chain==probability_contexts[1].jf_probability_chain and context.jf_probability_chain.remaining>=0) end;assert_probability_dispatch_complete(probability_roll_count,probability_roll_count)");
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

fn deck_card_rule(params: Value) -> Value {
    json!([{"id":"targets","trigger":"card_used","effects":[{
        "effect_type":"edit_all_starting_cards","params":params
    }]}])
}

fn deck_cards_case(
    cases: &mut Vec<Value>,
    name: &str,
    definition: DeckDef,
    prepare: &str,
    invoke: &str,
    verify: &str,
) {
    cases.push(json!({"kind":"deck_cards","name":format!("deck_cards_{name}"),
        "code":Emitter::new().emit_chunk(&compile_deck(&definition,"mod")),
        "prepare":prepare,"invoke":invoke,"verify":verify}));
}

fn append_deck_card_target_cases(cases: &mut Vec<Value>) {
    // Deck apply precedes creation of the starting cards. Both target filtering
    // and game-variable counts must execute in the queued startup callback.
    let startup="assert(G.playing_cards==nil);test_definition:apply(actor);assert(#event_queue>0);assert(G.playing_cards==nil);initialize_starting_cards();run_events()";
    for (name, selection, target_suit, target_rank, ids) in [
        ("legacy_all",None,"Hearts","King","{1,2,3,4,5,6,7,8}"),
        ("explicit_all",Some("all"),"Hearts","King","{1,2,3,4,5,6,7,8}"),
        ("unknown_selector_legacy_all",Some("old_value"),"Hearts","King","{1,2,3,4,5,6,7,8}"),
        ("matching_suit",Some("matching"),"Hearts","any","{1,2,5,8}"),
        ("matching_rank",Some("matching"),"any","King","{2,3,5}"),
        ("matching_rank_abbreviation",Some("matching"),"any","A","{4,8}"),
        ("matching_intersection",Some("matching"),"Hearts","K","{2,5}"),
        ("matching_any",Some("matching"),"any","any","{1,2,3,4,5,6,7,8}"),
        ("matching_empty",Some("matching"),"Spades","Ace","{}"),
    ] {
        let mut params=json!({"target_suit":target_suit,"target_rank":target_rank,
            "enhancement":"m_bonus","edition":"polychrome"});
        if let Some(selection)=selection {
            params["selection_method"]=json!(selection);
        }
        deck_cards_case(cases,name,deck(deck_card_rule(params)),"",startup,
            &format!("assert_starting_card_targets({ids},'m_bonus','e_polychrome')"));
    }

    deck_cards_case(cases,"typed_selectors_and_modifications",deck(deck_card_rule(json!({
        "selection_method":{"value":"matching","valueType":"specific"},
        "target_suit":{"value":"Hearts","valueType":"specific"},
        "target_rank":{"value":"K","valueType":"specific"},
        "enhancement":{"value":"m_mult","valueType":"specific"},
        "edition":{"value":"e_polychrome","valueType":"specific"}
    }))),"",startup,"assert_starting_card_targets({2,5},'m_mult','e_polychrome')");

    deck_cards_case(cases,"simultaneous_modifications",deck(deck_card_rule(json!({
        "selection_method":"matching","target_suit":"Hearts","target_rank":"King",
        "enhancement":"m_mult","edition":"polychrome","seal":"Gold","suit":"Spades","rank":"A"
    }))),"",startup,"assert_starting_card_targets({2,5},'m_mult','e_polychrome','Gold','Spades','Ace')");

    for edition in ["shiny","e_mod_shiny"] {
        deck_cards_case(cases,&format!("custom_edition_{edition}"),deck(deck_card_rule(json!({
            "selection_method":"matching","target_suit":"Hearts","target_rank":"A","edition":edition
        }))),"",startup,"assert_starting_card_targets({8},nil,'e_mod_shiny');assert(initial_deck_cards[8].edition.x_mult==2)");
    }

    // An edition hook can modify another card's suit while applying the first
    // edition. The eligible set must already have been captured at this point.
    deck_cards_case(cases,"matching_snapshot_before_edition_hooks",deck(deck_card_rule(json!({
        "selection_method":"matching","target_suit":"Hearts","target_rank":"King","edition":"polychrome"
    }))),
        "G.P_CENTERS.e_polychrome.on_apply=function(card) for _,id in ipairs({2,5}) do if initial_deck_cards[id]~=card then initial_deck_cards[id].base.suit='Clubs' end end end",
        startup,
        "for id,card in ipairs(initial_deck_cards) do assert((card.edition_calls or 0)==((id==2 or id==5) and 1 or 0));assert((card.edition and card.edition.key)==((id==2 or id==5) and 'e_polychrome' or nil)) end");

    for (name,count,expected) in [
        ("random_count",json!(2),2),
        ("random_zero",json!(0),0),
        ("random_negative",json!(-3),0),
        ("random_fractional",json!(2.9),2),
        ("random_oversized",json!(99),4),
        ("random_numeric_string",json!("3"),3),
        ("random_typed_number",json!({"value":2,"valueType":"number"}),2),
    ] {
        deck_cards_case(cases,name,deck(deck_card_rule(json!({
            "selection_method":"random","count":count,"target_suit":"Hearts","target_rank":"any",
            "enhancement":"m_bonus","edition":"polychrome"
        }))),"",startup,&format!("assert_random_starting_targets({expected},'Hearts')"));
    }

    for (name,target_suit,target_rank,count,expected) in [
        ("random_unfiltered","any","any",3,3),
        ("random_intersection","Hearts","K",1,1),
        ("random_intersection_oversized","Hearts","K",5,2),
        ("random_empty","Spades","Ace",2,0),
    ] {
        let suit=if target_suit=="any" {"nil"}else{"'Hearts'"};
        let suit=if target_suit=="Spades" {"'Spades'"}else{suit};
        let rank=if target_rank=="any" {"nil"}else if target_rank=="K" {"'King'"}else{"'Ace'"};
        deck_cards_case(cases,name,deck(deck_card_rule(json!({
            "selection_method":"random","count":count,"target_suit":target_suit,"target_rank":target_rank,
            "enhancement":"m_bonus","edition":"polychrome"
        }))),"",startup,&format!("assert_random_starting_targets({expected},{suit},{rank})"));
    }

    deck_cards_case(cases,"random_runtime_config_count",deck(deck_card_rule(json!({
        "selection_method":"random","count":1,"enhancement":"m_bonus","edition":"polychrome"
    }))),"test_definition.config.extra.edit_starting_cards_count0=3",startup,"assert_random_starting_targets(3)");

    let mut definition=deck(deck_card_rule(json!({
        "selection_method":"random","count":{"value":"bonus","valueType":"userVariable"},
        "enhancement":"m_bonus","edition":"polychrome"
    })));
    definition.user_variables=serde_json::from_value(json!([
        {"name":"bonus","var_type":"number","initial_value":2}
    ])).unwrap();
    deck_cards_case(cases,"random_user_variable_count",definition,
        "test_definition.config.extra.bonus=3",startup,"assert_random_starting_targets(3)");

    for (name,count) in [
        ("random_game_variable_count",json!({"value":"GAMEVAR:cards_in_deck|0.5|0","valueType":"gameVariable"})),
        ("random_legacy_game_variable_count",json!("GAMEVAR:cards_in_deck|0.5|0")),
    ] {
        deck_cards_case(cases,name,deck(deck_card_rule(json!({
            "selection_method":"random","count":count,"enhancement":"m_bonus","edition":"polychrome"
        }))),"",startup,"assert_random_starting_targets(4)");
    }

    deck_cards_case(cases,"random_count_read_after_apply",deck(deck_card_rule(json!({
        "selection_method":"random","count":1,"enhancement":"m_bonus","edition":"polychrome"
    }))),"",
        "test_definition:apply(actor);test_definition.config.extra.edit_starting_cards_count0=3;initialize_starting_cards();run_events()",
        "assert_random_starting_targets(3)");

    deck_cards_case(cases,"random_modification_values",deck(deck_card_rule(json!({
        "selection_method":"matching","target_suit":"Spades","target_rank":"King",
        "enhancement":"random","edition":"random","seal":"random","suit":"random","rank":"random"
    }))),"",startup,
        "assert_starting_card_targets({3},'m_bonus','e_holo','Gold','Clubs','10')");

    for (name,effect,params,ids) in [
        ("legacy_suit_block","edit_starting_suits",json!({"selected_suit":"Hearts","replace_suit":"Spades","enhancement":"m_bonus","edition":"polychrome"}),"{1,2,5,8}"),
        ("legacy_rank_block","edit_starting_ranks",json!({"specific_selected_Rank":"King","specific_replace_Rank":"Ace","enhancement":"m_bonus","edition":"polychrome"}),"{2,3,5}"),
    ] {
        let suit=if effect=="edit_starting_suits" {"'Spades'"}else{"nil"};
        let rank=if effect=="edit_starting_ranks" {"'Ace'"}else{"nil"};
        deck_cards_case(cases,name,deck(json!([{"id":"legacy","trigger":"card_used","effects":[{
            "effect_type":effect,"params":params
        }]}])),"",startup,&format!("assert_starting_card_targets({ids},'m_bonus','e_polychrome',nil,{suit},{rank})"));
    }

    deck_cards_case(cases,"multiple_rules",deck(json!([
        {"id":"hearts","trigger":"card_used","effects":[{"effect_type":"edit_all_starting_cards","params":{
            "selection_method":"matching","target_suit":"Hearts","enhancement":"m_bonus"
        }}]},
        {"id":"kings","trigger":"card_used","effects":[{"effect_type":"edit_all_starting_cards","params":{
            "selection_method":"matching","target_rank":"K","edition":"polychrome"
        }}]}
    ])),"",startup,
        "for _,card in ipairs(initial_deck_cards) do assert(card.config.center.key==(card.original_suit=='Hearts' and 'm_bonus' or 'c_base'));assert((card.edition and card.edition.key)==(card.original_rank=='King' and 'e_polychrome' or nil)) end;assert(#event_queue==0 and G.GAME.starting_deck_size==8)");

    deck_cards_case(cases,"loop_group",deck(json!([{
        "id":"looped","trigger":"card_used","loop_groups":[{"id":"repeat","count":2,"effects":[{
            "effect_type":"edit_all_starting_cards","params":{
                "selection_method":"matching","target_suit":"Spades","target_rank":"King",
                "enhancement":"m_bonus","edition":"polychrome"
            }
        }]}]
    }])),"",startup,
        "for id,card in ipairs(initial_deck_cards) do assert((card.enhancement_calls or 0)==(id==3 and 2 or 0));assert((card.edition_calls or 0)==(id==3 and 2 or 0)) end;assert(#event_queue==0 and G.GAME.starting_deck_size==8)");
}

fn append_playing_card_transform_cases(cases: &mut Vec<Value>) {
    for edition in [json!("polychrome"),json!("e_polychrome"),json!({"value":"polychrome","valueType":"specific"})] {
        let definition=joker(json!([{"id":"modify","trigger":"card_scored","effects":[{
            "effect_type":"edit_playing_card","params":{"new_edition":edition}
        }]}]));
        cases.push(json!({"kind":"rule_options","name":format!("playing_card_transform_polychrome_{edition}"),
            "playing_card_transform_runtime":true,"code":Emitter::new().emit_chunk(&compile_joker(&definition,"mod")),
            "invoke":"test_definition:calculate(actor,{individual=true,cardarea=G.play,other_card=observed_card});run_events()",
            "verify":"assert(observed_card.edition.key=='e_polychrome' and observed_card.edition.x_mult==1.5);assert(other_playing_card.edition==nil)"}));
    }
    for (name,trigger,invoke,verify) in [
        ("native_score_card","card_scored","local context={cardarea=G.play};SMODS.score_card(observed_card,context);assert(context.other_card==nil);run_events()","assert(observed_card.edition and observed_card.edition.key=='e_polychrome');assert(other_playing_card.edition==nil)"),
        ("native_shared_scoring_context","card_scored","local context={cardarea=G.play};SMODS.score_card(observed_card,context);SMODS.score_card(other_playing_card,context);assert(context.other_card==nil);run_events()","assert(observed_card.edition.key=='e_polychrome' and other_playing_card.edition.key=='e_polychrome')"),
        ("reused_context","card_scored","local context={individual=true,cardarea=G.play,other_card=observed_card};test_definition:calculate(actor,context);context.other_card=other_playing_card;run_events()","assert(observed_card.edition and observed_card.edition.key=='e_polychrome');assert(other_playing_card.edition==nil)"),
        ("cleared_context","card_scored","local context={individual=true,cardarea=G.play,other_card=observed_card};test_definition:calculate(actor,context);context.other_card=nil;run_events()","assert(observed_card.edition and observed_card.edition.key=='e_polychrome')"),
        ("missing_target","hand_played","local effect=test_definition:calculate(actor,{joker_main=true});if effect then SMODS.calculate_effect(effect,actor) end;run_events()","assert(observed_card.edition==nil and other_playing_card.edition==nil)"),
    ] {
        let definition=joker(json!([{"id":"modify","trigger":trigger,"effects":[{
            "effect_type":"edit_playing_card","params":{"new_edition":"polychrome"}
        }]}]));
        cases.push(json!({"kind":"rule_options","name":format!("playing_card_transform_{name}"),
            "playing_card_transform_runtime":true,"code":Emitter::new().emit_chunk(&compile_joker(&definition,"mod")),
            "invoke":invoke,"verify":verify}));
    }
    for flag in ["removed","destroyed","shattered","getting_sliced"] {
        let definition=joker(json!([{"id":"modify","trigger":"card_scored","effects":[{
            "effect_type":"edit_playing_card","params":{"new_edition":"polychrome"}
        }]}]));
        cases.push(json!({"kind":"rule_options","name":format!("playing_card_transform_stale_{flag}"),
            "playing_card_transform_runtime":true,"code":Emitter::new().emit_chunk(&compile_joker(&definition,"mod")),
            "invoke":format!("test_definition:calculate(actor,{{individual=true,cardarea=G.play,other_card=observed_card}});observed_card.{flag}=true;run_events()"),
            "verify":"assert(observed_card.edition==nil and other_playing_card.edition==nil)"}));
    }
    for (name,trigger,context) in [
        ("discarded","card_discarded","{discard=true,other_card=observed_card}"),
        ("held","card_held_in_hand","{individual=true,cardarea=G.hand,other_card=observed_card}"),
        ("held_round_end","card_held_in_hand_end_of_round","{individual=true,cardarea=G.hand,end_of_round=true,other_card=observed_card}"),
    ] {
        let definition=joker(json!([{"id":"modify","trigger":trigger,"effects":[{
            "effect_type":"edit_playing_card","params":{"new_edition":"polychrome"}
        }]}]));
        cases.push(json!({"kind":"rule_options","name":format!("playing_card_transform_{name}_cleared_context"),
            "playing_card_transform_runtime":true,"code":Emitter::new().emit_chunk(&compile_joker(&definition,"mod")),
            "invoke":format!("local context={context};local effect=test_definition:calculate(actor,context);context.other_card=nil;SMODS.calculate_effect(effect,actor);run_events()"),
            "verify":"assert(observed_card.edition.key=='e_polychrome');assert(other_playing_card.edition==nil)"}));
    }
    for (name,edition,expected) in [
        ("custom",json!("e_mod_shiny"),"e_mod_shiny"),
        ("random",json!("random"),"e_holo"),
        ("legacy_alias",json!("polychrome"),"e_polychrome"),
    ] {
        let effect=if name=="legacy_alias" {"edit_card"} else {"edit_playing_card"};
        let definition=joker(json!([{"id":"modify","trigger":"card_scored","effects":[{
            "effect_type":effect,"params":{"new_edition":edition}
        }]}]));
        cases.push(json!({"kind":"rule_options","name":format!("playing_card_transform_{name}"),
            "playing_card_transform_runtime":true,"code":Emitter::new().emit_chunk(&compile_joker(&definition,"mod")),
            "invoke":"SMODS.score_card(observed_card,{cardarea=G.play});run_events()",
            "verify":format!("assert(observed_card.edition.key=='{expected}');assert(other_playing_card.edition==nil)")}));
    }
    for scope in ["local","global","persistent"] {
        let mut definition=joker(json!([{"id":"modify","trigger":"card_scored","effects":[{
            "effect_type":"edit_playing_card","params":{"new_edition":{"value":"chosen_edition","valueType":"userVariable"}}
        }]}]));
        definition.user_variables=serde_json::from_value(json!([{"name":"chosen_edition","var_type":"key","initial_value":"e_polychrome",
            "is_global":scope!="local","is_persistent":scope=="persistent"}])).unwrap();
        cases.push(json!({"kind":"rule_options","name":format!("playing_card_transform_{scope}_edition_variable"),
            "playing_card_transform_runtime":true,"code":Emitter::new().emit_chunk(&compile_joker(&definition,"mod")),
            "prepare":"G.GAME.jf_global_vars={chosen_edition='e_polychrome'};JF_GLOBALS={chosen_edition='e_polychrome'}",
            "invoke":"SMODS.score_card(observed_card,{cardarea=G.play});run_events()",
            "verify":"assert(observed_card.edition.key=='e_polychrome');assert(other_playing_card.edition==nil)"}));
    }
    for object in ["enhancement","seal","edition"] {
        let input=json!({"key":"runtime_test","name":"Runtime Test","description":["Test"],"atlas":"CustomCards","pos":{"x":0,"y":0},
            "rules":[{"id":"modify","trigger":"card_scored","effects":[{"effect_type":"edit_playing_card","params":{"new_edition":"polychrome"}}]}]});
        let chunk=match object {
            "seal"=>compile_seal(&serde_json::from_value::<SealDef>(input).unwrap(),"mod"),
            "edition"=>compile_edition(&serde_json::from_value::<EditionDef>(input).unwrap(),"mod"),
            _=>compile_enhancement(&serde_json::from_value::<EnhancementDef>(input).unwrap(),"mod"),
        };
        cases.push(json!({"kind":"rule_options","name":format!("playing_card_transform_{object}_self_target"),
            "playing_card_transform_runtime":true,"code":Emitter::new().emit_chunk(&chunk),
            "prepare":"actor=observed_card;actor.ability.extra=copy_table((test_definition.config or {}).extra or {});actor.ability.seal=copy_table(test_definition.config or {})",
            "invoke":"local context={main_scoring=true,cardarea=G.play,other_card=other_playing_card};local effect=test_definition:calculate(actor,context);context.other_card=nil;if effect then SMODS.calculate_effect(effect,actor) end;run_events()",
            "verify":"assert(actor.edition and actor.edition.key=='e_polychrome');assert(other_playing_card.edition==nil)"}));
    }
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

fn append_booster_option_cases(cases: &mut Vec<Value>) {
    rule_option_case(cases,"booster_voucher_choice_add","voucher","edit_booster_packs",
        json!({"selected_type":"choice","operation":"add","value":2}),"",
        "assert_booster(existing_first_pack,3,8);assert_booster(booster_card('p_second'),4,10);assert_booster_centers_unchanged()");
    cases.last_mut().unwrap()["booster_open_runtime"]=json!(true);

    rule_option_case(cases,"booster_voucher_native_choice_cap","voucher","edit_booster_packs",
        json!({"selected_type":"choice","operation":"add","value":30}),"existing_first_pack.ability.extra=3",
        "assert_booster(existing_first_pack,3,3);assert_booster(booster_card('p_first'),8,8);assert_booster(booster_card('p_second'),10,10);assert_booster_centers_unchanged()");
    cases.last_mut().unwrap()["booster_open_runtime"]=json!(true);

    for object in ["voucher", "voucher_passive"] {
        for selected_type in ["choice", "size"] {
            for (operation, expected_modifier) in [("add",4), ("subtract",0), ("set",2)] {
                let (choice, size) = if selected_type=="choice" {(expected_modifier,2)} else {(2,expected_modifier)};
                let verify=format!("assert(G.GAME.modifiers.booster_choice_mod=={choice});assert(G.GAME.modifiers.booster_size_mod=={size});assert_booster(existing_first_pack,{},{});assert_booster(existing_second_pack,{},{});assert_booster(booster_card('p_first'),{},{});assert_booster(booster_card('p_second'),{},{});assert_booster_centers_unchanged()",
                    1+choice,8+size,2+choice,10+size,1+choice,8+size,2+choice,10+size);
                rule_option_case(cases,&format!("booster_{object}_{selected_type}_{operation}"),object,"edit_booster_packs",
                    json!({"selected_type":selected_type,"operation":operation,"value":2}),
                    "G.GAME.modifiers={booster_choice_mod=2,booster_size_mod=2}",&verify);
                cases.last_mut().unwrap()["booster_open_runtime"]=json!(true);
            }
        }
        for (mode,value) in [
            ("typed_number",json!({"value":3,"valueType":"number"})),
            ("user_variable",json!({"value":"amount","valueType":"userVariable"})),
            ("game_variable",json!({"value":"GAMEVAR:joker_count|1|0","valueType":"gameVariable"})),
        ] {
            rule_option_case(cases,&format!("booster_{object}_choice_{mode}"),object,"edit_booster_packs",
                json!({"selected_type":"choice","operation":"add","value":value}),"",
                "assert_booster(existing_first_pack,4,8);assert_booster(booster_card('p_second'),5,10);assert_booster_centers_unchanged()");
            cases.last_mut().unwrap()["booster_open_runtime"]=json!(true);
        }
    }
    rule_option_case(cases,"booster_voucher_saved_run_and_new_run","voucher_passive","edit_booster_packs",
        json!({"selected_type":"choice","operation":"add","value":2}),"",
        "assert_booster(existing_first_pack,3,8);assert_booster(booster_card('p_second'),4,10);local saved_game=copy_table(G.GAME);G.GAME=copy_table(saved_game);assert_booster(existing_first_pack,3,8);assert_booster(booster_card('p_second'),4,10);G.GAME={modifiers={}};assert_booster(existing_first_pack,1,8);assert_booster(existing_second_pack,2,10);assert_booster(booster_card('p_first'),1,8);assert_booster(booster_card('p_second'),2,10);assert_booster_centers_unchanged()");
    cases.last_mut().unwrap()["booster_open_runtime"]=json!(true);

    rule_option_case(cases,"booster_voucher_native_size_and_choice_limits","voucher","edit_booster_packs",
        json!({"selected_type":"choice","operation":"add","value":30}),"G.GAME.modifiers.booster_size_mod=-20",
        "assert_booster(existing_first_pack,1,1);assert_booster(booster_card('p_second'),1,1);assert_booster_centers_unchanged()");
    cases.last_mut().unwrap()["booster_open_runtime"]=json!(true);

    for (name,rules,prepare,expected_choice,expected_size) in [
        ("multiple_rules",json!([
            {"id":"one","trigger":"card_used","effects":[{"effect_type":"edit_booster_packs","params":{"selected_type":"choice","operation":"add","value":2}}]},
            {"id":"two","trigger":"passive","effects":[{"effect_type":"edit_booster_packs","params":{"selected_type":"size","operation":"add","value":3}},{"effect_type":"edit_booster_packs","params":{"selected_type":"choice","operation":"subtract","value":1}}]}
        ]),"",2,11),
        ("loop",json!([{"id":"one","trigger":"passive","loop_groups":[{"id":"loop","count":3,"effects":[{"effect_type":"edit_booster_packs","params":{"selected_type":"choice","operation":"add","value":1}}]}]}]),"",4,8),
        ("chance_success",json!([{"id":"one","trigger":"passive","random_groups":[{"id":"chance","chance_numerator":1,"chance_denominator":2,"effects":[{"effect_type":"edit_booster_packs","params":{"selected_type":"choice","operation":"add","value":2}}]}]}]),"SMODS.pseudorandom_probability=function() return true end",3,8),
        ("chance_miss",json!([{"id":"one","trigger":"passive","random_groups":[{"id":"chance","chance_numerator":1,"chance_denominator":2,"effects":[{"effect_type":"edit_booster_packs","params":{"selected_type":"choice","operation":"add","value":2}}]}]}]),"SMODS.pseudorandom_probability=function() return false end",1,8),
        ("condition_miss",json!([{"id":"one","trigger":"passive","condition_groups":[{"conditions":[{"condition_type":"player_money","params":{"operator":"greater_than","value":10}}]}],"effects":[{"effect_type":"edit_booster_packs","params":{"selected_type":"choice","operation":"add","value":2}}]}]),"G.GAME.dollars=5",1,8),
    ] {
        let definition:VoucherDef=serde_json::from_value(json!({"key":"runtime_test","name":"Runtime Test","description":["Test"],"atlas":"CustomVouchers","pos":{"x":0,"y":0},"rules":rules})).unwrap();
        cases.push(json!({"kind":"rule_options","name":format!("booster_voucher_{name}"),"booster_open_runtime":true,
            "code":Emitter::new().emit_chunk(&compile_voucher(&definition,"mod")),"prepare":prepare,"invoke":"test_definition:redeem(actor)",
            "verify":format!("assert_booster(existing_first_pack,{expected_choice},{expected_size});assert_booster(booster_card('p_second'),{},{});assert_booster_centers_unchanged()",expected_choice+1,expected_size+2)}));
    }

    for (operation, expected) in [("add",4),("subtract",0),("set",2)] {
        rule_option_case(cases,&format!("booster_joker_passive_choice_{operation}"),"joker_passive","edit_booster_packs",
            json!({"selected_type":"choice","operation":operation,"value":2}),
            "G.GAME.modifiers.booster_choice_mod=2;actor.config={center={key='j_mod_runtime_test'}};G.jokers.cards={actor}",
            &format!("assert_booster(existing_first_pack,{},8);assert_booster(booster_card('p_second'),{},10);test_definition:remove_from_deck(actor);assert_booster(existing_first_pack,3,8);assert_booster(booster_card('p_second'),4,10);assert_booster_centers_unchanged()",expected+1,expected+2));
        cases.last_mut().unwrap()["booster_open_runtime"]=json!(true);
        cases.last_mut().unwrap()["invoke"]=json!("test_definition:add_to_deck(actor)");
    }
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

fn append_required_flags_cases(cases: &mut Vec<Value>) {
    let frontend_code = |field: Option<(&str, Value)>| {
        let mut input = json!({
            "objectKey":"runtime_test", "name":"Runtime Test", "description":"Test",
            "cost":4, "rarity":"common", "unlocked":true, "discovered":true, "rules":[]
        });
        if let Some((key, value)) = field { input[key] = value; }
        let input: export::JokerDataInput = serde_json::from_value(input).unwrap();
        let definition = export::joker_data_to_def(&input, "mod", export::AtlasPosInput { x:0, y:0 }, None);
        Emitter::new().emit_chunk(&compile_joker(&definition, "mod"))
    };
    let emit_flag_code = |change: &str| Emitter::new().emit_chunk(&compile_joker(&joker(json!([{
        "id":"change_flag", "trigger":"hand_played", "effects":[{
            "effect_type":"emit_flag", "params":{"flag_name":"1 active-flag!", "change":change, "display_message":"n"}
        }]
    }])), "mod"));
    cases.push(json!({"kind":"rule_options", "name":"required_flags_frontend_emit_set_clear_and_rounds",
        "pool_dispatch_runtime":true,
        "code":format!("{}\nrequired=test_definition\n{}\nsetter=test_definition\n{}\nclearer=test_definition",
            frontend_code(Some(("appearFlags", json!(["1 active-flag!"])))),
            emit_flag_code("true"), emit_flag_code("false")),
        "prepare":"setter_card={ability=copy_table(setter.config)};clearer_card={ability=copy_table(clearer.config)};G.GAME.pool_flags=nil",
        "invoke":"assert(not SMODS.add_to_pool(required,{source='shop'}),'unset required flag must reject');setter:calculate(setter_card,{joker_main=true});assert(SMODS.add_to_pool(required,{source='shop'}),'Emit Flag must enable the advanced restriction');G.GAME.current_round={};assert(SMODS.add_to_pool(required,{source='shop'}),'required flags must survive a new round');clearer:calculate(clearer_card,{joker_main=true})",
        "verify":"assert(not SMODS.add_to_pool(required,{source='shop'}),'cleared flag must immediately reject');assert(G.GAME.pool_flags.mod_1_active_flag_==false);assert(#status_messages==0)"}));

    for (name, field, flags) in [
        ("frontend_array", "appearFlags", json!(["enabled", "second", "not blocked"])),
        ("legacy_array", "appear_flags", json!(["enabled", "second", "not blocked"])),
        ("legacy_csv", "appear_flags", json!(" enabled, second , not blocked ")),
        ("frontend_csv", "appearFlags", json!(" enabled, second , not blocked ")),
    ] {
        cases.push(json!({"kind":"rule_options", "name":format!("required_flags_{name}_all_and_negated"),
            "pool_dispatch_runtime":true, "code":frontend_code(Some((field, flags))),
            "prepare":"G.GAME.pool_flags={mod_enabled=true}",
            "invoke":"assert(not SMODS.add_to_pool(test_definition,{source='shop'}),'all required flags must be active');G.GAME.pool_flags.mod_second=true;assert(SMODS.add_to_pool(test_definition,{source='shop'}),'unset negated flag must permit');G.GAME.pool_flags.mod_blocked=true;assert(not SMODS.add_to_pool(test_definition,{source='shop'}),'active negated flag must reject');G.GAME.pool_flags.mod_blocked=false;assert(SMODS.add_to_pool(test_definition,{source='shop'}),'clearing a negated flag must permit');G.GAME.pool_flags.mod_enabled=false",
            "verify":"assert(not SMODS.add_to_pool(test_definition,{source='shop'}))"}));
    }
    for (name, field) in [
        ("missing", None), ("null", Some(("appearFlags", json!(null)))),
        ("empty_array", Some(("appearFlags", json!([])))),
        ("empty_csv", Some(("appear_flags", json!(" , , ")))),
    ] {
        cases.push(json!({"kind":"rule_options", "name":format!("required_flags_unrestricted_{name}"),
            "pool_dispatch_runtime":true, "code":frontend_code(field), "setup":"G=nil",
            "invoke":"assert(test_definition.in_pool==nil)",
            "verify":"assert(SMODS.add_to_pool(test_definition,{source='shop'}),'no flags must preserve native pool eligibility')"}));
    }
    for (name, setup) in [("missing_global", "G=nil"), ("missing_game", "G={}"), ("missing_pool_flags", "G={GAME={}}"),
        ("no_round", "G={GAME={pool_flags={mod_enabled=true}}}")] {
        cases.push(json!({"kind":"rule_options", "name":format!("required_flags_safe_{name}"),
            "pool_dispatch_runtime":true,
            "code":format!("{}\npositive=test_definition\n{}\nnegative=test_definition",
                frontend_code(Some(("appearFlags", json!(["enabled"])))),
                frontend_code(Some(("appearFlags", json!(["not blocked"]))))),
            "setup":setup,
            "invoke":format!("assert((not not SMODS.add_to_pool(positive,{{source='shop'}}))=={})", name=="no_round"),
            "verify":"assert(SMODS.add_to_pool(negative,{source='shop'}),'negated missing flag must be safe and permit')"}));
    }
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
    append_blind_win_cases(&mut cases);
    append_edition_shader_cases(&mut cases);
    append_rarity_shop_cases(&mut cases);
    append_description_blank_line_cases(&mut cases);
    append_description_format_cases(&mut cases);
    append_consumable_creation_message_cases(&mut cases);
    append_size_message_cases(&mut cases);
    append_probability_result_cases(&mut cases);
    append_probability_chain_cases(&mut cases);
    append_game_variable_description_and_loop_cases(&mut cases);
    append_retrigger_scoring_cases(&mut cases);
    append_card_retrigger_cases(&mut cases);
    append_card_self_destruct_cases(&mut cases);
    append_playing_card_transform_cases(&mut cases);
    append_deck_settings_cases(&mut cases);
    append_deck_card_target_cases(&mut cases);
    append_joker_creation_cases(&mut cases);
    append_rule_option_cases(&mut cases);
    append_booster_option_cases(&mut cases);
    append_joker_selection_and_key_cases(&mut cases);
    append_joker_selection_size_cases(&mut cases);
    append_required_flags_cases(&mut cases);
    append_flag_and_variable_cases(&mut cases);
    append_global_lifecycle_case(&mut cases);
    append_typed_global_cases(&mut cases);
    std::fs::write(output, serde_json::to_vec(&cases).unwrap()).unwrap();
}
