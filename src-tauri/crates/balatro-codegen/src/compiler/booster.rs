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
