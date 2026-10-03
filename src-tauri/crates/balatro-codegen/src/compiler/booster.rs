use super::colors::normalize_hex_colour;
use crate::lua_ast::*;
use crate::types::*;

pub fn compile_booster(booster: &BoosterDef, mod_prefix: &str) -> Chunk {
    compile_booster_with_options(booster, mod_prefix, true)
}

pub fn compile_booster_with_options(
    booster: &BoosterDef,
    mod_prefix: &str,
    include_loc_txt: bool,
) -> Chunk {
    Chunk {
        stmts: vec![
            lua_comment(format!(" {}", booster.name)),
            Stmt::ExprStmt(lua_table_call(
                lua_path(&["SMODS", "Booster"]),
                build_booster_table(booster, mod_prefix, include_loc_txt),
            )),
        ],
    }
}

fn build_booster_table(
    booster: &BoosterDef,
    mod_prefix: &str,
    include_loc_txt: bool,
) -> Vec<TableEntry> {
    let extra = booster.extra.unwrap_or(3).max(1);
    let choose = booster.choose.or(booster.draw).unwrap_or(1).clamp(1, extra);
    let key = local_booster_key(&booster.key, mod_prefix);
    let mut entries = vec![
        kv("key", lua_str(key)),
        kv("atlas", lua_str(&booster.atlas)),
        kv(
            "pos",
            lua_table(vec![
                ("x", lua_int(booster.pos.x as i64)),
                ("y", lua_int(booster.pos.y as i64)),
            ]),
        ),
        kv(
            "config",
            lua_table(vec![
                ("extra", lua_int(extra as i64)),
                ("choose", lua_int(choose as i64)),
            ]),
        ),
    ];
    if include_loc_txt {
        let text = booster
            .description
            .iter()
            .map(|line| TableEntry::Value(lua_str(line)))
            .collect();
        entries.push(kv(
            "loc_txt",
            lua_table(vec![
                ("name", lua_str(&booster.name)),
                ("text", lua_table_raw(text)),
                (
                    "group_name",
                    lua_str(nonempty(&booster.group_key).unwrap_or(&booster.name)),
                ),
            ]),
        ));
    }
    if let Some(cost) = booster.cost {
        entries.push(kv("cost", lua_int(cost.max(0) as i64)));
    }
    if let Some(weight) = booster.weight.filter(|weight| weight.is_finite()) {
        entries.push(kv("weight", lua_num(weight.max(0.0))));
    }
    if let Some(kind) = nonempty(&booster.kind) {
        entries.push(kv("kind", lua_str(kind)));
    }
    for (key, value) in [
        ("draw_hand", booster.draw_hand),
        ("unlocked", booster.unlocked),
        ("discovered", booster.discovered),
        ("no_collection", booster.hidden),
    ] {
        if let Some(value) = value {
            entries.push(kv(key, lua_bool(value)));
        }
    }
    if booster.instant_use == Some(false) && has_consumable_content(booster) {
        let mut sets = std::collections::BTreeSet::from([
            "Tarot".to_string(),
            "Planet".to_string(),
            "Spectral".to_string(),
        ]);
        for rule in &booster.card_rules {
            if rule_booster_type(booster, rule) == "consumable" {
                if let Some(set) = selected(&rule.set) {
                    sets.insert(set.to_string());
                }
            }
        }
        entries.push(kv(
            "select_card",
            lua_table_raw(
                sets.into_iter()
                    .map(|set| TableEntry::IndexValue(lua_str(set), lua_str("consumeables")))
                    .collect(),
            ),
        ));
    }
    entries.push(kv("create_card", create_card_function(booster, mod_prefix)));
    if let Some(callback) = background_function(booster) {
        entries.push(kv("ease_background_colour", callback));
    }
    entries
}

fn local_booster_key<'a>(key: &'a str, mod_prefix: &str) -> &'a str {
    let key = key.strip_prefix("p_").unwrap_or(key);
    key.strip_prefix(&format!("{mod_prefix}_")).unwrap_or(key)
}

fn has_consumable_content(booster: &BoosterDef) -> bool {
    booster.booster_type == "consumable"
        || booster.card_rules.iter().any(|rule| {
            nonempty(&rule.specific_type) == Some("consumable")
                && selected(&rule.specific_key).is_some()
        })
}

fn rule_booster_type<'a>(booster: &'a BoosterDef, rule: &'a BoosterCardRuleDef) -> &'a str {
    if selected(&rule.specific_key).is_some() {
        nonempty(&rule.specific_type).unwrap_or(&booster.booster_type)
    } else {
        &booster.booster_type
    }
}

fn create_card_function(booster: &BoosterDef, mod_prefix: &str) -> Expr {
    let rules: Vec<_> = booster
        .card_rules
        .iter()
        .filter(|rule| rule.weight.is_finite() && rule.weight > 0.0)
        .collect();
    let mut body = Vec::new();
    if rules.len() > 1 {
        // Scale first to avoid overflowing when very large finite weights are
        // imported. Zero-weight rules never receive a selection interval.
        let max_weight = rules.iter().map(|rule| rule.weight).fold(0.0, f64::max);
        let total_weight: f64 = rules.iter().map(|rule| rule.weight / max_weight).sum();
        body.push(lua_local(
            "roll",
            lua_mul(
                lua_call(
                    "pseudorandom",
                    vec![lua_str(format!("{mod_prefix}_{}_card", booster.key))],
                ),
                lua_num(total_weight),
            ),
        ));
        let mut cumulative = 0.0;
        let mut branches = Vec::new();
        for rule in &rules[..rules.len() - 1] {
            cumulative += rule.weight / max_weight;
            branches.push((
                lua_lt(lua_ident("roll"), lua_num(cumulative)),
                vec![lua_return(card_config(booster, rule, mod_prefix))],
            ));
        }
        body.push(Stmt::If {
            branches,
            else_body: Some(vec![lua_return(card_config(
                booster,
                rules.last().unwrap(),
                mod_prefix,
            ))]),
        });
    } else if let Some(rule) = rules.first() {
        body.push(lua_return(card_config(booster, rule, mod_prefix)));
    } else {
        body.push(lua_return(card_config(
            booster,
            &BoosterCardRuleDef::default(),
            mod_prefix,
        )));
    }
    Expr::Function {
        params: vec!["self".into(), "card".into(), "i".into()],
        body,
    }
}

fn card_config(booster: &BoosterDef, rule: &BoosterCardRuleDef, mod_prefix: &str) -> Expr {
    let specific_key = selected(&rule.specific_key);
    let booster_type = rule_booster_type(booster, rule);
    let set = match booster_type {
        "consumable" => selected(&rule.set).unwrap_or("Tarot").to_string(),
        "playing_card" => {
            if matches!(nonempty(&rule.enhancement), Some("none" | "base")) {
                "Base".to_string()
            } else {
                "Playing Card".to_string()
            }
        }
        "voucher" => "Voucher".to_string(),
        // desktop prefixes its Joker ObjectType pools, w hile
        // ConsumableType keys above are registered without a prefix.
        _ => selected(&rule.pool)
            .map(|pool| prefixed_key(pool, mod_prefix))
            .unwrap_or_else(|| "Joker".to_string()),
    };
    let seed = format!("{mod_prefix}_{}", booster.key);
    let mut entries = vec![
        kv("set", lua_str(set)),
        kv("area", lua_path(&["G", "pack_cards"])),
        kv("skip_materialize", lua_bool(true)),
        kv("soulable", lua_bool(true)),
        kv("key_append", lua_str(&seed)),
    ];
    if let Some(key) = specific_key {
        let class = match booster_type {
            "consumable" => "c",
            "voucher" => "v",
            "playing_card" => "m",
            _ => "j",
        };
        entries.push(kv("key", lua_str(class_key(key, class, mod_prefix))));
    }
    if booster_type == "joker" {
        if let Some(rarity) = selected(&rule.rarity) {
            entries.push(kv("rarity", lua_str(rarity_key(rarity, mod_prefix))));
        }
    }
    if booster_type == "playing_card" {
        if let Some(suit) = selected(&rule.suit) {
            entries.push(kv("suit", lua_str(suit)));
        }
        if let Some(rank) = selected(&rule.rank) {
            entries.push(kv(
                "rank",
                lua_str(match rank {
                    "J" => "Jack",
                    "Q" => "Queen",
                    "K" => "King",
                    "A" => "Ace",
                    _ => rank,
                }),
            ));
        }
        if let Some(enhancement) = property_selected(&rule.enhancement) {
            entries.push(kv(
                "enhancement",
                property_value(enhancement, "enhancement", mod_prefix, &seed),
            ));
        }
    }
    if matches!(nonempty(&rule.edition), Some("none" | "base")) {
        entries.push(kv("no_edition", lua_bool(true)));
    } else if let Some(edition) = property_selected(&rule.edition) {
        entries.push(kv(
            "edition",
            property_value(edition, "edition", mod_prefix, &seed),
        ));
    }
    if let Some(seal) = property_selected(&rule.seal) {
        entries.push(kv("seal", property_value(seal, "seal", mod_prefix, &seed)));
    }
    lua_table_raw(entries)
}

fn property_value(value: &str, property: &str, mod_prefix: &str, seed: &str) -> Expr {
    if value == "random" {
        return lua_call(
            format!("SMODS.poll_{property}"),
            vec![lua_table(vec![
                ("key", lua_str(format!("{seed}_{property}"))),
                ("guaranteed", lua_bool(true)),
            ])],
        );
    }
    let key = match property {
        "edition" if matches!(value, "foil" | "holo" | "polychrome" | "negative") => {
            format!("e_{value}")
        }
        "edition" => class_key(value, "e", mod_prefix),
        "enhancement"
            if matches!(
                value,
                "bonus" | "mult" | "wild" | "glass" | "steel" | "stone" | "gold" | "lucky"
            ) =>
        {
            format!("m_{value}")
        }
        "enhancement" => class_key(value, "m", mod_prefix),
        "seal" => match value {
            "gold" | "Gold" => "Gold".into(),
            "red" | "Red" => "Red".into(),
            "blue" | "Blue" => "Blue".into(),
            "purple" | "Purple" => "Purple".into(),
            _ => prefixed_key(value, mod_prefix),
        },
        _ => value.into(),
    };
    lua_str(key)
}

fn class_key(value: &str, class: &str, mod_prefix: &str) -> String {
    if value.starts_with(&format!("{class}_")) {
        value.into()
    } else {
        format!("{class}_{}", prefixed_key(value, mod_prefix))
    }
}

fn prefixed_key(value: &str, mod_prefix: &str) -> String {
    if mod_prefix.is_empty() || value.starts_with(&format!("{mod_prefix}_")) {
        value.into()
    } else {
        format!("{mod_prefix}_{value}")
    }
}

fn rarity_key(value: &str, mod_prefix: &str) -> String {
    match value.to_ascii_lowercase().as_str() {
        "1" | "common" => "Common".into(),
        "2" | "uncommon" => "Uncommon".into(),
        "3" | "rare" => "Rare".into(),
        "4" | "legendary" => "Legendary".into(),
        _ => prefixed_key(value, mod_prefix),
    }
}

fn background_function(booster: &BoosterDef) -> Option<Expr> {
    let background = nonempty(&booster.background_colour).and_then(normalize_hex_colour);
    let special = nonempty(&booster.special_colour).and_then(normalize_hex_colour);
    if background.is_none() && special.is_none() {
        return None;
    }
    let main = background
        .map(|hex| lua_call("HEX", vec![lua_str(hex)]))
        .unwrap_or_else(|| lua_path(&["G", "C", "FILTER"]));
    let special = special
        .map(|hex| lua_call("HEX", vec![lua_str(hex)]))
        .unwrap_or_else(|| lua_path(&["G", "C", "BLACK"]));
    Some(Expr::Function {
        params: vec!["self".into()],
        body: vec![
            lua_expr_stmt(lua_call(
                "ease_colour",
                vec![lua_path(&["G", "C", "DYN_UI", "MAIN"]), main.clone()],
            )),
            lua_expr_stmt(lua_call(
                "ease_background_colour",
                vec![lua_table(vec![
                    ("new_colour", main),
                    ("special_colour", special),
                    ("contrast", lua_int(2)),
                ])],
            )),
        ],
    })
}

fn nonempty(value: &Option<String>) -> Option<&str> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn selected(value: &Option<String>) -> Option<&str> {
    property_selected(value).filter(|value| *value != "random")
}

fn property_selected(value: &Option<String>) -> Option<&str> {
    nonempty(value).filter(|value| !matches!(*value, "any" | "none" | "base"))
}

fn kv(key: &str, value: Expr) -> TableEntry {
    TableEntry::KeyValue(key.into(), value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pack(mut fields: serde_json::Value) -> BoosterDef {
        let mut value = json!({
            "key": "test_pack",
            "name": "Test Pack",
            "description": ["Choose #1# of #2# cards"],
            "atlas": "custom_boosters",
            "pos": {"x": 2, "y": 1}
        });
        value
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object_mut().unwrap().clone());
        serde_json::from_value(value).unwrap()
    }

    fn emitted(fields: serde_json::Value) -> String {
        Emitter::new().emit_chunk(&compile_booster(&pack(fields), "forge"))
    }

    #[test]
    fn all_pack_types_create_pack_cards_with_numeric_size_and_choices() {
        for (kind, set) in [
            ("joker", "Joker"),
            ("consumable", "Tarot"),
            ("playing_card", "Playing Card"),
            ("voucher", "Voucher"),
        ] {
            let lua =
                emitted(json!({"booster_type": kind, "extra": 5, "choose": 2, "hidden": true}));
            assert!(lua.contains("extra = 5"));
            assert!(lua.contains("choose = 2"));
            assert!(!lua.contains("extra = {"));
            assert!(lua.contains(&format!("set = '{set}'")));
            assert!(lua.contains("area = G.pack_cards"));
            assert!(lua.contains("skip_materialize = true"));
            assert!(lua.contains("no_collection = true"));
            assert!(!lua.contains("SMODS.add_card"));
            assert!(!lua.contains("add_to_deck"));
            assert!(!lua.contains("loc_vars ="));
        }
    }

    #[test]
    fn playing_card_content_honors_rank_suit_and_modifications() {
        let lua = emitted(json!({
            "booster_type": "playing_card",
            "draw_hand": true,
            "card_rules": [{"rank": "A", "suit": "Hearts", "enhancement": "glass", "edition": "none", "seal": "purple"}]
        }));
        assert!(lua.contains("rank = 'Ace'"));
        assert!(lua.contains("suit = 'Hearts'"));
        assert!(lua.contains("enhancement = 'm_glass'"));
        assert!(lua.contains("no_edition = true"));
        assert!(lua.contains("seal = 'Purple'"));
        assert!(lua.contains("draw_hand = true"));

        let lua = emitted(json!({
            "booster_type": "playing_card",
            "card_rules": [{"enhancement": "none", "edition": "random", "seal": "random"}]
        }));
        assert!(lua.contains("set = 'Base'"));
        assert!(lua.contains("edition = SMODS.poll_edition"));
        assert!(lua.contains("seal = SMODS.poll_seal"));
        assert!(lua.contains("guaranteed = true"));
        assert!(!lua.contains("no_edition"));
    }

    #[test]
    fn weighted_rules_exclude_disabled_entries_and_always_return_a_card() {
        let lua = emitted(json!({
            "card_rules": [
                {"weight": 3, "specific_key": "first"},
                {"weight": 0, "specific_key": "disabled"},
                {"weight": 1, "specific_key": "j_foreign_last"}
            ]
        }));
        assert!(
            lua.contains("local roll = pseudorandom('forge_test_pack_card') * 1.3333333333333333")
        );
        assert!(lua.contains("if roll < 1 then"));
        assert!(lua.contains("key = 'j_forge_first'"));
        assert!(lua.contains("else\n"));
        assert!(lua.contains("key = 'j_foreign_last'"));
        assert!(!lua.contains("disabled"));

        let lua = emitted(json!({"card_rules": [{"weight": 0, "specific_key": "disabled"}]}));
        assert!(lua.contains("set = 'Joker'"));
        assert!(!lua.contains("disabled"));
    }

    #[test]
    fn custom_keys_sets_pools_and_mixed_specific_types_match_registered_objects() {
        let lua = emitted(json!({
            "booster_type": "joker",
            "instant_use": false,
            "card_rules": [
                {"specific_type": "consumable", "specific_key": "moon", "set": "CustomSet", "edition": "custom_edition"},
                {"pool": "CustomPool", "rarity": "custom_rarity"},
                {"specific_type": "voucher", "specific_key": "v_forge_discount"}
            ]
        }));
        assert!(lua.contains("set = 'CustomSet'"));
        assert!(lua.contains("key = 'c_forge_moon'"));
        assert!(lua.contains("edition = 'e_forge_custom_edition'"));
        assert!(lua.contains("set = 'forge_CustomPool'"));
        assert!(lua.contains("rarity = 'forge_custom_rarity'"));
        assert!(lua.contains("set = 'Voucher'"));
        assert!(lua.contains("key = 'v_forge_discount'"));
        assert!(lua.contains("select_card = {"));
        assert!(lua.contains("['CustomSet'] = 'consumeables'"));
        assert!(lua.contains("['Planet'] = 'consumeables'"));
        assert!(!lua.contains("['Joker'] = 'consumeables'"));
        assert!(!lua.contains("forge_CustomSet"));
        assert!(!lua.contains("forge_forge_CustomPool"));
    }

    #[test]
    fn rarity_and_canonical_card_modifications_do_not_gain_duplicate_prefixes() {
        let lua = emitted(json!({"card_rules": [{"rarity": "3", "edition": "e_forge_glowing"}]}));
        assert!(lua.contains("rarity = 'Rare'"));
        assert!(lua.contains("edition = 'e_forge_glowing'"));
        let lua = emitted(json!({
            "booster_type": "playing_card",
            "card_rules": [{"enhancement": "m_forge_shiny", "seal": "forge_flower"}]
        }));
        assert!(lua.contains("enhancement = 'm_forge_shiny'"));
        assert!(lua.contains("seal = 'forge_flower'"));
    }

    #[test]
    fn pack_localization_and_registration_keys_match_prefixed_or_bare_inputs() {
        for key in [
            "test_pack",
            "p_test_pack",
            "forge_test_pack",
            "p_forge_test_pack",
        ] {
            let booster = pack(json!({"key": key, "group_key": "Custom Group"}));
            let lua = Emitter::new().emit_chunk(&compile_booster(&booster, "forge"));
            assert!(lua.contains("key = 'test_pack'"));
            assert!(lua.contains("group_name = 'Custom Group'"));
            assert!(!lua.contains("group_key ="));
            let lua =
                Emitter::new().emit_chunk(&compile_booster_with_options(&booster, "forge", false));
            assert!(!lua.contains("loc_txt"));
            assert!(lua.contains("create_card = function"));
        }
    }

    #[test]
    fn both_background_colors_work_independently_and_invalid_values_are_ignored() {
        let lua = emitted(json!({"special_colour": "#123456"}));
        assert!(lua.contains("ease_background_colour = function(self)"));
        assert!(lua.contains("new_colour = G.C.FILTER"));
        assert!(lua.contains("special_colour = HEX('123456')"));
        let lua = emitted(json!({"background_colour": "#abcDEF", "special_colour": "bad lua"}));
        assert!(lua.contains("HEX('ABCDEF')"));
        assert!(lua.contains("special_colour = G.C.BLACK"));
        let lua = emitted(json!({"background_colour": "invalid", "special_colour": "bad lua"}));
        assert!(!lua.contains("ease_background_colour"));
    }
}
