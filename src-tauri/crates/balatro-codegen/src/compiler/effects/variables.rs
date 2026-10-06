use crate::compiler::context::CompileContext;
use crate::compiler::effects::utils::{get_str, get_str_default};
use crate::compiler::effects::EffectOutput;
use crate::lua_ast::*;
use crate::types::{EffectDef, ParamValue};

fn is_scoring_trigger(trigger: &str) -> bool {
    matches!(trigger, "hand_played" | "card_scored")
}

// ---------------------------------------------------------------------------
// modify_internal_variable
// ---------------------------------------------------------------------------

/// Modify Internal Variable: changes a user-defined number variable.
pub fn modify_internal_variable(
    effect: &EffectDef,
    ctx: &mut CompileContext,
    trigger: &str,
) -> EffectOutput {
    let variable_name = get_str_default(effect, "variable_name", "var1");
    let operation = get_str_default(effect, "operation", "increment");
    let index_method = get_str_default(effect, "index_method", "self");
    let custom_message = get_str(effect, "customMessage");
    let search_key = get_str_default(effect, "joker_key", "j_joker");
    let search_var = get_str_default(effect, "joker_variable", "jokerVar");
    let scoring = is_scoring_trigger(trigger);

    let resolved = crate::compiler::values::resolve_config_value(
        &effect.params,
        "value",
        ctx,
        &format!("var_{}", variable_name),
    );
    let variable_path = ctx.user_var_path(&variable_name);

    let operation_code = match operation.as_str() {
        "set" => format!(
            "{path} = {val}",
            path = variable_path,
            val = resolved.lua_str
        ),
        "increment" => format!(
            "{path} = ({path}) + {val}",
            path = variable_path,
            val = resolved.lua_str
        ),
        "decrement" => format!(
            "{path} = math.max(0, ({path}) - {val})",
            path = variable_path,
            val = resolved.lua_str
        ),
        "multiply" => format!(
            "{path} = ({path}) * {val}",
            path = variable_path,
            val = resolved.lua_str
        ),
        "divide" => format!(
            "{path} = ({path}) / {val}",
            path = variable_path,
            val = resolved.lua_str
        ),
        "power" => format!(
            "{path} = ({path}) ^ {val}",
            path = variable_path,
            val = resolved.lua_str
        ),
        "absolute" => format!("{path} = math.abs({path})", path = variable_path),
        "natural_log" => format!("{path} = math.log({path})", path = variable_path),
        "log10" => format!("{path} = math.log10({path})", path = variable_path),
        "square_root" => format!("{path} = math.sqrt({path})", path = variable_path),
        "ceil" => format!("{path} = math.ceil({path})", path = variable_path),
        "floor" => format!("{path} = math.floor({path})", path = variable_path),
        "index" => match index_method.as_str() {
            "self" => format!(
                "for i = 1, #G.jokers.cards do\n\
                    if G.jokers.cards[i] == card then\n\
                        {path} = i\n\
                        break\n\
                    end\n\
                end",
                path = variable_path
            ),
            "random" => format!(
                "{path} = math.random(1, #G.jokers.cards)",
                path = variable_path
            ),
            "first" => format!("{path} = 1", path = variable_path),
            "last" => format!("{path} = #G.jokers.cards", path = variable_path),
            "left" => format!(
                "local my_pos = nil\n\
                for i = 1, #G.jokers.cards do\n\
                    if G.jokers.cards[i] == card then\n\
                        my_pos = i\n\
                        break\n\
                    end\n\
                end\n\
                {path} = math.max(my_pos - 1, 0)",
                path = variable_path
            ),
            "right" => format!(
                "local my_pos = nil\n\
                for i = 1, #G.jokers.cards do\n\
                    if G.jokers.cards[i] == card then\n\
                        my_pos = i\n\
                        break\n\
                    end\n\
                end\n\
                if my_pos > #G.jokers.cards then\n\
                    my_pos = -1\n\
                end\n\
                {path} = my_pos + 1",
                path = variable_path
            ),
            "key" => format!(
                "local search_key = '{key}'\n\
                {path} = 0\n\
                for i = 1, #G.jokers.cards do\n\
                    if G.jokers.cards[i].config.center.key == search_key then\n\
                        {path} = i\n\
                        break\n\
                    end\n\
                end",
                path = variable_path,
                key = search_key
            ),
            "variable" => format!(
                "local search_key = {search_var_path}\n\
                {path} = 0\n\
                for i = 1, #G.jokers.cards do\n\
                    if G.jokers.cards[i].config.center.key == search_key then\n\
                        {path} = i\n\
                        break\n\
                    end\n\
                end",
                search_var_path = ctx.user_var_path(&search_var),
                path = variable_path
            ),
            "selected_joker" => format!(
                "for i = 1, #G.jokers.cards do\n\
                    if G.jokers.cards[i] == G.jokers.highlighted[1] then\n\
                        {path} = i\n\
                        break\n\
                    end\n\
                end",
                path = variable_path
            ),
            "evaled_joker" => format!(
                "for i = 1, #G.jokers.cards do\n\
                    if G.jokers.cards[i] == context.other_joker then\n\
                        {path} = i\n\
                        break\n\
                    end\n\
                end",
                path = variable_path
            ),
            _ => format!(
                "{path} = ({path}) + {val}",
                path = variable_path,
                val = resolved.lua_str
            ),
        },
        _ => format!(
            "{path} = ({path}) + {val}",
            path = variable_path,
            val = resolved.lua_str
        ),
    };

    let message_colour = match operation.as_str() {
        "set" => "G.C.BLUE",
        "increment" => "G.C.GREEN",
        "decrement" => "G.C.RED",
        "multiply" | "divide" => "G.C.MULT",
        _ => "G.C.BLUE",
    };

    let message = custom_message.map(lua_str);

    if scoring {
        EffectOutput {
            return_fields: vec![],
            pre_return: vec![lua_raw_stmt(operation_code)],
            config_vars: vec![],
            message,
            colour: Some(lua_raw_expr(message_colour)),

            segment_id: None,
        }
    } else {
        let func_body = vec![lua_raw_stmt(format!("{}\nreturn true", operation_code))];
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
            message,
            colour: Some(lua_raw_expr(message_colour)),

            segment_id: None,
        }
    }
}

// ---------------------------------------------------------------------------
// change_key_variable
// ---------------------------------------------------------------------------

/// Change Key Variable: changes a key-type user variable.
pub fn change_key_variable(effect: &EffectDef, ctx: &mut CompileContext) -> EffectOutput {
    let Some(variable_name) =
        key_string_param(effect, &["variable_name", "variableName", "variable"])
    else {
        return EffectOutput::default();
    };
    let key_type = key_string_param(effect, &["key_type", "keyType"]).unwrap_or("joker");
    let pool_name = match key_type {
        "joker" => "Joker",
        "consumable" => "Consumeables",
        "tarot" => "Tarot",
        "planet" => "Planet",
        "spectral" => "Spectral",
        "enhancement" => "Enhanced",
        "seal" => "Seal",
        "edition" => "Edition",
        "voucher" => "Voucher",
        "tag" => "Tag",
        "booster" => "Booster",
        _ => return EffectOutput::default(),
    };
    // The editor has a separate change selector for every key type. Read the
    // original generic fields only when the current fields are absent.
    let selector_key = format!("{key_type}_change_type");
    let selector = effect
        .params
        .get(&selector_key)
        .or_else(|| effect.params.get("change_type"));
    let change_type = match selector {
        None => "specific",
        Some(value) => match value
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(value) => value,
            None => return EffectOutput::default(),
        },
    };
    let custom_message = get_str(effect, "customMessage");
    let variable_path = ctx.user_var_path(&variable_name);
    let is_user_reference = selector.is_some_and(|value| match value {
        ParamValue::Typed(value) => {
            crate::compiler::values::is_user_variable_type(&value.value_type)
        }
        ParamValue::Str(name) => ctx.has_user_var(name),
        _ => false,
    });
    let code = if is_user_reference {
        guarded_key_assignment(&variable_path, &ctx.user_var_path(change_type))
    } else if let Some(context_key) = key_context_value(change_type, key_type) {
        guarded_key_assignment(&variable_path, &context_key)
    } else {
        match change_type {
            "specific" => {
                let specific = format!("specific_{key_type}");
                let Some(value) = effect
                    .params
                    .get(&specific)
                    .or_else(|| effect.params.get("specific_key"))
                else {
                    return EffectOutput::default();
                };
                let Some(key) = value.as_str().map(str::trim).filter(|key| !key.is_empty()) else {
                    return EffectOutput::default();
                };
                let value = match value {
                    ParamValue::Typed(value)
                        if crate::compiler::values::is_user_variable_type(&value.value_type) =>
                    {
                        ctx.user_var_path(key)
                    }
                    _ => key_literal_value(key, key_type, &ctx.mod_prefix),
                };
                guarded_key_assignment(&variable_path, &value)
            }
            "random" | "increment" => {
                let Some((pool, filter)) = key_variable_pool(effect, key_type, pool_name, ctx)
                else {
                    return EffectOutput::default();
                };
                let build_pool = format!(
                    "local source = {pool}\nlocal candidates = {{}}\nfor _, entry in ipairs(source or {{}}) do\n    local item = type(entry) == 'string' and ((G and G.P_CENTERS and G.P_CENTERS[entry]) or (G and G.P_SEALS and G.P_SEALS[entry]) or (G and G.P_TAGS and G.P_TAGS[entry])) or entry\n    if type(item) == 'table' and type(item.key) == 'string' and ({filter}) then candidates[#candidates + 1] = item end\nend"
                );
                let pick = if change_type == "random" {
                    format!(
                        "if #candidates > 0 then\n    local chosen = pseudorandom_element(candidates, pseudoseed({seed}))\n    if chosen and chosen.key then {variable_path} = chosen.key end\nend",
                        seed = lua_str(variable_name),
                    )
                } else {
                    let count_key = format!("{key_type}_increment_count");
                    let count = effect
                        .params
                        .get(&count_key)
                        .or_else(|| effect.params.get("increment_count"));
                    let amount = count
                        .map(|value| key_numeric_value(value, ctx))
                        .unwrap_or_else(|| "1".to_string());
                    format!(
                        "local step = tonumber({amount})\nif #candidates > 0 and step and step == step and step ~= math.huge and step ~= -math.huge then\n    for index, item in ipairs(candidates) do\n        if item.key == {variable_path} then\n            {variable_path} = candidates[((index - 1 + math.floor(step)) % #candidates) + 1].key\n            break\n        end\n    end\nend"
                    )
                };
                format!("{build_pool}\n{pick}")
            }
            _ => return EffectOutput::default(),
        }
    };

    let message = custom_message.map(lua_str);

    EffectOutput {
        return_fields: vec![],
        pre_return: vec![lua_raw_stmt(format!("do\n{code}\nend"))],
        config_vars: vec![],
        message,
        colour: Some(lua_raw_expr("G.C.FILTER")),

        segment_id: None,
    }
}

fn key_string_param<'a>(effect: &'a EffectDef, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        effect
            .params
            .get(*key)
            .and_then(ParamValue::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

fn guarded_key_assignment(path: &str, value: &str) -> String {
    format!("local new_key = {value}\nif type(new_key) == 'string' and new_key ~= '' then {path} = new_key end")
}

/// Selectors can copy a key variable or the object supplied by the trigger.
/// Every context is optional, so an absent source preserves the current key.
fn key_context_value(mode: &str, key_type: &str) -> Option<String> {
    let source = match mode {
        "evaled_joker" | "evaluated_joker" => "(context and context.other_joker)",
        "selected_joker" => "(G and G.jokers and G.jokers.highlighted and G.jokers.highlighted[1])",
        "scored_card" | "held_card" | "card_held_in_hand" | "discarded_card" => "(context and context.other_card)",
        "destroyed_card" => "(context and (context.other_card or context.destroy_card or (context.removed and context.removed[1])))",
        "added_card" => "(context and (context.other_card or context.card or (context.cards and context.cards[1])))",
        "used_consumable" => "(context and (context.consumeable or context.consumable))",
        "redeemed_voucher" => "(context and (context.voucher or context.card))",
        "opened_booster" | "skipped_booster" | "exited_booster" => "(context and (context.booster or context.card))",
        "added_tag" => "(context and context.tag_added)",
        "blind_tag" => "(G and G.GAME and G.GAME.round_resets and G.GAME.round_resets.blind_tag)",
        _ => return None,
    };
    let value = match key_type {
        "seal" => "(source and source.seal)",
        "edition" => "(source and source.edition and source.edition.key)",
        _ => "(source and ((source.config and source.config.center and source.config.center.key) or source.key))",
    };
    Some(format!(
        "(function() local source = {source}; return {value} end)()"
    ))
}

fn key_literal_value(key: &str, key_type: &str, mod_prefix: &str) -> String {
    let class_prefix = match key_type {
        "joker" => "j_",
        "consumable" | "tarot" | "planet" | "spectral" => "c_",
        "enhancement" => "m_",
        "edition" => "e_",
        "voucher" => "v_",
        "booster" => "p_",
        "tag" => "tag_",
        _ => "",
    };
    if !class_prefix.is_empty() && key.starts_with(class_prefix) {
        return lua_str(key).to_string();
    }
    // These selectors supply vanilla short aliases rather than registry keys.
    // Preserve their meaning even when no run/registry is loaded yet.
    let vanilla_alias = match key_type {
        "tag" => matches!(
            key,
            "uncommon"
                | "rare"
                | "negative"
                | "foil"
                | "holo"
                | "polychrome"
                | "investment"
                | "voucher"
                | "boss"
                | "standard"
                | "charm"
                | "meteor"
                | "buffoon"
                | "handy"
                | "garbage"
                | "ethereal"
                | "coupon"
                | "double"
                | "juggle"
                | "d_six"
                | "top_up"
                | "skip"
                | "orbital"
                | "economy"
        ),
        "edition" => matches!(key, "foil" | "holo" | "polychrome" | "negative"),
        "enhancement" => matches!(
            key,
            "bonus" | "mult" | "wild" | "glass" | "steel" | "stone" | "gold" | "lucky"
        ),
        _ => false,
    };
    if vanilla_alias {
        return lua_str(format!("{class_prefix}{key}")).to_string();
    }
    // Seals have no class prefix; registered keys and fully mod-prefixed
    // selector values should be used verbatim.
    if key_type == "seal" && matches!(key, "Gold" | "Red" | "Blue" | "Purple") {
        return lua_str(key).to_string();
    }
    let bare = format!("{class_prefix}{key}");
    let local = if mod_prefix.is_empty() || key.starts_with(&format!("{mod_prefix}_")) {
        bare.clone()
    } else {
        format!("{class_prefix}{mod_prefix}_{key}")
    };
    let registry = match key_type {
        "seal" => "G.P_SEALS",
        "tag" => "G.P_TAGS",
        _ => "G.P_CENTERS",
    };
    format!(
        "(function() local registry = G and {registry}; if registry and registry[{key}] then return {key} end; if registry and registry[{bare}] then return {bare} end; return {local} end)()",
        key = lua_str(key), bare = lua_str(bare), local = lua_str(local),
    )
}

fn key_numeric_value(value: &ParamValue, ctx: &CompileContext) -> String {
    if let ParamValue::Typed(value) = value {
        if crate::compiler::values::is_user_variable_type(&value.value_type) {
            return value
                .value
                .as_str()
                .map(|name| ctx.user_var_path(name))
                .unwrap_or_else(|| "nil".into());
        }
    }
    if let ParamValue::Str(value) = value {
        if ctx.has_user_var(value) {
            return ctx.user_var_path(value);
        }
    }
    crate::compiler::values::resolve_value(value, ctx.object_type, None).to_string()
}

/// Return the source and selection predicate for the live catalog's random
/// options. Increment always uses the whole collection, in collection order.
fn key_variable_pool(
    effect: &EffectDef,
    key_type: &str,
    pool_name: &str,
    ctx: &CompileContext,
) -> Option<(String, String)> {
    let mode_key = format!("{key_type}_change_type");
    let random_key = format!("{key_type}_random_type");
    let mode = key_string_param(effect, &[&mode_key, "change_type"]).unwrap_or("specific");
    let selection = if mode == "increment" {
        "all"
    } else {
        key_string_param(effect, &[&random_key, "random_type"]).unwrap_or("all")
    };
    let pool = format!(
        "(G and G.P_CENTER_POOLS and G.P_CENTER_POOLS[{}])",
        lua_str(pool_name)
    );
    let result = match (key_type, selection) {
        (_, "all") => (pool, "true".to_string()),
        ("joker", "unlocked") => (pool, "item.unlocked ~= false".to_string()),
        ("joker", "locked") => (pool, "item.unlocked == false".to_string()),
        ("joker" | "consumable", "owned") => {
            let area = if key_type == "joker" {
                "jokers"
            } else {
                "consumeables"
            };
            (format!("(function() local owned = {{}}; for _, card in ipairs((G and G.{area} and G.{area}.cards) or {{}}) do local center = card.config and card.config.center; if center then owned[#owned + 1] = center end end; return owned end)()"), "true".to_string())
        }
        ("joker", "pool") => {
            let name = key_string_param(effect, &["joker_pool", "pool"])?;
            let prefixed =
                if ctx.mod_prefix.is_empty() || name.starts_with(&format!("{}_", ctx.mod_prefix)) {
                    name.to_string()
                } else {
                    format!("{}_{name}", ctx.mod_prefix)
                };
            (format!("(G and G.P_CENTER_POOLS and (G.P_CENTER_POOLS[{name}] or G.P_CENTER_POOLS[{prefixed}]))", name = lua_str(name), prefixed = lua_str(prefixed)), "true".to_string())
        }
        ("joker", "rarity") => {
            let rarity = key_string_param(effect, &["joker_rarity", "rarity"])?;
            let (value, alias) = match rarity.to_ascii_lowercase().as_str() {
                "common" | "1" => ("1".into(), "Common".into()),
                "uncommon" | "2" => ("2".into(), "Uncommon".into()),
                "rare" | "3" => ("3".into(), "Rare".into()),
                "legendary" | "4" => ("4".into(), "Legendary".into()),
                _ => {
                    let custom = if ctx.mod_prefix.is_empty()
                        || rarity.starts_with(&format!("{}_", ctx.mod_prefix))
                    {
                        rarity.to_string()
                    } else {
                        format!("{}_{rarity}", ctx.mod_prefix)
                    };
                    (lua_str(&custom).to_string(), rarity.to_string())
                }
            };
            (
                pool,
                format!(
                    "(item.rarity == {value} or item.rarity == {alias})",
                    alias = lua_str(alias)
                ),
            )
        }
        ("consumable", "set") => {
            let set = key_string_param(effect, &["consumable_set", "set"])?;
            if set == "all" {
                (pool, "true".into())
            } else {
                let prefixed = if ctx.mod_prefix.is_empty()
                    || set.starts_with(&format!("{}_", ctx.mod_prefix))
                {
                    set.to_string()
                } else {
                    format!("{}_{set}", ctx.mod_prefix)
                };
                (
                    pool,
                    format!(
                        "(item.set == {set} or item.set == {prefixed})",
                        set = lua_str(set),
                        prefixed = lua_str(prefixed)
                    ),
                )
            }
        }
        ("voucher", "possible") => (
            "(get_current_pool and get_current_pool('Voucher'))".into(),
            "true".into(),
        ),
        ("booster", "category") => {
            let category =
                key_string_param(effect, &["booster_category", "category"]).unwrap_or("Arcana");
            (pool, format!("item.kind == {}", lua_str(category)))
        }
        ("booster", "size") => {
            let extra = key_numeric_value(effect.params.get("booster_size_extra")?, ctx);
            let choose = key_numeric_value(effect.params.get("booster_size_choose")?, ctx);
            (pool, format!("(item.config and item.config.extra == tonumber({extra}) and item.config.choose == tonumber({choose}))"))
        }
        _ => return None,
    };
    Some(result)
}

// ---------------------------------------------------------------------------
// change_text_variable
// ---------------------------------------------------------------------------

/// Change Text Variable: changes a text-type user variable.
pub fn change_text_variable(effect: &EffectDef, ctx: &mut CompileContext) -> EffectOutput {
    let variable_name = get_str_default(effect, "variable_name", "textvar");
    let change_type = get_str_default(effect, "change_type", "custom_text");
    let custom_text = get_str_default(effect, "text", "");
    let key_var = get_str_default(effect, "key_variable", "keyvar");
    let custom_message = get_str(effect, "customMessage");

    let variable_path = ctx.user_var_path(&variable_name);
    let key_var_path = ctx.user_var_path(&key_var);
    let code = match change_type.as_str() {
        "key_var" => format!(
            "local all_key_lists = {{}}\n\
            for _, pool in pairs(G.P_CENTER_POOLS) do\n\
                for _, item in pairs(pool) do\n\
                    table.insert(all_key_lists, item)\n\
                end\n\
            end\n\
            for _, current_card in pairs(all_key_lists) do\n\
                if current_card.key == {kv_path} then\n\
                    if current_card.set == 'Seal' then\n\
                        {var_path} = current_card.key\n\
                    else\n\
                        {var_path} = current_card.name\n\
                    end\n\
                    break\n\
                end\n\
            end",
            var_path = variable_path,
            kv_path = key_var_path
        ),
        _ => format!("{path} = '{t}'", path = variable_path, t = custom_text),
    };

    let message = custom_message.map(lua_str);

    EffectOutput {
        return_fields: vec![],
        pre_return: vec![lua_raw_stmt(code)],
        config_vars: vec![],
        message,
        colour: Some(lua_raw_expr("G.C.FILTER")),

        segment_id: None,
    }
}

// ---------------------------------------------------------------------------
// change_rank_variable
// ---------------------------------------------------------------------------

/// Change Rank Variable: changes a rank-type user variable.
pub fn change_rank_variable(effect: &EffectDef, ctx: &mut CompileContext) -> EffectOutput {
    change_card_variable(effect, ctx, "rank")
}

/// Change Suit Variable: changes a suit-type user variable.
pub fn change_suit_variable(effect: &EffectDef, ctx: &mut CompileContext) -> EffectOutput {
    change_card_variable(effect, ctx, "suit")
}

const RANK_ORDER: &[&str] = &[
    "2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K", "A",
];
const SUIT_ORDER: &[&str] = &["Spades", "Hearts", "Diamonds", "Clubs"];
const HAND_ORDER: &[&str] = &[
    "High Card",
    "Pair",
    "Two Pair",
    "Three of a Kind",
    "Straight",
    "Flush",
    "Full House",
    "Four of a Kind",
    "Five of a Kind",
    "Straight Flush",
    "Flush House",
    "Flush Five",
];

/// Checkbox pools are positional in the editor, and older projects serialize
/// them as JSON strings. Also accept named values to preserve custom options.
fn selected_variable_pool(effect: &EffectDef, parameter: &str, order: &[&str]) -> Option<String> {
    let values = match effect.params.get(parameter)? {
        ParamValue::Typed(value) => value.value.as_array()?.clone(),
        ParamValue::Str(value) => serde_json::from_str::<Vec<serde_json::Value>>(value).ok()?,
        _ => return None,
    };
    let selected: Vec<String> = values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            let selected = match value {
                serde_json::Value::Bool(true) => order.get(index).copied(),
                serde_json::Value::String(value) if !value.trim().is_empty() => Some(value.trim()),
                _ => None,
            }?;
            Some(lua_str(selected).to_string())
        })
        .collect();
    Some(format!("{{{}}}", selected.join(", ")))
}

fn round_variable_source(name: &str, field: &str) -> String {
    format!(
        "(G and G.GAME and G.GAME.current_round and G.GAME.current_round[{}])",
        lua_str(format!("{name}_{field}"))
    )
}

fn typed_variable_source(ctx: &CompileContext, name: &str, field: &str) -> String {
    if ctx.user_var_is_global(name) {
        ctx.user_var_expr(name).to_string()
    } else {
        round_variable_source(name, field)
    }
}

fn global_typed_assignment(ctx: &CompileContext, name: &str, guard: &str, value: &str) -> String {
    let path = ctx.user_var_path(name);
    if ctx.user_var_is_persistent(name) {
        format!("if {guard} and JF_GLOBALS then {path} = {value} end")
    } else {
        format!("if {guard} and G and G.GAME then\n    G.GAME.jf_global_vars = G.GAME.jf_global_vars or {{}}\n    {path} = {value}\nend")
    }
}

fn change_card_variable(effect: &EffectDef, ctx: &CompileContext, property: &str) -> EffectOutput {
    let Some(variable) = key_string_param(effect, &["variable_name", "variableName", "variable"])
    else {
        return EffectOutput::default();
    };
    let mode = key_string_param(effect, &["change_type", "changeType"]).unwrap_or("random");
    let pool_parameter = format!("{property}_pool");
    let specific_parameter = format!("specific_{property}");
    let selection = match mode {
        "random" => format!(
            "local candidates = {{}}\nfor _, playing_card in ipairs((G and G.playing_cards) or {{}}) do\n    if playing_card.base and playing_card.base.{value} and not (SMODS and SMODS.has_no_{property} and SMODS.has_no_{property}(playing_card)) then candidates[#candidates + 1] = playing_card end\nend\nif #candidates > 0 then\n    local source = pseudorandom_element(candidates, pseudoseed({seed}))\n    selected_value = source.base.{value}\n    {id}\nend",
            value = if property == "rank" { "value" } else { "suit" },
            id = if property == "rank" { "selected_id = source.base.id" } else { "" },
            seed = lua_str(variable),
        ),
        "pool" => {
            let order = if property == "rank" { RANK_ORDER } else { SUIT_ORDER };
            let Some(pool) = selected_variable_pool(effect, &pool_parameter, order) else {
                return EffectOutput::default();
            };
            format!("local candidates = {pool}\nif #candidates > 0 then selected_value = pseudorandom_element(candidates, pseudoseed({seed})) end", seed = lua_str(variable))
        }
        "specific" => {
            let Some(value) = effect.params.get(&specific_parameter).or_else(|| effect.params.get(property)) else {
                return EffectOutput::default();
            };
            let Some(name) = value.as_str().map(str::trim).filter(|value| !value.is_empty()) else {
                return EffectOutput::default();
            };
            if matches!(value, ParamValue::Typed(value) if crate::compiler::values::is_user_variable_type(&value.value_type)) {
                let source = typed_variable_source(ctx, name, "card");
                if ctx.user_var_is_global(name) {
                    format!("selected_value = {source}")
                } else {
                    format!("local source = {source}\nselected_value = source and source.{property}\n{id}", id = if property == "rank" { "selected_id = source and source.id" } else { "" })
                }
            } else {
                format!("selected_value = {}", lua_str(name))
            }
        }
        "scored_card" | "held_card" | "card_held_in_hand" | "discarded_card" | "destroyed_card" | "added_card" => {
            let source = match mode {
                "destroyed_card" => "(context and (context.other_card or context.destroy_card or (context.removed and context.removed[1])))",
                "added_card" => "(context and (context.other_card or context.card or (context.cards and context.cards[1])))",
                _ => "(context and context.other_card)",
            };
            format!("local source = {source}\nif source and source.base and not (SMODS and SMODS.has_no_{property} and SMODS.has_no_{property}(source)) then\n    selected_value = source.base.{value}\n    {id}\nend",
                value = if property == "rank" { "value" } else { "suit" },
                id = if property == "rank" { "selected_id = source.base.id" } else { "" },
            )
        }
        _ => return EffectOutput::default(),
    };
    let normalize = if property == "rank" {
        "local rank_names = {A = 'Ace', K = 'King', Q = 'Queen', J = 'Jack'}\nselected_value = rank_names[selected_value] or selected_value\nlocal rank_ids = {Ace = 14, King = 13, Queen = 12, Jack = 11, ['2'] = 2, ['3'] = 3, ['4'] = 4, ['5'] = 5, ['6'] = 6, ['7'] = 7, ['8'] = 8, ['9'] = 9, ['10'] = 10}\nselected_id = selected_id or rank_ids[selected_value] or (SMODS and SMODS.Ranks and SMODS.Ranks[selected_value] and SMODS.Ranks[selected_value].id)\n"
    } else {
        ""
    };
    let field = lua_str(format!("{variable}_card"));
    let guard = if property == "rank" {
        "type(selected_value) == 'string' and selected_value ~= '' and selected_id ~= nil"
    } else {
        "type(selected_value) == 'string' and selected_value ~= ''"
    };
    let assignment = if ctx.user_var_is_global(variable) {
        global_typed_assignment(ctx, variable, guard, "selected_value")
    } else {
        let assign = if property == "rank" {
            "target.rank = selected_value\ntarget.id = selected_id"
        } else {
            "target.suit = selected_value"
        };
        format!("if {guard} and G and G.GAME and G.GAME.current_round then\n    local target = G.GAME.current_round[{field}]\n    if type(target) ~= 'table' then target = {{}}; G.GAME.current_round[{field}] = target end\n    {assign}\nend")
    };
    variable_effect_output(effect, format!(
        "do\nlocal selected_value, selected_id\n{selection}\n{normalize}{assignment}\nend"
    ))
}

fn variable_effect_output(effect: &EffectDef, code: String) -> EffectOutput {
    EffectOutput {
        pre_return: vec![lua_raw_stmt(code)],
        message: get_str(effect, "customMessage").map(lua_str),
        colour: Some(lua_raw_expr("G.C.FILTER")),
        ..EffectOutput::default()
    }
}

/// Change Poker Hand Variable: changes a poker-hand-type user variable.
pub fn change_poker_hand_variable(effect: &EffectDef, ctx: &mut CompileContext) -> EffectOutput {
    let Some(variable) = key_string_param(effect, &["variable_name", "variableName", "variable"])
    else {
        return EffectOutput::default();
    };
    let mode = key_string_param(effect, &["change_type", "changeType"]).unwrap_or("random");
    let selection = match mode {
        "random" | "pool" => {
            let source = if mode == "pool" {
                let Some(pool) = selected_variable_pool(effect, "pokerhand_pool", HAND_ORDER)
                else {
                    return EffectOutput::default();
                };
                pool
            } else {
                "(function() local keys = {}; for key, _ in pairs(hands) do keys[#keys + 1] = key end; return (G and G.handlist) or keys end)()".into()
            };
            format!("local candidates = {{}}\nfor _, hand_name in ipairs({source}) do\n    if hands[hand_name] and hands[hand_name].visible then candidates[#candidates + 1] = hand_name end\nend\nif #candidates > 0 then selected = pseudorandom_element(candidates, pseudoseed({seed})) end", seed = lua_str(variable))
        }
        "most_played" | "least_played" => {
            let (initial, operator) = if mode == "most_played" {
                ("-math.huge", ">")
            } else {
                ("math.huge", "<")
            };
            format!("local tally = {initial}\nfor _, hand_name in ipairs((G and G.handlist) or {{}}) do\n    local hand = hands[hand_name]\n    if hand and hand.visible and (hand.played or 0) {operator} tally then selected = hand_name; tally = hand.played or 0 end\nend")
        }
        "specific" => {
            let Some(value) = effect
                .params
                .get("specific_pokerhand")
                .or_else(|| effect.params.get("specific_poker_hand"))
            else {
                return EffectOutput::default();
            };
            let Some(name) = value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                return EffectOutput::default();
            };
            if matches!(value, ParamValue::Typed(value) if crate::compiler::values::is_user_variable_type(&value.value_type))
            {
                format!("selected = {}", typed_variable_source(ctx, name, "hand"))
            } else {
                format!("selected = {}", lua_str(name))
            }
        }
        _ => return EffectOutput::default(),
    };
    let guard = "type(selected) == 'string' and hands[selected]";
    let assignment = if ctx.user_var_is_global(variable) {
        global_typed_assignment(ctx, variable, guard, "selected")
    } else {
        format!("if {guard} and G and G.GAME and G.GAME.current_round then G.GAME.current_round[{field}] = selected end", field = lua_str(format!("{variable}_hand")))
    };
    variable_effect_output(effect, format!(
        "do\nlocal hands = (G and G.GAME and G.GAME.hands) or {{}}\nlocal selected\n{selection}\n{assignment}\nend"
    ))
}
