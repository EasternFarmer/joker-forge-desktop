use super::*;
use std::collections::HashMap;

fn preview_test_joker(rules: serde_json::Value) -> JokerDef {
    serde_json::from_value(serde_json::json!({
        "key": "preview_test", "name": "Preview Test", "description": ["Test"],
        "cost": 4, "rarity": "common", "blueprint_compat": true, "eternal_compat": true,
        "perishable_compat": true, "unlocked": true, "discovered": true,
        "atlas": "CustomJokers", "pos": { "x": 0, "y": 0 }, "rules": rules
    }))
    .unwrap()
}

fn field_binding_text<'a>(code: &'a str, binding: &LuaFieldBinding) -> &'a str {
    assert_eq!(binding.start_line, binding.end_line);
    let line = code.lines().nth(binding.start_line - 1).unwrap();
    &line[binding.start_column - 1..binding.end_column - 1]
}

#[test]
fn live_preview_bindings_target_each_condition_and_effect_source() {
    for system in ["Windows", "Linux", "OS X"] {
        let joker = preview_test_joker(serde_json::json!([{
            "id": "rule-os", "trigger": "hand_played", "condition_groups": [{ "conditions": [
                { "id": "condition-os", "condition_type": "system_condition", "params": { "system": system } }
            ] }],
            "effects": [{ "id": "effect-mult", "effect_type": "add_mult", "params": { "value": 7, "customMessage": "Windows" } }]
        }]));
        let chunk = compile_joker(&joker, "mod");
        let (code, segments, bindings) = Emitter::new().emit_chunk_with_field_bindings(&chunk);
        let system_path = serde_json::json!([
            "rules",
            0,
            "conditionGroups",
            0,
            "conditions",
            0,
            "params",
            "system",
            "value"
        ]);
        let system_binding = bindings
            .iter()
            .find(|binding| serde_json::json!(binding.source_path) == system_path)
            .expect("OS field must be editable");
        assert_eq!(system_binding.original_value, serde_json::json!(system));
        assert_eq!(
            field_binding_text(&code, system_binding),
            format!("'{system}'")
        );
        let value_path = serde_json::json!(["rules", 0, "effects", 0, "params", "value", "value"]);
        let value_binding = bindings
            .iter()
            .find(|binding| serde_json::json!(binding.source_path) == value_path)
            .expect("numeric config must be editable");
        assert_eq!(field_binding_text(&code, value_binding), "7");
        assert!(bindings.iter().any(|binding| binding.source_path
            == vec![
                serde_json::json!("rules"),
                serde_json::json!(0),
                serde_json::json!("effects"),
                serde_json::json!(0),
                serde_json::json!("params"),
                serde_json::json!("customMessage"),
                serde_json::json!("value")
            ]));
        assert!(!segments.is_empty());
    }
}

#[test]
fn live_preview_bindings_cover_condition_config_chances_and_loop_counts() {
    let joker = preview_test_joker(serde_json::json!([{
        "id": "rule", "trigger": "hand_played", "condition_groups": [{ "conditions": [
            { "id": "money", "condition_type": "player_money", "params": { "value": 12, "operator": "greater_than" } }
        ] }],
        "random_groups": [{ "id": "chance", "chance_numerator": 1, "chance_denominator": 8,
            "effects": [{ "id": "xmult", "effect_type": "apply_x_mult", "params": { "value": 5 } }] }],
        "loop_groups": [{ "id": "loop", "count": 3,
            "effects": [{ "id": "mult", "effect_type": "add_mult", "params": { "value": 2 } }] }]
    }]));
    let chunk = compile_joker(&joker, "mod");
    let (code, _, bindings) = Emitter::new().emit_chunk_with_field_bindings(&chunk);
    for (path, expected) in [
        (
            serde_json::json!([
                "rules",
                0,
                "conditionGroups",
                0,
                "conditions",
                0,
                "params",
                "value",
                "value"
            ]),
            "12",
        ),
        (
            serde_json::json!(["rules", 0, "randomGroups", 0, "chance_numerator", "value"]),
            "1",
        ),
        (
            serde_json::json!(["rules", 0, "randomGroups", 0, "chance_denominator", "value"]),
            "8",
        ),
        (
            serde_json::json!(["rules", 0, "loops", 0, "repetitions", "value"]),
            "3",
        ),
        (
            serde_json::json!(["rules", 0, "loops", 0, "effects", 0, "params", "value", "value"]),
            "2",
        ),
    ] {
        let binding = bindings
            .iter()
            .find(|binding| serde_json::json!(binding.source_path) == path)
            .expect("generated config should retain its source");
        assert_eq!(field_binding_text(&code, binding), expected);
    }
    assert!(!bindings
        .iter()
        .any(|binding| binding.source_path.contains(&serde_json::json!("operator"))));
}

#[test]
fn live_preview_omits_ambiguous_literals_and_keeps_unrelated_fields_separate() {
    let joker = preview_test_joker(serde_json::json!([{
        "id": "rule", "trigger": "hand_played", "condition_groups": [{ "conditions": [
            { "id": "ambiguous", "condition_type": "system_condition", "params": { "system": "Windows", "unrelated": "Windows" } },
            { "id": "exact", "condition_type": "system_condition", "params": { "system": "Windows" } }
        ] }],
        "effects": [{ "id": "mult", "effect_type": "add_mult", "params": { "value": 4 } }]
    }]));
    let chunk = compile_joker(&joker, "mod");
    let (code, _, bindings) = Emitter::new().emit_chunk_with_field_bindings(&chunk);
    let os_bindings: Vec<_> = bindings
        .iter()
        .filter(|binding| binding.original_value == serde_json::json!("Windows"))
        .collect();
    assert_eq!(os_bindings.len(), 1);
    assert_eq!(os_bindings[0].source_path[5], serde_json::json!(1));
    assert_eq!(code.matches("love.system.getOS() == 'Windows'").count(), 2);
}

#[test]
fn live_preview_normalizes_typed_numeric_string_fields() {
    let joker = preview_test_joker(serde_json::json!([{
        "id": "rule", "trigger": "hand_played", "effects": [{
            "id": "mult", "effect_type": "add_mult", "params": { "value": { "value": "7", "valueType": "number" } }
        }]
    }]));
    let chunk = compile_joker(&joker, "mod");
    let (code, _, bindings) = Emitter::new().emit_chunk_with_field_bindings(&chunk);
    let binding = bindings
        .iter()
        .find(|binding| {
            binding.source_path
                == vec![
                    serde_json::json!("rules"),
                    serde_json::json!(0),
                    serde_json::json!("effects"),
                    serde_json::json!(0),
                    serde_json::json!("params"),
                    serde_json::json!("value"),
                    serde_json::json!("value"),
                ]
        })
        .unwrap();
    assert_eq!(binding.value_type, "number");
    assert_eq!(binding.original_value, serde_json::json!(7));
    assert_eq!(field_binding_text(&code, binding), "7");
}

fn ordered_description_code(ctx: &CompileContext) -> String {
    let loc_vars = build_shared_loc_vars(ctx, &[]).expect("ordered bindings should emit loc_vars");
    Emitter::new().emit_expr_to_string(&loc_vars)
}

#[test]
fn description_bindings_preserve_slots_and_duplicate_literals() {
    let mut ctx = CompileContext::new(ObjectType::Joker, "mod".into(), "test".into(), false);
    ctx.set_user_vars(vec![UserVariableDef {
        name: "label".into(),
        var_type: UserVarType::Text,
        initial_value: ParamValue::Str("chance".into()),
        is_global: false,
        is_persistent: false,
    }]);
    let rules: Vec<RuleDef> = serde_json::from_value(serde_json::json!([{
        "id": "rule", "trigger": "hand_played",
        "effects": [{ "id": "hidden", "effect_type": "juice_up_card", "params": { "scale": 4, "rotation": 3 } }],
        "random_groups": [{ "id": "chance", "chance_numerator": 1, "chance_denominator": 8,
            "effects": [{ "id": "mult", "effect_type": "apply_x_mult", "params": { "value": 5 } }] }]
    }])).unwrap();
    compile_rules(&rules, &mut ctx);
    ctx.set_description_variables(Some(
        serde_json::from_value(serde_json::json!([
            { "kind": "probability", "group_id": "chance", "part": "numerator" },
            { "kind": "probability", "group_id": "chance", "part": "denominator" },
            { "kind": "user", "name": "label" },
            { "kind": "literal", "value": 7 },
            { "kind": "literal", "value": 7 },
            { "kind": "config", "name": "Xmult0", "effect_id": "mult", "fallback": 5 }
        ]))
        .unwrap(),
    ));
    let code = ordered_description_code(&ctx);
    let vars = &code[index_of(&code, "vars = {")..];
    assert!(index_of(vars, "description_numerator0") < index_of(vars, "description_denominator0"));
    assert!(index_of(vars, "description_denominator0") < index_of(vars, "['label']"));
    assert!(index_of(vars, "['label']") < index_of(vars, "['Xmult0']"));
    assert_eq!(
        vars.lines()
            .filter(|line| line.trim().trim_end_matches(',') == "7")
            .count(),
        2
    );
    assert!(!vars.contains("juice_scale"));
    assert!(!vars.contains("juice_rotation"));
    assert_eq!(code.matches("SMODS.get_probability_vars").count(), 1);
}

#[test]
fn description_probabilities_use_group_identity_across_skipped_and_many_groups() {
    let mut ctx = CompileContext::new(ObjectType::Joker, "mod".into(), "test".into(), false);
    let mut groups = vec![
        serde_json::json!({ "id": "empty", "chance_numerator": 3, "chance_denominator": 9, "effects": [] }),
    ];
    groups.extend((0..12).map(|index| serde_json::json!({
        "id": format!("group{index}"), "chance_numerator": index + 1, "chance_denominator": index + 20,
        "effects": [{ "id": format!("effect{index}"), "effect_type": "add_mult", "params": { "value": 2 } }]
    })));
    let rules: Vec<RuleDef> = serde_json::from_value(
        serde_json::json!([{ "id": "rule", "trigger": "hand_played", "random_groups": groups }]),
    )
    .unwrap();
    compile_rules(&rules, &mut ctx);
    ctx.set_description_variables(Some(
        serde_json::from_value(serde_json::json!([
            { "kind": "probability", "group_id": "group10", "part": "denominator" },
            { "kind": "probability", "group_id": "group2", "part": "numerator" },
            { "kind": "probability", "group_id": "empty", "part": "numerator" },
            { "kind": "probability", "group_id": "group10", "part": "numerator" }
        ]))
        .unwrap(),
    ));
    let code = ordered_description_code(&ctx);
    let first_pair = &code[..index_of(&code, "local description_numerator1")];
    assert!(first_pair.contains("['numerator_10']"));
    assert!(first_pair.contains("['odds_10']"));
    assert!(code.contains("SMODS.get_probability_vars(card, 3, 9, 'j_mod_test')"));
    let vars = &code[index_of(&code, "vars = {")..];
    assert!(index_of(vars, "description_denominator0") < index_of(vars, "description_numerator1"));
    assert!(index_of(vars, "description_numerator1") < index_of(vars, "description_numerator2"));
    assert!(index_of(vars, "description_numerator2") < index_of(vars, "description_numerator0"));
}

#[test]
fn description_config_bindings_resolve_actual_effect_names_and_dynamic_fallbacks() {
    let mut ctx = CompileContext::new(ObjectType::Edition, "mod".into(), "test".into(), false);
    ctx.set_user_vars(vec![UserVariableDef {
        name: "amount".into(),
        var_type: UserVarType::Number,
        initial_value: ParamValue::Int(4),
        is_global: false,
        is_persistent: false,
    }]);
    let rules: Vec<RuleDef> = serde_json::from_value(serde_json::json!([{
        "id": "rule", "trigger": "card_scored", "effects": [
            { "id": "dynamic", "effect_type": "add_chips", "params": { "value": { "value": "amount", "valueType": "user_var" } } },
            { "id": "literal", "effect_type": "add_chips", "params": { "value": 17 } }
        ]
    }])).unwrap();
    compile_rules(&rules, &mut ctx);
    ctx.set_description_variables(Some(serde_json::from_value(serde_json::json!([
        { "kind": "config", "name": "chips0", "effect_id": "literal", "fallback": 17 },
        { "kind": "config", "name": "chips1", "effect_id": "dynamic", "fallback": { "value": "amount", "valueType": "user_var" } },
        { "kind": "game", "id": "os.execute('unsafe')" }
    ])).unwrap()));
    let code = ordered_description_code(&ctx);
    assert!(code.contains("card.edition.extra['chips1']"));
    assert!(code.contains("card.edition.extra['amount']"));
    assert!(!code.contains("['chips0']"));
    assert!(!code.contains("os.execute"));
}

#[test]
fn description_user_bindings_guard_scoped_values_and_global_defaults() {
    for object_type in [
        ObjectType::Joker,
        ObjectType::Seal,
        ObjectType::Edition,
        ObjectType::Deck,
    ] {
        let mut ctx = CompileContext::new(object_type, "mod".into(), "test".into(), false);
        ctx.set_user_vars(vec![
            UserVariableDef {
                name: "amount".into(),
                var_type: UserVarType::Number,
                initial_value: ParamValue::Int(3),
                is_global: false,
                is_persistent: false,
            },
            UserVariableDef {
                name: "shared".into(),
                var_type: UserVarType::Number,
                initial_value: ParamValue::Int(6),
                is_global: true,
                is_persistent: false,
            },
        ]);
        ctx.set_description_variables(Some(vec![
            DescriptionVariableBinding::User {
                name: "amount".into(),
            },
            DescriptionVariableBinding::User {
                name: "shared".into(),
            },
        ]));
        let code = ordered_description_code(&ctx);
        assert!(code.contains(&format!("{}['amount']", object_type.ability_path())));
        assert!(code.contains("self.config.extra['amount']"));
        assert!(code.contains("G.GAME.jf_global_vars['shared']"));
        assert!(code.contains("or 6"));
    }
}

#[test]
fn description_game_bindings_accept_current_catalog_ids_and_dynamic_parameters() {
    let mut ctx = CompileContext::new(ObjectType::Joker, "mod".into(), "test".into(), false);
    ctx.set_description_variables(Some(serde_json::from_value(serde_json::json!([
        { "kind": "game", "id": "cards_in_deck" },
        { "kind": "game", "id": "cards_in_hand" },
        { "kind": "game", "id": "total_playing_cards" },
        { "kind": "game", "id": "blind_chip_req" },
        { "kind": "config", "name": "chips0", "fallback": { "value": "GAMEVAR:current_money|2|3", "valueType": "gameVariable" } }
    ])).unwrap()));
    let code = ordered_description_code(&ctx);
    assert!(code.contains("G.deck.cards"));
    assert!(code.contains("G.hand.cards"));
    assert!(code.contains("G.playing_cards"));
    assert!(code.contains("G.GAME.blind.chips"));
    assert!(code.contains("G.GAME.dollars"));
    assert!(code.contains("+ 3"));
    assert!(code.contains("* 2"));
    assert!(!code.contains("context."));
}

fn make_rule_output(
    rule_id: &str,
    trigger: &str,
    condition_expr: Option<Expr>,
    message: &str,
) -> RuleOutput {
    RuleOutput {
        rule_id: rule_id.to_string(),
        trigger: trigger.to_string(),
        condition_expr,
        condition_segment_ids: vec![],
        effect_stmts: vec![lua_return(lua_table(vec![("message", lua_str(message))]))],
        is_passive: false,
        passive_outputs: vec![],
        passive_hooks: vec![],
        has_retrigger: false,
        has_destroy: false,
        blind_rewards: vec![],
    }
}

fn index_of(haystack: &str, needle: &str) -> usize {
    haystack
        .find(needle)
        .unwrap_or_else(|| panic!("Expected '{needle}' in:\n{haystack}"))
}

fn effect_code(effect_type: &str, params: &[(&str, ParamValue)]) -> String {
    effect_code_with_user_vars(effect_type, params, Vec::new())
}

fn effect_code_with_prefix(
    effect_type: &str,
    params: &[(&str, ParamValue)],
    mod_prefix: &str,
) -> String {
    effect_code_with_prefix_and_user_vars(effect_type, params, mod_prefix, Vec::new())
}

fn effect_code_with_user_vars(
    effect_type: &str,
    params: &[(&str, ParamValue)],
    user_vars: Vec<UserVariableDef>,
) -> String {
    effect_code_with_prefix_and_user_vars(effect_type, params, "mod", user_vars)
}

fn effect_code_with_prefix_and_user_vars(
    effect_type: &str,
    params: &[(&str, ParamValue)],
    mod_prefix: &str,
    user_vars: Vec<UserVariableDef>,
) -> String {
    let mut ctx = CompileContext::new(
        ObjectType::Joker,
        mod_prefix.to_string(),
        "test".to_string(),
        false,
    );
    ctx.set_user_vars(user_vars);
    let effect = EffectDef {
        id: String::new(),
        effect_type: effect_type.to_string(),
        params: params
            .iter()
            .map(|(key, value)| ((*key).to_string(), value.clone()))
            .collect::<HashMap<_, _>>(),
    };
    let output =
        effects::compile_effect(&effect, &mut ctx, "hand_played").expect("effect should compile");
    Emitter::new().emit_stmts(&effects::build_return_block(&[output]))
}

#[test]
fn edit_dollars_respects_each_operation() {
    let cases = [
        ("add", "dollars = card.ability.extra.dollars0"),
        (
            "subtract",
            "dollars = -math.min(G.GAME.dollars, card.ability.extra.dollars0)",
        ),
        (
            "multiply",
            "dollars = (G.GAME.dollars * (card.ability.extra.dollars0)) - G.GAME.dollars",
        ),
        (
            "divide",
            "dollars = (G.GAME.dollars / (card.ability.extra.dollars0)) - G.GAME.dollars",
        ),
        (
            "set",
            "dollars = (card.ability.extra.dollars0) - G.GAME.dollars",
        ),
    ];

    for (operation, expected) in cases {
        let code = effect_code(
            "set_dollars",
            &[
                ("operation", ParamValue::Str(operation.to_string())),
                ("value", ParamValue::Int(5)),
            ],
        );
        assert!(
            code.contains(expected),
            "operation '{operation}' generated:\n{code}"
        );
    }
}

#[test]
fn scoring_effect_uses_user_var_when_typed_as_text() {
    let code = effect_code_with_user_vars(
        "add_mult",
        &[(
            "value",
            ParamValue::Typed(TypedValue {
                value: serde_json::Value::String("diagrammult".to_string()),
                value_type: "text".to_string(),
            }),
        )],
        vec![UserVariableDef {
            name: "diagrammult".to_string(),
            var_type: UserVarType::Number,
            initial_value: ParamValue::Int(2),
            is_global: false,
            is_persistent: false,
        }],
    );

    assert!(code.contains("mult = card.ability.extra.diagrammult"));
    assert!(!code.contains("mult = 'diagrammult'"));
}

#[test]
fn create_joker_uses_snake_case_specific_key_param() {
    let code = effect_code(
        "create_joker",
        &[
            ("joker_type", ParamValue::Str("specific".to_string())),
            ("joker_key", ParamValue::Str("j_greedy_joker".to_string())),
        ],
    );

    assert!(code.contains("set = 'Joker', key = 'j_greedy_joker'"));
}

#[test]
fn create_joker_normalizes_rarity_param_for_smods() {
    let code = effect_code(
        "create_joker",
        &[
            ("joker_type", ParamValue::Str("random".to_string())),
            ("rarity", ParamValue::Str("rare".to_string())),
        ],
    );

    assert!(code.contains("set = 'Joker', rarity = 'Rare'"));
}

#[test]
fn create_joker_pool_uses_custom_object_type_set() {
    let code = effect_code(
        "create_joker",
        &[
            ("joker_type", ParamValue::Str("pool".to_string())),
            ("pool", ParamValue::Str("overview_jokers".to_string())),
            ("edition", ParamValue::Str("negative".to_string())),
        ],
    );

    assert!(
        code.contains("SMODS.add_card({ set = 'mod_overview_jokers', edition = 'e_negative' })")
    );
    assert!(!code.contains("key_append = 'mod_overview_jokers'"));
}

#[test]
fn create_joker_pool_does_not_duplicate_current_mod_prefix() {
    let code = effect_code_with_prefix(
        "create_joker",
        &[
            ("joker_type", ParamValue::Str("pool".to_string())),
            ("pool", ParamValue::Str("overview_jokers".to_string())),
            ("edition", ParamValue::Str("negative".to_string())),
        ],
        "overview",
    );

    assert!(code.contains("SMODS.add_card({ set = 'overview_jokers', edition = 'e_negative' })"));
    assert!(!code.contains("set = 'overview_overview_jokers'"));
}

#[test]
fn in_pool_accepts_type_or_source_for_named_pools() {
    let ctx = CompileContext::new(
        ObjectType::Joker,
        "mod".to_string(),
        "test".to_string(),
        false,
    );
    let appearance = AppearanceDef {
        appears_in: vec!["mod_overview_jokers".to_string()],
        not_appears_in: vec![],
        appear_flags: vec![],
    };

    let expr = build_in_pool(&appearance, &ctx).expect("in_pool should be generated");
    let code = Emitter::new().emit_expr_to_string(&expr);

    assert!(code.contains("args.type == 'mod_overview_jokers'"));
    assert!(code.contains("args.source == 'mod_overview_jokers'"));
}

#[test]
fn destroy_joker_uses_selection_method_specific_key_param() {
    let code = effect_code(
        "destroy_joker",
        &[
            ("selection_method", ParamValue::Str("specific".to_string())),
            ("joker_key", ParamValue::Str("j_greedy_joker".to_string())),
        ],
    );

    assert!(code.contains("joker.config.center.key == 'j_greedy_joker'"));
}

#[test]
fn create_tag_normalizes_every_vanilla_picker_value_and_accepts_wrapped_params() {
    for tag in [
        "uncommon", "rare", "negative", "foil", "holo", "polychrome", "investment",
        "voucher", "boss", "standard", "charm", "meteor", "buffoon", "handy", "garbage",
        "ethereal", "coupon", "double", "juggle", "d_six", "top_up", "skip", "orbital", "economy",
    ] {
        for selected in [
            ParamValue::Str(tag.into()),
            ParamValue::Typed(TypedValue { value: serde_json::json!(tag), value_type: "text".into() }),
            ParamValue::Str(format!("tag_{tag}")),
        ] {
            let code = effect_code("create_tag", &[
                ("tag_type", ParamValue::Str("specific".into())),
                ("specific_tag", selected),
            ]);
            assert!(code.contains(&format!("if G.P_TAGS['tag_{tag}'] then")), "{code}");
            assert!(code.contains(&format!("add_tag(Tag('tag_{tag}'))")), "{code}");
            assert!(!code.contains("tag_tag_"), "{code}");
            assert!(!code.contains("set_ability"), "Tag constructor must initialize abilities once: {code}");
            assert!(code.contains("message = created_tag0 and 'Created Tag!'"), "{code}");
        }
    }
}

#[test]
fn create_tag_preserves_custom_keys_and_escapes_lua_literals() {
    for (selected, expected) in [
        ("  negative  ", "tag_negative"),
        ("tag_other_mod_bonus", "tag_other_mod_bonus"),
        ("custom_registered_key", "custom_registered_key"),
        ("tag_mod_quote'\\line\nnext", "tag_mod_quote\\'\\\\line\\nnext"),
    ] {
        let code = effect_code("create_tag", &[
            ("tag_type", ParamValue::Str("specific".into())),
            ("specific_tag", ParamValue::Str(selected.into())),
        ]);
        assert!(code.contains(&format!("add_tag(Tag('{expected}'))")), "{code}");
        assert!(code.contains(&format!("if G.P_TAGS['{expected}'] then")), "{code}");
    }
}

#[test]
fn create_tag_random_uses_eligible_keys_and_handles_an_empty_pool() {
    let code = effect_code("create_tag", &[("tag_type", ParamValue::Str("random".into()))]);
    assert!(code.contains("SMODS.get_clean_pool('Tag', nil, nil, 'create_tag')"), "{code}");
    assert!(code.contains("if #tag_pool0 > 0 then"), "{code}");
    assert!(code.contains("pseudorandom_element(tag_pool0, pseudoseed('create_tag'))"), "{code}");
    assert!(code.contains("if tag_key0 and G.P_TAGS[tag_key0] then"), "{code}");
    assert!(!code.contains("pseudorandom_element(G.P_TAGS"));
    assert!(!code.contains(").key"));
    assert!(!code.contains("set_ability"));
}

#[test]
fn create_tag_key_variables_use_live_values_and_skip_invalid_references() {
    let params = [
        ("tag_type", ParamValue::Str("keyvar".into())),
        ("variable", ParamValue::Str("chosen_tag".into())),
    ];
    for (is_global, is_persistent, expected) in [
        (false, false, "card.ability.extra.chosen_tag"),
        (true, false, "(G.GAME and G.GAME.jf_global_vars and G.GAME.jf_global_vars.chosen_tag)"),
        (true, true, "JF_GLOBALS.chosen_tag"),
    ] {
        let code = effect_code_with_user_vars("create_tag", &params, vec![UserVariableDef {
            name: "chosen_tag".into(), var_type: UserVarType::Key,
            initial_value: ParamValue::Str("negative".into()), is_global, is_persistent,
        }]);
        assert!(code.contains(&format!("local tag_key0 = {expected}")), "{code}");
        assert!(code.contains("if type(tag_key0) == 'string' then"), "{code}");
        assert!(code.contains("if not G.P_TAGS[tag_key0] and string.sub(tag_key0, 1, 4) ~= 'tag_' then"), "{code}");
        assert!(code.contains("tag_key0 = 'tag_' .. tag_key0"), "{code}");
        assert!(code.contains("if G.P_TAGS[tag_key0] then"), "{code}");
        assert!(!code.contains("tag_double"), "{code}");
    }
    let missing = effect_code("create_tag", &params);
    assert!(!missing.contains("add_tag("));
    assert!(!missing.contains("tag_double"));
    assert!(missing.contains("local created_tag0 = false"));
}

#[test]
fn repeated_create_tag_effects_keep_separate_success_messages() {
    let joker = preview_test_joker(serde_json::json!([{
        "id": "rule", "trigger": "hand_played", "effects": [
            { "effect_type": "create_tag", "params": { "tag_type": "specific", "specific_tag": "negative" } },
            { "effect_type": "create_tag", "params": { "tag_type": "random" } }
        ]
    }]));
    let code = Emitter::new().emit_chunk(&compile_joker(&joker, "mod"));
    assert!(code.contains("local created_tag0 = false"));
    assert!(code.contains("local created_tag1 = false"));
    assert!(code.contains("message = created_tag0 and 'Created Tag!'"));
    assert!(code.contains("message = created_tag1 and 'Created Tag!'"));
}

#[test]
fn create_playing_card_applies_every_static_property_from_raw_and_typed_params() {
    for typed in [false, true] {
        let params: Vec<(&str, ParamValue)> = [
            ("location", "hand"), ("rank", "8"), ("suit", "Hearts"),
            ("enhancement", "m_lucky"), ("seal", "Blue"), ("edition", "foil"),
        ].into_iter().map(|(key, value)| (key, if typed {
            ParamValue::Typed(TypedValue { value: serde_json::json!(value), value_type: "text".into() })
        } else { ParamValue::Str(value.into()) })).collect();
        let code = effect_code("create_playing_card", &params);
        for property in [
            "set = 'Base'", "area = G.hand", "rank = '8'", "suit = 'Hearts'",
            "enhancement = 'm_lucky'", "seal = 'Blue'", "edition = 'e_foil'", "no_edition = true",
        ] { assert!(code.contains(property), "missing {property}: {code}"); }
        assert_eq!(code.matches("SMODS.add_card(").count(), 1, "{code}");
        assert!(!code.contains(":emplace("), "SMODS.add_card must place the card once: {code}");
        assert_eq!(code.matches("playing_card_added = true").count(), 1, "{code}");
        assert!(code.contains("SMODS.calculate_context("), "{code}");
    }
    for rank in ["J", "Q", "K", "A"] {
        let code = effect_code("create_playing_card", &[("rank", ParamValue::Str(rank.into()))]);
        assert!(code.contains(&format!("rank = '{rank}'")), "{code}");
    }
}

#[test]
fn create_playing_card_none_and_default_location_do_not_generate_invalid_metadata() {
    for location in [
        ParamValue::Str("deck".into()), ParamValue::Str("[\"deck\"]".into()),
        ParamValue::Typed(TypedValue { value: serde_json::json!(["deck"]), value_type: "select".into() }),
    ] {
        let code = effect_code("create_playing_card", &[
            ("location", location), ("rank", ParamValue::Str("random".into())),
            ("suit", ParamValue::Str("random".into())), ("enhancement", ParamValue::Str("none".into())),
            ("seal", ParamValue::Str("none".into())), ("edition", ParamValue::Str("none".into())),
        ]);
        assert!(code.contains("area = G.deck"), "{code}");
        assert!(code.contains("no_edition = true"), "{code}");
        for property in ["rank =", "suit =", "enhancement =", "seal =", "edition = '"] {
            assert!(!code.contains(property), "None/random must not use literal sentinels: {code}");
        }
        assert!(!code.contains(":emplace("), "{code}");
    }
}

#[test]
fn create_playing_card_normalizes_legacy_modifiers_and_preserves_custom_keys() {
    let code = effect_code("create_playing_card", &[
        ("enhancement", ParamValue::Str("bonus".into())), ("seal", ParamValue::Str("blue".into())),
        ("edition", ParamValue::Str("sparkle".into())),
    ]);
    for property in ["enhancement = 'm_bonus'", "seal = 'Blue'", "edition = 'e_mod_sparkle'"] {
        assert!(code.contains(property), "{code}");
    }
    let custom = effect_code("create_playing_card", &[
        ("enhancement", ParamValue::Str("m_other_custom".into())),
        ("seal", ParamValue::Str("other_quote'\\seal\nnext".into())),
        ("edition", ParamValue::Str("e_other_custom".into())),
    ]);
    for property in ["enhancement = 'm_other_custom'", "seal = 'other_quote\\'\\\\seal\\nnext'", "edition = 'e_other_custom'"] {
        assert!(custom.contains(property), "{custom}");
    }
}

#[test]
fn create_playing_cards_random_modifiers_are_polled_without_sentinel_keys() {
    for effect in ["create_playing_card", "create_playing_cards"] {
        let code = effect_code(effect, &[
            ("rank", ParamValue::Str("random".into())), ("suit", ParamValue::Str("none".into())),
            ("enhancement", ParamValue::Str("random".into())), ("seal", ParamValue::Str("random".into())),
            ("edition", ParamValue::Str("random".into())), ("count", ParamValue::Int(3)),
        ]);
        for poller in ["SMODS.poll_enhancement(", "SMODS.poll_seal(", "SMODS.poll_edition("] {
            assert!(code.contains(poller), "missing {poller}: {code}");
        }
        assert!(code.contains("guaranteed = true"), "{code}");
        assert!(code.contains("no_edition = true"), "{code}");
        for property in ["rank = 'random'", "suit = 'none'", "enhancement = 'random'", "seal = 'random'", "edition = 'random'"] {
            assert!(!code.contains(property), "{code}");
        }
        assert_eq!(code.matches("SMODS.add_card(").count(), 1, "{code}");
        assert!(!code.contains(":emplace("), "{code}");
        if effect == "create_playing_cards" {
            for poller in ["SMODS.poll_enhancement(", "SMODS.poll_seal(", "SMODS.poll_edition("] {
                assert!(index_of(&code, "= 1, card.ability.extra.create_cards_count0 do") < index_of(&code, poller), "Random modifiers must be rerolled for each created card: {code}");
            }
        }
    }
}

#[test]
fn create_playing_card_uses_live_suit_rank_and_scoped_key_variables() {
    for value_type in [Some("userVariable"), Some("user_var"), None] {
        for (enhancement, seal, edition) in [
            ("m_bonus", "Blue", "e_foil"),
            ("none", "none", "none"),
            ("e_foil", "missing_seal", "m_bonus"),
        ] {
            let params: Vec<(&str, ParamValue)> = [
                ("suit", "chosen_suit"), ("rank", "chosen_rank"),
                ("enhancement", "chosen_enhancement"), ("seal", "chosen_seal"), ("edition", "chosen_edition"),
            ].into_iter().map(|(key, value)| (key, match value_type {
                Some(value_type) => ParamValue::Typed(TypedValue { value: serde_json::json!(value), value_type: value_type.into() }),
                None => ParamValue::Str(value.into()),
            })).collect();
            let vars = vec![
                UserVariableDef { name: "chosen_suit".into(), var_type: UserVarType::Suit, initial_value: ParamValue::Str("Spades".into()), is_global: false, is_persistent: false },
                UserVariableDef { name: "chosen_rank".into(), var_type: UserVarType::Rank, initial_value: ParamValue::Str("Ace".into()), is_global: false, is_persistent: false },
                UserVariableDef { name: "chosen_enhancement".into(), var_type: UserVarType::Key, initial_value: ParamValue::Str(enhancement.into()), is_global: false, is_persistent: false },
                UserVariableDef { name: "chosen_seal".into(), var_type: UserVarType::Key, initial_value: ParamValue::Str(seal.into()), is_global: true, is_persistent: false },
                UserVariableDef { name: "chosen_edition".into(), var_type: UserVarType::Key, initial_value: ParamValue::Str(edition.into()), is_global: true, is_persistent: true },
            ];
            let code = effect_code_with_user_vars("create_playing_card", &params, vars);
            for live_value in [
                "G.GAME.current_round.chosen_suit_card.suit", "G.GAME.current_round.chosen_rank_card.rank",
                "card.ability.extra.chosen_enhancement", "G.GAME.jf_global_vars.chosen_seal", "JF_GLOBALS.chosen_edition",
            ] { assert!(code.contains(live_value), "missing {live_value}: {code}"); }
            for quoted_name in ["'chosen_suit'", "'chosen_rank'", "'chosen_enhancement'", "'chosen_seal'", "'chosen_edition'"] {
                assert!(!code.contains(quoted_name), "Variable names must not become card properties: {code}");
            }
            assert!(!code.contains("chosen_rank_card.id"), "SMODS.create_card takes a rank name/card key: {code}");
            for (property, registry, object_kind) in [
                ("enhancement", "G.P_CENTERS[", Some("'Enhanced'")),
                ("seal", "G.P_SEALS[", None),
                ("edition", "G.P_CENTERS[", Some("'Edition'")),
            ] {
                let assignment = code.lines().find(|line| line.trim_start().starts_with(&format!("{property} = ")))
                    .unwrap_or_else(|| panic!("Missing {property} property in {code}"));
                assert!(assignment.contains("type(") && assignment.contains("'string'") && assignment.contains(registry),
                    "None, non-string, and unregistered keys must not reach the creation API: {assignment}");
                if let Some(object_kind) = object_kind {
                    assert!(assignment.contains(".set") && assignment.contains(object_kind),
                        "A registered key for another object kind must not reach {property}: {assignment}");
                }
            }
        }
    }
}

#[test]
fn create_playing_cards_resolves_pool_encodings_inside_the_count_loop() {
    for typed in [false, true] {
        let pool = |value: serde_json::Value| if typed {
            ParamValue::Typed(TypedValue { value, value_type: "checkbox".into() })
        } else { ParamValue::Str(value.to_string()) };
        let code = effect_code("create_playing_cards", &[
            ("count", if typed { ParamValue::Typed(TypedValue { value: serde_json::json!("3"), value_type: "number".into() }) } else { ParamValue::Int(3) }),
            ("rank", ParamValue::Str("pool".into())),
            ("rank_pool", pool(serde_json::json!(["2", "J", "A"]))),
            ("suit", ParamValue::Str("pool".into())), ("suit_pool", pool(serde_json::json!(["Hearts", "Clubs"]))),
        ]);
        assert!(code.contains("area = G.hand"), "Plural effect must add to hand: {code}");
        assert!(code.contains("= 1, card.ability.extra.create_cards_count0 do"), "{code}");
        assert!(index_of(&code, "= 1, card.ability.extra.create_cards_count0 do") < index_of(&code, "SMODS.add_card("), "Creation options must be fresh for each card: {code}");
        assert!(code.matches("pseudorandom_element(").count() >= 2, "{code}");
        for selection in ["'2'", "'J'", "'A'", "'Hearts'", "'Clubs'"] { assert!(code.contains(selection), "{code}"); }
        assert!(!code.contains("rank = 'pool'") && !code.contains("suit = 'pool'"), "{code}");
        assert!(!code.contains(":emplace("), "{code}");
        assert_eq!(code.matches("playing_card_added = true").count(), 1, "Notify once after the whole batch: {code}");
        assert!(code.contains("math.max("), "Older Steamodded deck capacity must include created cards: {code}");
    }
    for typed in [false, true] {
        let pool = |value: serde_json::Value| if typed {
            ParamValue::Typed(TypedValue { value, value_type: "checkbox".into() })
        } else { ParamValue::Str(value.to_string()) };
        let code = effect_code("create_playing_cards", &[
            ("count", ParamValue::Int(2)), ("rank", ParamValue::Str("pool".into())),
            ("rank_pool", pool(serde_json::json!([true, false, false, false, false, false, false, false, false, false, false, false, true]))),
            ("suit", ParamValue::Str("pool".into())),
            ("suit_pool", pool(serde_json::json!([true, false, true, false]))),
        ]);
        for selected in ["'2'", "'A'", "'Spades'", "'Diamonds'"] { assert!(code.contains(selected), "{code}"); }
        for excluded in ["'Hearts'", "'Clubs'", "'J'", "'K'"] { assert!(!code.contains(excluded), "{code}"); }
    }
    let empty = effect_code("create_playing_cards", &[
        ("count", ParamValue::Int(1)), ("rank", ParamValue::Str("pool".into())),
        ("rank_pool", ParamValue::Str("[]".into())), ("suit", ParamValue::Str("pool".into())),
        ("suit_pool", ParamValue::Str("[]".into())),
    ]);
    assert!(!empty.contains("pseudorandom_element("), "Empty pools should fall back to normal random rank/suit: {empty}");
    assert!(!empty.contains("rank =") && !empty.contains("suit ="), "{empty}");
}

#[test]
fn create_playing_card_static_properties_remain_editable_in_live_preview() {
    let joker = preview_test_joker(serde_json::json!([{
        "id": "rule", "trigger": "hand_played", "effects": [{
            "id": "create", "effect_type": "create_playing_card", "params": {
                "rank": "8", "suit": "Hearts", "enhancement": "m_lucky", "seal": "Blue", "edition": "e_foil"
            }
        }]
    }]));
    let (code, _, bindings) = Emitter::new().emit_chunk_with_field_bindings(&compile_joker(&joker, "mod"));
    for (key, value) in [("rank", "8"), ("suit", "Hearts"), ("enhancement", "m_lucky"), ("seal", "Blue"), ("edition", "e_foil")] {
        let path = serde_json::json!(["rules", 0, "effects", 0, "params", key, "value"]);
        let binding = bindings.iter().find(|binding| serde_json::json!(binding.source_path) == path)
            .unwrap_or_else(|| panic!("Missing editable {key} property in {code}"));
        assert_eq!(binding.original_value, serde_json::json!(value));
        assert_eq!(field_binding_text(&code, binding), format!("'{value}'"));
    }
}

#[test]
fn rule_chain_places_conditional_before_unconditional_fallback() {
    let fallback = make_rule_output("r_fallback", "hand_played", None, "FALLBACK");
    let conditional = make_rule_output(
        "r_cond",
        "hand_played",
        Some(lua_path(&["context", "cond_a"])),
        "COND",
    );

    let rules = vec![&fallback, &conditional];
    let mut out = Vec::new();
    append_rule_chain_with_fallback(&mut out, &rules, |ro| ro.effect_stmts.clone());

    let code = Emitter::new().emit_stmts(&out);
    assert!(code.contains("if context.cond_a then"));
    assert!(code.contains("else"));
    assert!(index_of(&code, "message = 'COND'") < index_of(&code, "message = 'FALLBACK'"));
}

#[test]
fn rule_chain_keeps_conditional_order_then_fallback() {
    let fallback = make_rule_output("r_fallback", "hand_played", None, "FALLBACK");
    let cond_1 = make_rule_output(
        "r_cond_1",
        "hand_played",
        Some(lua_path(&["context", "cond_1"])),
        "COND_1",
    );
    let cond_2 = make_rule_output(
        "r_cond_2",
        "hand_played",
        Some(lua_path(&["context", "cond_2"])),
        "COND_2",
    );

    let rules = vec![&fallback, &cond_1, &cond_2];
    let mut out = Vec::new();
    append_rule_chain_with_fallback(&mut out, &rules, |ro| ro.effect_stmts.clone());

    let code = Emitter::new().emit_stmts(&out);
    let cond_1_idx = index_of(&code, "message = 'COND_1'");
    let cond_2_idx = index_of(&code, "message = 'COND_2'");
    let fallback_idx = index_of(&code, "message = 'FALLBACK'");
    assert!(cond_1_idx < cond_2_idx);
    assert!(cond_2_idx < fallback_idx);
}

#[test]
fn rule_chain_moves_all_unconditional_rules_to_fallback_tail() {
    let fallback_1 = make_rule_output("r_fallback_1", "hand_played", None, "FALLBACK_1");
    let cond = make_rule_output(
        "r_cond",
        "hand_played",
        Some(lua_path(&["context", "cond_a"])),
        "COND",
    );
    let fallback_2 = make_rule_output("r_fallback_2", "hand_played", None, "FALLBACK_2");

    let rules = vec![&fallback_1, &cond, &fallback_2];
    let mut out = Vec::new();
    append_rule_chain_with_fallback(&mut out, &rules, |ro| ro.effect_stmts.clone());

    let code = Emitter::new().emit_stmts(&out);
    let cond_idx = index_of(&code, "message = 'COND'");
    let fb_1_idx = index_of(&code, "message = 'FALLBACK_1'");
    let fb_2_idx = index_of(&code, "message = 'FALLBACK_2'");
    assert!(cond_idx < fb_1_idx);
    assert!(fb_1_idx < fb_2_idx);
}

#[test]
fn rule_chain_preserves_order_when_all_rules_unconditional() {
    let first = make_rule_output("r_first", "hand_played", None, "FIRST");
    let second = make_rule_output("r_second", "hand_played", None, "SECOND");

    let rules = vec![&first, &second];
    let mut out = Vec::new();
    append_rule_chain_with_fallback(&mut out, &rules, |ro| ro.effect_stmts.clone());

    let code = Emitter::new().emit_stmts(&out);
    assert!(!code.contains("if context."));
    assert!(index_of(&code, "message = 'FIRST'") < index_of(&code, "message = 'SECOND'"));
}

#[test]
fn rule_chain_with_only_conditionals_has_no_unconditional_fallback() {
    let cond_1 = make_rule_output(
        "r_cond_1",
        "hand_played",
        Some(lua_path(&["context", "cond_1"])),
        "COND_1",
    );
    let cond_2 = make_rule_output(
        "r_cond_2",
        "hand_played",
        Some(lua_path(&["context", "cond_2"])),
        "COND_2",
    );

    let rules = vec![&cond_1, &cond_2];
    let mut out = Vec::new();
    append_rule_chain_with_fallback(&mut out, &rules, |ro| ro.effect_stmts.clone());

    let code = Emitter::new().emit_stmts(&out);
    assert!(code.contains("if context.cond_1 then"));
    assert!(code.contains("if context.cond_2 then"));
    assert!(!code.contains("FALLBACK"));
}

#[test]
fn shared_calculate_groups_same_trigger_with_fallback_order() {
    let ctx = CompileContext::new(
        ObjectType::Consumable,
        "mod".to_string(),
        "c_test".to_string(),
        false,
    );

    let conditional = make_rule_output(
        "r_cond",
        "hand_played",
        Some(lua_path(&["context", "cond_a"])),
        "COND",
    );
    let fallback = make_rule_output("r_fallback", "hand_played", None, "FALLBACK");

    let calc = build_shared_calculate_function(&[fallback, conditional], &ctx)
        .expect("calculate function should be generated");
    let code = Emitter::new().emit_expr_to_string(&calc);

    assert!(code.contains("if context."));
    assert!(index_of(&code, "message = 'COND'") < index_of(&code, "message = 'FALLBACK'"));
}

#[test]
fn shared_calculate_ignores_card_used_trigger_rules() {
    let ctx = CompileContext::new(
        ObjectType::Consumable,
        "mod".to_string(),
        "c_test".to_string(),
        false,
    );

    let card_used = make_rule_output("r_use", "card_used", None, "USE_ONLY");
    let calc_rule = make_rule_output("r_calc", "hand_played", None, "CALC_ONLY");

    let calc = build_shared_calculate_function(&[card_used, calc_rule], &ctx)
        .expect("calculate function should be generated");
    let code = Emitter::new().emit_expr_to_string(&calc);

    assert!(code.contains("CALC_ONLY"));
    assert!(!code.contains("USE_ONLY"));
}

#[test]
fn joker_calculate_wraps_rules_and_keeps_fallback_last_for_same_trigger() {
    let ctx = CompileContext::new(
        ObjectType::Joker,
        "mod".to_string(),
        "j_test".to_string(),
        false,
    );

    let fallback = make_rule_output("rule_fallback", "first_hand_drawn", None, "FALLBACK");
    let conditional = make_rule_output(
        "rule_cond",
        "first_hand_drawn",
        Some(lua_path(&["context", "cond_a"])),
        "COND",
    );

    let calc = build_calculate_function(&[fallback, conditional], &ctx)
        .expect("calculate function should be generated");
    let code = Emitter::new().emit_expr_to_string(&calc);

    assert!(index_of(&code, "message = 'COND'") < index_of(&code, "message = 'FALLBACK'"));
}

#[test]
fn shared_calculate_keeps_triggers_isolated() {
    let ctx = CompileContext::new(
        ObjectType::Deck,
        "mod".to_string(),
        "b_test".to_string(),
        false,
    );

    let hand_rule = make_rule_output("r_hand", "hand_played", None, "HAND");
    let round_rule = make_rule_output("r_round", "round_end", None, "ROUND");

    let calc = build_shared_calculate_function(&[hand_rule, round_rule], &ctx)
        .expect("calculate function should be generated");
    let code = Emitter::new().emit_expr_to_string(&calc);

    assert!(code.contains("message = 'HAND'"));
    assert!(code.contains("message = 'ROUND'"));
    let trigger_if_count = code.matches("if context.").count();
    assert!(
        trigger_if_count >= 2,
        "expected at least two trigger-scoped if blocks, got {trigger_if_count} in:\n{code}"
    );
    assert!(index_of(&code, "message = 'HAND'") < index_of(&code, "message = 'ROUND'"));
}

#[test]
fn joker_calculate_uses_elseif_for_multiple_triggers() {
    let ctx = CompileContext::new(
        ObjectType::Joker,
        "mod".to_string(),
        "j_test".to_string(),
        false,
    );

    let hand_rule = make_rule_output("r_hand", "hand_played", None, "HAND");
    let round_rule = make_rule_output("r_round", "round_end", None, "ROUND");

    let calc = build_calculate_function(&[hand_rule, round_rule], &ctx)
        .expect("calculate function should be generated");
    let code = Emitter::new().emit_expr_to_string(&calc);

    assert!(code.contains("message = 'HAND'"));
    assert!(code.contains("message = 'ROUND'"));
    let trigger_if_count = code.matches("if context.").count();
    assert!(
        trigger_if_count >= 2,
        "expected at least two trigger-scoped if blocks, got {trigger_if_count} in:\n{code}"
    );
    assert!(index_of(&code, "message = 'HAND'") < index_of(&code, "message = 'ROUND'"));
}

#[test]
fn trigger_segment_starts_on_trigger_if_line() {
    let ctx = CompileContext::new(
        ObjectType::Joker,
        "mod".to_string(),
        "j_test".to_string(),
        false,
    );

    let hand_rule = make_rule_output("r_hand", "hand_played", None, "HAND");
    let round_rule = make_rule_output("r_round", "round_end", None, "ROUND");

    let calc = build_calculate_function(&[hand_rule, round_rule], &ctx)
        .expect("calculate function should be generated");
    let chunk = Chunk {
        stmts: vec![Stmt::Local("calculate".to_string(), Some(calc))],
    };
    let (code, segments) = Emitter::new().emit_chunk_with_segments(&chunk);
    for segment_id in ["trigger:r_hand", "trigger:r_round"] {
        let seg = segments
            .iter()
            .find(|segment| segment.id == segment_id)
            .unwrap_or_else(|| panic!("missing segment {}", segment_id));
        let seg_line = code
            .lines()
            .nth(seg.start_line.saturating_sub(1))
            .unwrap_or_else(|| panic!("segment {segment_id} start line out of bounds"));
        assert!(
            seg_line.trim_start().starts_with("if context."),
            "segment {segment_id} should start on trigger if line, got: {}",
            seg_line
        );
    }
}
