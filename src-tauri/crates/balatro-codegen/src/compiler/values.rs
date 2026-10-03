use crate::lua_ast::*;
use crate::types::{ObjectType, ParamValue};

pub fn is_user_variable_type(value_type: &str) -> bool {
    matches!(value_type, "userVariable" | "user_var")
}

pub fn is_game_variable_type(value_type: &str) -> bool {
    matches!(value_type, "gameVariable" | "game_var")
}

pub fn is_range_type(value_type: &str) -> bool {
    matches!(value_type, "range" | "range_var")
}

/// Parsed game variable reference.
/// Format in params: `"GAMEVAR:varId|multiplier|startsFrom"`
pub struct GameVarRef {
    pub var_id: String,
    pub multiplier: f64,
    pub starts_from: f64,
}

/// Parsed range variable reference.
/// Format in params: `"RANGE:min|max"`
pub struct RangeRef {
    pub min: f64,
    pub max: f64,
}

/// Parse a `GAMEVAR:id|mult|start` string.
pub fn parse_game_var(s: &str) -> Option<GameVarRef> {
    let rest = s.strip_prefix("GAMEVAR:")?;
    let parts: Vec<&str> = rest.split('|').collect();
    if parts.len() != 3 || parts[0].is_empty() {
        return None;
    }
    let multiplier = parts[1].parse::<f64>().ok()?;
    let starts_from = parts[2].parse::<f64>().ok()?;
    if !multiplier.is_finite() || !starts_from.is_finite() {
        return None;
    }
    Some(GameVarRef {
        var_id: parts[0].to_string(),
        multiplier,
        starts_from,
    })
}

/// Parse a `RANGE:min|max` string.
pub fn parse_range_var(s: &str) -> Option<RangeRef> {
    let rest = s.strip_prefix("RANGE:")?;
    let parts: Vec<&str> = rest.split('|').collect();
    if parts.len() < 2 {
        return None;
    }
    Some(RangeRef {
        min: parts[0].parse().unwrap_or(0.0),
        max: parts[1].parse().unwrap_or(0.0),
    })
}

/// Build a guarded path from compiler-owned field names, never project input.
fn guarded_path(root: &str, fields: &[&str], fallback: &str) -> String {
    let mut path = root.to_string();
    let mut checks = vec![path.clone()];
    for field in fields {
        path.push('.');
        path.push_str(field);
        checks.push(path.clone());
    }
    format!("({} or {fallback})", checks.join(" and "))
}

fn game_field(path: &str) -> String {
    guarded_path("G", &path.split('.').collect::<Vec<_>>(), "0")
}

fn area_cards(area: &str) -> String {
    guarded_path("G", &[area, "cards"], "{}")
}

fn playing_cards() -> String {
    guarded_path("G", &["playing_cards"], "{}")
}

fn count_cards(cards: &str, predicate: &str) -> String {
    format!("(function() local count = 0; for _, card in ipairs({cards}) do if {predicate} then count = count + 1 end end; return count end)()")
}

fn poker_hand_value(key: &str, field: &str, guard: &str) -> String {
    format!("(G and G.GAME and G.GAME.hands and {guard} and G.GAME.hands[{key}] and G.GAME.hands[{key}].{field} or 0)")
}

fn profile_value(path: &str, fallback: &str) -> String {
    let value = guarded_path("profile", &path.split('.').collect::<Vec<_>>(), fallback);
    format!("(function() local profile = G and G.PROFILES and G.SETTINGS and G.PROFILES[G.SETTINGS.profile]; return {value} end)()")
}

fn profile_percentage(tally: &str, total: &str) -> String {
    let tally = profile_value(tally, "0");
    let total = profile_value(total, "0");
    format!("(function() local tally = {tally}; local total = {total}; if total <= 0 then return 0 end; return math.floor(0.01 + 100 * tally / total) end)()")
}

fn profile_usage_count(usage: &str, rank: usize) -> String {
    let usage = guarded_path("profile", &[usage], "{}");
    format!("(function() local profile = G and G.PROFILES and G.SETTINGS and G.PROFILES[G.SETTINGS.profile]; local counts = {{}}; for key, data in pairs({usage}) do if G and G.P_CENTERS and G.P_CENTERS[key] then counts[#counts + 1] = data.count or 0 end end; table.sort(counts, function(a, b) return a > b end); return counts[{rank}] or 0 end)()")
}

/// Editor game variable IDs and saved-project aliases → numeric Lua expressions.
/// Every expression also works when its game or trigger-specific state is absent.
/// Keep this catalog in sync with `src/lib/content/game-vars.ts` (tested below).
pub fn game_var_lua_code(var_id: &str) -> Option<String> {
    let direct = match var_id {
        "cards_in_deck" | "deck_size" => Some(format!("#{}", area_cards("deck"))),
        "total_playing_cards" | "full_deck_size" => Some(format!("#{}", playing_cards())),
        "cards_in_hand" | "hand_size" => Some(format!("#{}", area_cards("hand"))),
        "cards_in_discard" => Some(format!("#{}", area_cards("discard"))),
        "current_hand_size" => Some(game_field("hand.config.card_limit")),
        "cards_removed_from_deck" => Some(format!("(G and G.GAME and G.GAME.starting_deck_size and (G.GAME.starting_deck_size - #{}) or 0)", playing_cards())),
        "lowest_rank_in_hand" | "lowest_card_mult" => Some(format!("(function() local min = 14; for _, card in ipairs({}) do if card.base and card.base.id and card.base.id < min then min = card.base.id end end; return min end)()", area_cards("hand"))),
        "highest_rank_in_hand" | "highest_card_mult" => Some(format!("(function() local max = 0; for _, card in ipairs({}) do if card.base and card.base.id and card.base.id > max then max = card.base.id end end; return max end)()", area_cards("hand"))),
        "blind_chip_req" | "blind_chips" => Some(game_field("GAME.blind.chips")),
        "blind_mult" => Some(game_field("GAME.blind.mult")),
        "current_chip_total" => Some(game_field("GAME.chips")),
        "initial_scoring_chips" => Some(game_field("GAME.current_round.current_hand.chips")),
        "initial_scoring_mult" => Some(game_field("GAME.current_round.current_hand.mult")),
        "current_scoring_chips" => Some("(hand_chips or 0)".to_string()),
        "current_scoring_mult" => Some("(mult or 0)".to_string()),
        "current_seconds" | "current_minutes" | "current_hours" | "current_day"
        | "current_month" | "current_year" | "current_year_day" | "current_week_day" => {
            let field = match var_id {
                "current_seconds" => "sec",
                "current_minutes" => "min",
                "current_hours" => "hour",
                "current_day" => "day",
                "current_month" => "month",
                "current_year" => "year",
                "current_year_day" => "yday",
                _ => "wday",
            };
            Some(format!("(function() local date = os and os.date and os.date('*t'); return date and date.{field} or 0 end)()"))
        }
        "cumulative_chips" => Some("(function() local total = 0; for _, card in pairs(context and context.scoring_hand or {}) do total = total + (card.base and card.base.nominal or 0) end; return total end)()".to_string()),
        "scored_card_count" => Some("#(context and context.scoring_hand or {})".to_string()),
        "played_card_count" => Some("#(context and context.full_hand or {})".to_string()),
        "consumables_held" | "consumable_count" => Some(format!("#{}", area_cards("consumeables"))),
        "consumable_slots" => Some(game_field("consumeables.config.card_limit")),
        "free_consumable_slots" => Some(format!("({} - #{})", game_field("consumeables.config.card_limit"), area_cards("consumeables"))),
        "tarot_cards_used" => Some(game_field("GAME.consumeable_usage_total.tarot")),
        "spectral_cards_used" => Some(game_field("GAME.consumeable_usage_total.spectral")),
        "planet_cards_used" => Some(game_field("GAME.consumeable_usage_total.planet")),
        "unique_tarots_used" | "unique_planets_used" | "unique_spectrals_used" => {
            let set = match var_id { "unique_tarots_used" => "Tarot", "unique_planets_used" => "Planet", _ => "Spectral" };
            Some(format!("(function() local count = 0; for _, data in pairs({}) do if data.set == '{set}' then count = count + 1 end end; return count end)()", guarded_path("G", &["GAME", "consumeable_usage"], "{}")))
        }
        "joker_count" => Some(format!("#{}", area_cards("jokers"))),
        "joker_slots" => Some(game_field("jokers.config.card_limit")),
        "free_joker_slots" => Some(format!("({} - #{})", game_field("jokers.config.card_limit"), area_cards("jokers"))),
        "common_jokers" | "uncommon_jokers" | "rare_jokers" | "legendary_jokers" => {
            let rarity = match var_id { "common_jokers" => 1, "uncommon_jokers" => 2, "rare_jokers" => 3, _ => 4 };
            Some(count_cards(&area_cards("jokers"), &format!("card.config and card.config.center and card.config.center.rarity == {rarity}")))
        }
        "self_sell_value" => Some("(card and card.sell_cost or 0)".to_string()),
        "other_jokers_sell_value" | "all_jokers_sell_value" => {
            let guard = if var_id == "other_jokers_sell_value" { "joker ~= card" } else { "true" };
            Some(format!("(function() local total = 0; for _, joker in ipairs({}) do if {guard} then total = total + (joker.sell_cost or 0) end end; return total end)()", area_cards("jokers")))
        }
        "hand_level" => Some(poker_hand_value("context.scoring_name", "level", "context and context.scoring_name")),
        "current_hand_played_count" | "times_hand_played" => Some(poker_hand_value("context.scoring_name", "played", "context and context.scoring_name")),
        "poker_hand_count" => Some(format!("(function() local count = 0; for _, hand in pairs({}) do if hand.visible then count = count + 1 end end; return count end)()", guarded_path("G", &["GAME", "hands"], "{}"))),
        "total_hand_levels" | "hand_levels_above_one" => {
            let subtract = if var_id == "hand_levels_above_one" { " - 1" } else { "" };
            Some(format!("(function() local total = 0; for _, data in pairs({}) do local level = data.level or 0; if level >= (to_big and to_big(1) or 1) then total = total + level{subtract} end end; return total end)()", guarded_path("G", &["GAME", "hands"], "{}")))
        }
        "most_played_hand_level" | "least_played_hand_level" => {
            let (initial, compare) = if var_id == "most_played_hand_level" { ("0", ">") } else { ("math.huge", "<") };
            Some(format!("(function() local played = {initial}; local level = 0; for _, data in pairs({}) do if (data.played or 0) {compare} played then played = data.played or 0; level = data.level or 0 end end; return level end)()", guarded_path("G", &["GAME", "hands"], "{}")))
        }
        "most_recent_hand_level" => Some(poker_hand_value("G.GAME.last_hand_played", "level", "G.GAME.last_hand_played")),
        "current_ante" | "ante_level" => Some(game_field("GAME.round_resets.ante")),
        "current_money" | "player_money" | "dollars" => Some(game_field("GAME.dollars")),
        "hands_remaining" | "remaining_hands" => Some(game_field("GAME.current_round.hands_left")),
        "discards_remaining" | "remaining_discards" => Some(game_field("GAME.current_round.discards_left")),
        "hands_played_this_round" => Some(game_field("GAME.current_round.hands_played")),
        "discards_used_this_round" => Some(game_field("GAME.current_round.discards_used")),
        "blinds_skipped" => Some(game_field("GAME.skips")),
        "base_hands_per_round" => Some(game_field("GAME.round_resets.hands")),
        "base_discards_per_round" => Some(game_field("GAME.round_resets.discards")),
        "interest" => Some(game_field("GAME.interest_amount")),
        "leftmost_card_rank" | "rightmost_card_rank" => {
            let index = if var_id == "leftmost_card_rank" { "1" } else { "#cards" };
            Some(format!("(function() local cards = {}; local first = cards[{index}]; return first and first.base and first.base.id or 0 end)()", area_cards("hand")))
        }
        _ => None,
    };
    if direct.is_some() {
        return direct;
    }

    // Composition variables use a finite allowlist of ranks, suits and modifiers.
    // A project-supplied prefix must never become raw Lua.
    if let Some((prefix, cards)) = var_id
        .strip_suffix("_in_deck")
        .map(|prefix| (prefix, playing_cards()))
        .or_else(|| {
            var_id
                .strip_suffix("_in_hand")
                .map(|prefix| (prefix, area_cards("hand")))
        })
    {
        let predicate = match prefix {
            "twos" => "card.base and card.base.id == 2",
            "threes" => "card.base and card.base.id == 3",
            "fours" => "card.base and card.base.id == 4",
            "fives" => "card.base and card.base.id == 5",
            "sixes" => "card.base and card.base.id == 6",
            "sevens" => "card.base and card.base.id == 7",
            "eights" => "card.base and card.base.id == 8",
            "nines" => "card.base and card.base.id == 9",
            "tens" => "card.base and card.base.id == 10",
            "jacks" => "card.base and card.base.id == 11",
            "queens" => "card.base and card.base.id == 12",
            "kings" => "card.base and card.base.id == 13",
            "aces" => "card.base and card.base.id == 14",
            "spades" => "card.base and card.base.suit == 'Spades'",
            "hearts" => "card.base and card.base.suit == 'Hearts'",
            "diamonds" => "card.base and card.base.suit == 'Diamonds'",
            "clubs" => "card.base and card.base.suit == 'Clubs'",
            "bonus_cards" => {
                "SMODS and SMODS.has_enhancement and SMODS.has_enhancement(card, 'm_bonus')"
            }
            "mult_cards" => {
                "SMODS and SMODS.has_enhancement and SMODS.has_enhancement(card, 'm_mult')"
            }
            "wild_cards" => {
                "SMODS and SMODS.has_enhancement and SMODS.has_enhancement(card, 'm_wild')"
            }
            "glass_cards" => {
                "SMODS and SMODS.has_enhancement and SMODS.has_enhancement(card, 'm_glass')"
            }
            "steel_cards" => {
                "SMODS and SMODS.has_enhancement and SMODS.has_enhancement(card, 'm_steel')"
            }
            "stone_cards" => {
                "SMODS and SMODS.has_enhancement and SMODS.has_enhancement(card, 'm_stone')"
            }
            "gold_cards" => {
                "SMODS and SMODS.has_enhancement and SMODS.has_enhancement(card, 'm_gold')"
            }
            "lucky_cards" => {
                "SMODS and SMODS.has_enhancement and SMODS.has_enhancement(card, 'm_lucky')"
            }
            "enhanced_cards" => {
                "SMODS and SMODS.get_enhancements and next(SMODS.get_enhancements(card) or {})"
            }
            "foil_cards" => "card.edition and card.edition.foil",
            "holographic_cards" => "card.edition and card.edition.holo",
            "polychrome_cards" => "card.edition and card.edition.polychrome",
            "negative_cards" => "card.edition and card.edition.negative",
            "gold_sealed_cards" => "card.seal == 'Gold'",
            "red_sealed_cards" => "card.seal == 'Red'",
            "blue_sealed_cards" => "card.seal == 'Blue'",
            "purple_sealed_cards" => "card.seal == 'Purple'",
            _ => return None,
        };
        return Some(count_cards(&cards, predicate));
    }

    if let Some((prefix, field)) = var_id
        .strip_suffix("_played")
        .map(|prefix| (prefix, "played"))
        .or_else(|| {
            var_id
                .strip_suffix("_level")
                .map(|prefix| (prefix, "level"))
        })
    {
        let name = match prefix {
            "high_card" => "High Card",
            "pair" => "Pair",
            "two_pair" => "Two Pair",
            "three_of_a_kind" => "Three of a Kind",
            "straight" => "Straight",
            "flush" => "Flush",
            "full_house" => "Full House",
            "four_of_a_kind" => "Four of a Kind",
            "straight_flush" => "Straight Flush",
            "five_of_a_kind" => "Five of a Kind",
            "flush_house" => "Flush House",
            "flush_five" => "Flush Five",
            _ => "",
        };
        if !name.is_empty() {
            return Some(poker_hand_value(&format!("'{name}'"), field, "true"));
        }
    }

    let profile_path = match var_id {
        "best_hand" => "high_scores.hand.amt",
        "highest_round" => "high_scores.furthest_round.amt",
        "highest_ante" | "most_money" => "high_scores.furthest_ante.amt",
        "most_played_hand" => "high_scores.poker_hand.amt",
        "best_win_streak" => "high_scores.win_streak.amt",
        "collection_player_amount" => "high_scores.collection.amt",
        "collection_total_amount" => "high_scores.collection.tot",
        "challenges_player_amount" => "progress.challenges.tally",
        "challenges_total_amount" => "progress.challenges.of",
        "joker_sticker_player_amount" => "progress.joker_stickers.tally",
        "joker_sticker_total_amount" => "progress.joker_stickers.of",
        "deck_stake_wins_player_amount" => "progress.deck_stakes.tally",
        "deck_stake_wins_total_amount" => "progress.deck_stakes.of",
        "max_round_interest_cap_streak" => {
            return Some(profile_value(
                "career_stats.c_round_interest_cap_streak",
                "2",
            ))
        }
        "total_dollars_earned" => "career_stats.c_dollars_earned",
        "total_shop_dollars_spent" => "career_stats.c_shop_dollars_spent",
        "total_tarots_bought" => "career_stats.c_tarots_bought",
        "total_planets_bought" => "career_stats.c_planets_bought",
        "total_playing_cards_bought" => "career_stats.c_playing_cards_bought",
        "total_vouchers_bought" => "career_stats.c_vouchers_bought",
        "total_arcana_used" => "career_stats.c_tarot_reading_used",
        "total_celestial_used" => "career_stats.c_planetarium_used",
        "total_shop_rerolls" => "career_stats.c_shop_rerolls",
        "total_cards_played" => "career_stats.c_cards_played",
        "total_cards_discarded" => "career_stats.c_cards_discarded",
        "total_losses" => "career_stats.c_losses",
        "wins" => "career_stats.c_wins",
        "total_total_rounds" => "career_stats.c_rounds",
        "total_hands_played" => "career_stats.c_hands_played",
        "total_face_cards_played" => "career_stats.c_face_cards_played",
        "total_jokers_sold" => "career_stats.c_jokers_sold",
        "total_cards_sold" => "career_stats.c_cards_sold",
        "single_hand_round_streak" => "career_stats.c_single_hand_round_streak",
        _ => "",
    };
    if !profile_path.is_empty() {
        return Some(profile_value(profile_path, "0"));
    }

    match var_id {
        "progress" => Some(profile_percentage(
            "progress.overall_tally",
            "progress.overall_of",
        )),
        "amount_shown" => Some(profile_percentage(
            "progress.discovered.tally",
            "progress.discovered.of",
        )),
        "challenges_amount_shown" => Some(profile_percentage(
            "progress.challenges.tally",
            "progress.challenges.of",
        )),
        "joker_sticker_amount_shown" => Some(profile_percentage(
            "progress.joker_stickers.tally",
            "progress.joker_stickers.of",
        )),
        "deck_stake_wins_amount_shown" => Some(profile_percentage(
            "progress.deck_stakes.tally",
            "progress.deck_stakes.of",
        )),
        "round_winned_joker1" => Some(profile_usage_count("joker_usage", 1)),
        "round_winned_joker5" => Some(profile_usage_count("joker_usage", 5)),
        "round_winned_joker10" => Some(profile_usage_count("joker_usage", 10)),
        "card_used_consumeable1" => Some(profile_usage_count("consumeable_usage", 1)),
        "card_used_consumeable5" => Some(profile_usage_count("consumeable_usage", 5)),
        "card_used_consumeable10" => Some(profile_usage_count("consumeable_usage", 10)),
        "card_redeemed_voucher1" => Some(profile_usage_count("voucher_usage", 1)),
        "card_redeemed_voucher5" => Some(profile_usage_count("voucher_usage", 5)),
        "card_redeemed_voucher10" => Some(profile_usage_count("voucher_usage", 10)),
        "money_per_5" | "money_per_10" | "money_per_15" | "money_per_20" | "money_per_25"
        | "money_per_30" | "money_per_40" | "money_per_50" => {
            let amount = var_id.strip_prefix("money_per_")?;
            Some(format!("(function() local amount = {} / {amount}; return math.floor(lenient_bignum and lenient_bignum(amount) or amount) end)()", game_field("GAME.dollars")))
        }
        _ => None,
    }
}

/// Validate explicit game-variable parameters before export. Ordinary literals
/// and user variables are outside this check. Resolvers additionally fail closed
/// to numeric zero because their existing APIs cannot return compile errors.
pub fn validate_game_var_reference(value: &ParamValue) -> Result<(), String> {
    let (reference, typed) = match value {
        ParamValue::Typed(value) if is_game_variable_type(&value.value_type) => (
            value
                .value
                .as_str()
                .ok_or("Game variable must be a string reference")?,
            true,
        ),
        ParamValue::Typed(value) => match value.value.as_str() {
            Some(value) if value.starts_with("GAMEVAR:") => (value, false),
            _ => return Ok(()),
        },
        ParamValue::Str(value) if value.starts_with("GAMEVAR:") => (value.as_str(), false),
        _ => return Ok(()),
    };
    let id = if reference.starts_with("GAMEVAR:") {
        parse_game_var(reference).ok_or("Invalid game variable reference: expected GAMEVAR:id|multiplier|startsFrom with finite numbers")?.var_id
    } else if typed {
        reference.to_string()
    } else {
        return Ok(());
    };
    if game_var_lua_code(&id).is_none() {
        return Err(format!("Unsupported game variable '{id}'"));
    }
    Ok(())
}

fn is_explicit_game_var(value: &ParamValue) -> bool {
    match value {
        ParamValue::Str(value) => value.starts_with("GAMEVAR:"),
        ParamValue::Typed(value) => {
            is_game_variable_type(&value.value_type)
                || value
                    .value
                    .as_str()
                    .is_some_and(|value| value.starts_with("GAMEVAR:"))
        }
        _ => false,
    }
}

/// Resolve a parameter value to a Lua expression.
///
/// Handles plain numbers, strings, game variable references: range references,
/// and user variable references. When a value is a config variable (stored in
/// ability.extra): `config_var_name` provides the key to reference.
pub fn resolve_value(
    value: &ParamValue,
    object_type: ObjectType,
    config_var_name: Option<&str>,
) -> Expr {
    match value {
        ParamValue::Int(n) => {
            if let Some(name) = config_var_name {
                return ability_path_expr(object_type, name);
            }
            lua_int(*n)
        }
        ParamValue::Float(n) => {
            if let Some(name) = config_var_name {
                return ability_path_expr(object_type, name);
            }
            lua_num(*n)
        }
        ParamValue::Bool(b) => lua_bool(*b),
        ParamValue::Str(s) => resolve_string_value(s, object_type, config_var_name),
        ParamValue::Typed(typed) => {
            if is_game_variable_type(&typed.value_type) {
                if let Some(s) = typed.value.as_str() {
                    if let Some(gv) = parse_game_var(s) {
                        return build_game_var_expr(&gv);
                    }
                    // Older typed values may contain the catalog ID directly.
                    if let Some(code) = game_var_lua_code(s) {
                        return lua_raw_expr(code);
                    }
                }
                lua_int(0)
            } else if is_range_type(&typed.value_type) {
                if let Some(s) = typed.value.as_str() {
                    if let Some(rv) = parse_range_var(s) {
                        return lua_call(
                            "pseudorandom",
                            vec![lua_str(s), lua_num(rv.min), lua_num(rv.max)],
                        );
                    }
                }
                lua_nil()
            } else if is_user_variable_type(&typed.value_type) {
                if let Some(name) = typed.value.as_str() {
                    return ability_path_expr(object_type, name);
                }
                lua_nil()
            } else {
                // Fall through to string/number resolution
                if let Some(n) = typed.value.as_f64() {
                    if let Some(name) = config_var_name {
                        return ability_path_expr(object_type, name);
                    }
                    if n.fract() == 0.0 {
                        lua_int(n as i64)
                    } else {
                        lua_num(n)
                    }
                } else if let Some(s) = typed.value.as_str() {
                    resolve_string_value(s, object_type, config_var_name)
                } else {
                    lua_nil()
                }
            }
        }
    }
}

fn resolve_string_value(s: &str, object_type: ObjectType, config_var_name: Option<&str>) -> Expr {
    // Game variable reference
    if s.starts_with("GAMEVAR:") {
        return parse_game_var(s)
            .map(|gv| build_game_var_expr(&gv))
            .unwrap_or_else(|| lua_int(0));
    }
    // Range variable reference
    if let Some(rv) = parse_range_var(s) {
        return lua_call(
            "pseudorandom",
            vec![lua_str(s), lua_num(rv.min), lua_num(rv.max)],
        );
    }
    // Try parsing as number
    if let Ok(n) = s.parse::<f64>() {
        if let Some(name) = config_var_name {
            return ability_path_expr(object_type, name);
        }
        if n.fract() == 0.0 {
            return lua_int(n as i64);
        }
        return lua_num(n);
    }
    // Plain string value
    lua_str(s)
}

/// Build the Lua expression for a game variable reference.
fn build_game_var_expr(gv: &GameVarRef) -> Expr {
    let Some(base_code) = game_var_lua_code(&gv.var_id) else {
        return lua_int(0);
    };
    let base_code = lua_raw_expr(base_code);
    let scaled = if gv.multiplier != 1.0 {
        lua_mul(base_code, lua_num(gv.multiplier))
    } else {
        base_code
    };
    // The editor defines the offset after scaling: start + (value * multiplier).
    if gv.starts_from != 0.0 {
        lua_add(lua_num(gv.starts_from), scaled)
    } else {
        scaled
    }
}

/// Build a `card.ability.extra.varName` expression.
pub fn ability_path_expr(object_type: ObjectType, var_name: &str) -> Expr {
    let base = object_type.ability_path();
    lua_field(lua_raw_expr(base), var_name)
}

// ---------------------------------------------------------------------------
// Dynamic config-variable resolution
// ---------------------------------------------------------------------------

/// Result of resolving a parameter value with automatic config-var registration.
pub struct ResolvedValue {
    /// Lua expression to use in the generated code.
    pub expr: Expr,
    /// Lua expression as a raw string (for format! based code generation).
    pub lua_str: String,
}

/// Resolve a value parameter: automatically registering a config variable
/// for literal numeric values. This is the centralized helper that ensures
/// every integer/float in an effect gets stored in `config.extra` and
/// referenced via `card.ability.extra.<var>`: making `loc_vars` work
/// automatically.
///
/// - `effect_params`: the effect's param map
/// - `param_key`: which param to read (e.g. "value", "amount")
/// - `ctx`: compile context for config-var registration
/// - `var_base`: base name for the config variable (e.g. "dollars", "level")
///
/// Returns the resolved expression and its string form.
pub fn resolve_config_value(
    effect_params: &std::collections::HashMap<String, crate::types::ParamValue>,
    param_key: &str,
    ctx: &mut crate::compiler::context::CompileContext,
    var_base: &str,
) -> ResolvedValue {
    use crate::types::ParamValue;

    let count = ctx.next_effect_count(var_base);
    let var_name = ctx.unique_var_name(var_base, count);

    if let Some(param) = effect_params.get(param_key) {
        if is_explicit_game_var(param) {
            let expr = resolve_value(param, ctx.object_type, None);
            let lua_str = expr.to_string();
            return ResolvedValue { expr, lua_str };
        }
    }

    match effect_params.get(param_key) {
        Some(ParamValue::Int(n)) => {
            ctx.bind_preview_config_parameter(&var_name, param_key);
            ctx.add_config_int(&var_name, *n);
            let path = format!("{}.{}", ctx.ability_path(), var_name);
            ResolvedValue {
                expr: ability_path_expr(ctx.object_type, &var_name),
                lua_str: path,
            }
        }
        Some(ParamValue::Float(n)) => {
            ctx.bind_preview_config_parameter(&var_name, param_key);
            ctx.add_config_num(&var_name, *n);
            let path = format!("{}.{}", ctx.ability_path(), var_name);
            ResolvedValue {
                expr: ability_path_expr(ctx.object_type, &var_name),
                lua_str: path,
            }
        }
        Some(ParamValue::Str(s)) => {
            // Try numeric parse first
            if let Ok(n) = s.parse::<f64>() {
                if n.fract() == 0.0 {
                    ctx.bind_preview_config_parameter(&var_name, param_key);
                    ctx.add_config_int(&var_name, n as i64);
                } else {
                    ctx.bind_preview_config_parameter(&var_name, param_key);
                    ctx.add_config_num(&var_name, n);
                }
                let path = format!("{}.{}", ctx.ability_path(), var_name);
                return ResolvedValue {
                    expr: ability_path_expr(ctx.object_type, &var_name),
                    lua_str: path,
                };
            }
            // User variable reference by name (legacy raw-string encoding)
            if ctx.has_user_var(s) {
                let path = ctx.user_var_path(s);
                return ResolvedValue {
                    expr: ctx.user_var_expr(s),
                    lua_str: path,
                };
            }
            // Game variable reference
            if let Some(gv) = parse_game_var(s) {
                let expr = build_game_var_expr(&gv);
                let code = expr.to_string();
                return ResolvedValue {
                    expr,
                    lua_str: code,
                };
            }
            // Plain string
            ResolvedValue {
                expr: lua_str(s),
                lua_str: format!("\"{}\"", s),
            }
        }
        Some(ParamValue::Typed(t)) => {
            if is_game_variable_type(&t.value_type) {
                if let Some(s) = t.value.as_str() {
                    if let Some(gv) = parse_game_var(s) {
                        let expr = build_game_var_expr(&gv);
                        let code = expr.to_string();
                        return ResolvedValue {
                            expr,
                            lua_str: code,
                        };
                    }
                }
                ResolvedValue {
                    expr: lua_int(0),
                    lua_str: "0".to_string(),
                }
            } else if is_range_type(&t.value_type) {
                if let Some(s) = t.value.as_str() {
                    if let Some(rv) = parse_range_var(s) {
                        let expr = lua_call(
                            "pseudorandom",
                            vec![lua_str(s), lua_num(rv.min), lua_num(rv.max)],
                        );
                        let code = format!("pseudorandom(\"{}\", {}, {})", s, rv.min, rv.max);
                        return ResolvedValue {
                            expr,
                            lua_str: code,
                        };
                    }
                }
                ResolvedValue {
                    expr: lua_int(0),
                    lua_str: "0".to_string(),
                }
            } else if is_user_variable_type(&t.value_type) {
                if let Some(name) = t.value.as_str() {
                    let path = ctx.user_var_path(name);
                    return ResolvedValue {
                        expr: ctx.user_var_expr(name),
                        lua_str: path,
                    };
                }
                ResolvedValue {
                    expr: lua_int(0),
                    lua_str: "0".to_string(),
                }
            } else {
                // Try numeric
                if let Some(n) = t.value.as_f64() {
                    if n.fract() == 0.0 {
                        ctx.bind_preview_config_parameter(&var_name, param_key);
                        ctx.add_config_int(&var_name, n as i64);
                    } else {
                        ctx.bind_preview_config_parameter(&var_name, param_key);
                        ctx.add_config_num(&var_name, n);
                    }
                    let path = format!("{}.{}", ctx.ability_path(), var_name);
                    return ResolvedValue {
                        expr: ability_path_expr(ctx.object_type, &var_name),
                        lua_str: path,
                    };
                }
                if let Some(s) = t.value.as_str() {
                    if let Ok(n) = s.parse::<f64>() {
                        if n.fract() == 0.0 {
                            ctx.bind_preview_config_parameter(&var_name, param_key);
                            ctx.add_config_int(&var_name, n as i64);
                        } else {
                            ctx.bind_preview_config_parameter(&var_name, param_key);
                            ctx.add_config_num(&var_name, n);
                        }
                        let path = format!("{}.{}", ctx.ability_path(), var_name);
                        return ResolvedValue {
                            expr: ability_path_expr(ctx.object_type, &var_name),
                            lua_str: path,
                        };
                    }
                    if ctx.has_user_var(s) {
                        let path = ctx.user_var_path(s);
                        return ResolvedValue {
                            expr: ctx.user_var_expr(s),
                            lua_str: path,
                        };
                    }
                    // Try game var
                    if let Some(code) = game_var_lua_code(s) {
                        return ResolvedValue {
                            expr: lua_raw_expr(&code),
                            lua_str: code,
                        };
                    }
                    return ResolvedValue {
                        expr: lua_str(s),
                        lua_str: format!("\"{}\"", s),
                    };
                }
                ResolvedValue {
                    expr: lua_int(1),
                    lua_str: "1".to_string(),
                }
            }
        }
        Some(ParamValue::Bool(b)) => ResolvedValue {
            expr: lua_bool(*b),
            lua_str: if *b { "true" } else { "false" }.to_string(),
        },
        None => ResolvedValue {
            expr: lua_int(1),
            lua_str: "1".to_string(),
        },
    }
}

/// Resolve a numeric condition parameter, automatically registering it in
/// `config.extra` and returning the ability-path expression. Uses the
/// condition type and parameter name to build a unique slug, e.g.
/// `hand_count_value0`. Non-numeric values fall through to a literal.
pub fn resolve_condition_value(
    condition_params: &std::collections::HashMap<String, crate::types::ParamValue>,
    param_key: &str,
    ctx: &mut crate::compiler::context::CompileContext,
    condition_type: &str,
) -> Option<Expr> {
    use crate::types::ParamValue;

    let param = condition_params.get(param_key)?;

    if is_explicit_game_var(param) {
        return Some(resolve_value(param, ctx.object_type, None));
    }

    // Build the base name from condition_type + param_key
    let var_base = format!("{}_{}", condition_type, param_key);

    match param {
        ParamValue::Int(n) => {
            let count = ctx.next_effect_count(&var_base);
            let var_name = ctx.unique_var_name(&var_base, count);
            ctx.bind_preview_config_parameter(&var_name, param_key);
            ctx.add_config_int(&var_name, *n);
            Some(ability_path_expr(ctx.object_type, &var_name))
        }
        ParamValue::Float(n) => {
            let count = ctx.next_effect_count(&var_base);
            let var_name = ctx.unique_var_name(&var_base, count);
            ctx.bind_preview_config_parameter(&var_name, param_key);
            ctx.add_config_num(&var_name, *n);
            Some(ability_path_expr(ctx.object_type, &var_name))
        }
        ParamValue::Str(s) => {
            if s.trim().is_empty() {
                return None;
            }
            if let Ok(n) = s.parse::<f64>() {
                let count = ctx.next_effect_count(&var_base);
                let var_name = ctx.unique_var_name(&var_base, count);
                if n.fract() == 0.0 {
                    ctx.bind_preview_config_parameter(&var_name, param_key);
                    ctx.add_config_int(&var_name, n as i64);
                } else {
                    ctx.bind_preview_config_parameter(&var_name, param_key);
                    ctx.add_config_num(&var_name, n);
                }
                Some(ability_path_expr(ctx.object_type, &var_name))
            } else if ctx.has_user_var(s) {
                Some(ctx.user_var_expr(s))
            } else {
                // Non-numeric string literals must be quoted.
                Some(lua_str(s))
            }
        }
        ParamValue::Typed(t) => {
            if is_game_variable_type(&t.value_type) {
                if let Some(s) = t.value.as_str() {
                    if let Some(gv) = parse_game_var(s) {
                        return Some(build_game_var_expr(&gv));
                    }
                }
                None
            } else if is_user_variable_type(&t.value_type) {
                if let Some(name) = t.value.as_str() {
                    return Some(ctx.user_var_expr(name));
                }
                None
            } else {
                if let Some(n) = t.value.as_i64() {
                    let count = ctx.next_effect_count(&var_base);
                    let var_name = ctx.unique_var_name(&var_base, count);
                    ctx.bind_preview_config_parameter(&var_name, param_key);
                    ctx.add_config_int(&var_name, n);
                    return Some(ability_path_expr(ctx.object_type, &var_name));
                }
                if let Some(n) = t.value.as_f64() {
                    let count = ctx.next_effect_count(&var_base);
                    let var_name = ctx.unique_var_name(&var_base, count);
                    ctx.bind_preview_config_parameter(&var_name, param_key);
                    ctx.add_config_num(&var_name, n);
                    return Some(ability_path_expr(ctx.object_type, &var_name));
                }
                if let Some(s) = t.value.as_str() {
                    if s.trim().is_empty() {
                        return None;
                    }
                    if ctx.has_user_var(s) {
                        return Some(ctx.user_var_expr(s));
                    }
                    return Some(lua_str(s));
                }
                None
            }
        }
        _ => None,
    }
}

/// Convert a comparison operator string to the corresponding Lua binary op expression.
pub fn comparison_op(operator: &str, lhs: Expr, rhs: Expr) -> Expr {
    match operator {
        "equals" | "equal" | "==" => lua_eq(lhs, rhs),
        "not_equals" | "not_equal" | "~=" => lua_neq(lhs, rhs),
        "greater_than" | ">" => lua_gt(lhs, rhs),
        "less_than" | "<" => lua_lt(lhs, rhs),
        "greater_than_or_equal" | "greater_equals" | ">=" => lua_ge(lhs, rhs),
        "less_than_or_equal" | "less_equals" | "<=" => lua_le(lhs, rhs),
        _ => lua_eq(lhs, rhs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TypedValue;
    use serde_json::json;

    #[test]
    fn editor_game_var_ids_resolve_to_balatro_state() {
        assert_eq!(
            game_var_lua_code("hands_remaining"),
            Some(
                "(G and G.GAME and G.GAME.current_round and G.GAME.current_round.hands_left or 0)"
                    .to_string()
            )
        );
        assert_eq!(
            game_var_lua_code("discards_remaining"),
            Some("(G and G.GAME and G.GAME.current_round and G.GAME.current_round.discards_left or 0)".to_string())
        );
    }

    #[test]
    fn typed_game_var_preserves_start_and_multiplier() {
        let value = ParamValue::Typed(TypedValue {
            value: json!("GAMEVAR:hands_remaining|2|3"),
            value_type: "game_var".to_string(),
        });

        assert_eq!(
            resolve_value(&value, ObjectType::Joker, None).to_string(),
            "3 + (G and G.GAME and G.GAME.current_round and G.GAME.current_round.hands_left or 0) * 2"
        );
    }

    #[test]
    fn every_editor_game_variable_is_supported() {
        let catalog = include_str!("../../../../../src/lib/content/game-vars.ts");
        let mut current_id = None;
        let mut checked = 0;
        for line in catalog.lines() {
            let line = line.trim();
            if let Some(id) = line
                .strip_prefix("id: \"")
                .and_then(|line| line.strip_suffix("\","))
            {
                current_id = Some(id);
            } else if line
                .strip_prefix("code:")
                .is_some_and(|value| value.trim_start().starts_with(['\'', '"']))
            {
                let id = current_id.expect("catalog variable ID precedes its code");
                assert!(
                    game_var_lua_code(id).is_some(),
                    "unsupported editor variable: {id}"
                );
                checked += 1;
            }
        }
        assert!(
            checked >= 209,
            "catalog coverage must not silently become empty"
        );
    }

    #[test]
    fn time_variables_read_the_corresponding_local_date_field() {
        for (id, field) in [
            ("current_seconds", "sec"),
            ("current_minutes", "min"),
            ("current_hours", "hour"),
            ("current_day", "day"),
            ("current_month", "month"),
            ("current_year", "year"),
            ("current_year_day", "yday"),
            ("current_week_day", "wday"),
        ] {
            assert_eq!(
                game_var_lua_code(id).unwrap(),
                format!("(function() local date = os and os.date and os.date('*t'); return date and date.{field} or 0 end)()")
            );
        }
    }

    #[test]
    fn finite_game_variable_scalars_larger_than_i64_are_preserved() {
        let value = ParamValue::Str("GAMEVAR:cards_in_deck|1e30|1e40".to_string());
        let Expr::BinOp(offset, BinOp::Add, scaled) =
            resolve_value(&value, ObjectType::Joker, None)
        else {
            panic!("expected offset + scaled game variable");
        };
        assert!(matches!(*offset, Expr::Number(value) if value == 1e40));
        let Expr::BinOp(_, BinOp::Mul, multiplier) = *scaled else {
            panic!("expected multiplication");
        };
        assert!(matches!(*multiplier, Expr::Number(value) if value == 1e30));
    }

    #[test]
    fn legacy_game_variable_aliases_keep_their_meaning() {
        for (legacy, current) in [
            ("deck_size", "cards_in_deck"),
            ("full_deck_size", "total_playing_cards"),
            ("hand_size", "cards_in_hand"),
            ("remaining_hands", "hands_remaining"),
            ("remaining_discards", "discards_remaining"),
            ("player_money", "current_money"),
            ("dollars", "current_money"),
            ("ante_level", "current_ante"),
            ("blind_chips", "blind_chip_req"),
            ("consumable_count", "consumables_held"),
            ("times_hand_played", "current_hand_played_count"),
        ] {
            assert_eq!(
                game_var_lua_code(legacy),
                game_var_lua_code(current),
                "{legacy}"
            );
        }
    }

    #[test]
    fn malformed_game_variable_encodings_are_rejected() {
        for reference in [
            "GAMEVAR:",
            "GAMEVAR:cards_in_deck",
            "GAMEVAR:cards_in_deck|1",
            "GAMEVAR:|1|0",
            "GAMEVAR:cards_in_deck|no|0",
            "GAMEVAR:cards_in_deck|1|no",
            "GAMEVAR:cards_in_deck|NaN|0",
            "GAMEVAR:cards_in_deck|1|inf",
            "GAMEVAR:cards_in_deck|1|0|extra",
        ] {
            assert!(parse_game_var(reference).is_none(), "{reference}");
            for value in [
                ParamValue::Str(reference.to_string()),
                ParamValue::Typed(TypedValue {
                    value: json!(reference),
                    value_type: "game_var".to_string(),
                }),
                ParamValue::Typed(TypedValue {
                    value: json!(reference),
                    value_type: "specific".to_string(),
                }),
            ] {
                assert!(validate_game_var_reference(&value).is_err(), "{reference}");
                assert_eq!(
                    resolve_value(&value, ObjectType::Joker, None).to_string(),
                    "0",
                    "{reference}"
                );
            }
        }
    }

    #[test]
    fn unsupported_game_variables_never_become_raw_lua() {
        for reference in ["GAMEVAR:missing|2|3", "GAMEVAR:os.execute('bad')|2|3"] {
            let value = ParamValue::Str(reference.to_string());
            assert!(validate_game_var_reference(&value)
                .unwrap_err()
                .starts_with("Unsupported game variable"));
            assert_eq!(
                resolve_value(&value, ObjectType::Joker, None).to_string(),
                "0"
            );
        }
    }

    #[test]
    fn config_and_condition_resolvers_evaluate_raw_and_typed_game_variables() {
        use crate::compiler::context::CompileContext;
        use std::collections::HashMap;

        for reference in [
            "GAMEVAR:cards_in_deck|2|3",
            "GAMEVAR:missing|2|3",
            "GAMEVAR:cards_in_deck|NaN|0",
        ] {
            for value in [
                ParamValue::Str(reference.to_string()),
                ParamValue::Typed(TypedValue {
                    value: json!(reference),
                    value_type: "gameVariable".to_string(),
                }),
                ParamValue::Typed(TypedValue {
                    value: json!(reference),
                    value_type: "game_var".to_string(),
                }),
            ] {
                let expected = resolve_value(&value, ObjectType::Joker, None).to_string();
                let params = HashMap::from([("value".to_string(), value)]);
                let mut ctx = CompileContext::new(
                    ObjectType::Joker,
                    "test".to_string(),
                    "joker".to_string(),
                    false,
                );
                assert_eq!(
                    resolve_config_value(&params, "value", &mut ctx, "chips").lua_str,
                    expected
                );
                assert_eq!(
                    resolve_condition_value(&params, "value", &mut ctx, "hand_count")
                        .unwrap()
                        .to_string(),
                    expected
                );
                assert!(ctx.config_vars().is_empty());
            }
        }
    }

    #[test]
    fn typed_legacy_direct_ids_resolve_to_known_game_variables() {
        for value_type in ["gameVariable", "game_var"] {
            let value = ParamValue::Typed(TypedValue {
                value: json!("cards_in_deck"),
                value_type: value_type.to_string(),
            });
            assert!(validate_game_var_reference(&value).is_ok());
            assert_eq!(
                resolve_value(&value, ObjectType::Joker, None).to_string(),
                game_var_lua_code("cards_in_deck").unwrap()
            );
        }
    }
}
