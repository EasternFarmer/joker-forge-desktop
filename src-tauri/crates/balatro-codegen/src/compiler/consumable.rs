use super::colors::normalize_hex_colour;
use super::context::CompileContext;
use super::{build_shared_calculate_function, build_shared_loc_vars, compile_rules};
use crate::lua_ast::*;
use crate::types::*;

/// Compile a consumable definition into a Lua chunk.
pub fn compile_consumable(consumable: &ConsumableDef, mod_prefix: &str) -> Chunk {
    let mut ctx = CompileContext::new(
        ObjectType::Consumable,
        mod_prefix.to_string(),
        consumable.key.clone(),
        false,
    );
    ctx.set_user_vars(consumable.user_variables.clone());
    ctx.set_description_variables(consumable.description_variables.clone());

    let mut rules = consumable.rules.clone();
    for rule in &mut rules {
        if rule.trigger == "consumable_used" {
            rule.trigger = "card_used".into();
        }
    }
    let rule_outputs = compile_rules(&rules, &mut ctx);
    let table = build_consumable_table(consumable, &ctx, &rule_outputs);

    let smods_call = Stmt::ExprStmt(lua_table_call(
        lua_path(&["SMODS", "Consumable"]),
        match table {
            Expr::Table(entries) => entries,
            _ => vec![],
        },
    ));

    Chunk {
        stmts: vec![lua_comment(format!(" {}", consumable.name)), smods_call],
    }
}

/// Compile a ConsumableType definition into a Lua chunk.
pub fn compile_consumable_type(ct: &ConsumableTypeDef, mod_prefix: &str) -> Chunk {
    let primary = normalize_hex_colour(&ct.primary_colour).unwrap_or_else(|| "666666".to_string());
    let secondary =
        normalize_hex_colour(&ct.secondary_colour).unwrap_or_else(|| "666666".to_string());

    let mut entries: Vec<TableEntry> = Vec::new();
    entries.push(kv("key", lua_str(&ct.key)));
    entries.push(kv(
        "primary_colour",
        lua_call("HEX", vec![lua_str(&primary)]),
    ));
    entries.push(kv(
        "secondary_colour",
        lua_call("HEX", vec![lua_str(&secondary)]),
    ));
    entries.push(kv(
        "collection_rows",
        lua_table_raw(vec![
            TableEntry::Value(lua_int(ct.collection_rows.0 as i64)),
            TableEntry::Value(lua_int(ct.collection_rows.1 as i64)),
        ]),
    ));

    if let Some(default) = &ct.default_card {
        entries.push(kv("default", lua_str(default)));
    }
    if let Some(rate) = ct.shop_rate {
        entries.push(kv("shop_rate", lua_num(rate)));
    }

    // loc_txt
    let collection_name = ct
        .collection_name
        .clone()
        .unwrap_or_else(|| format!("{} Cards", ct.name));
    entries.push(TableEntry::KeyValue(
        "loc_txt".to_string(),
        lua_table(vec![
            ("name", lua_str(&ct.name)),
            ("collection", lua_str(&collection_name)),
        ]),
    ));

    // cards table, populated by consumables referencing this set
    let cards_key = format!("c_{}_{}", mod_prefix, ct.key);
    let _ = cards_key; // Will be linked externally
    entries.push(kv("cards", lua_table_raw(vec![])));

    let smods_call = Stmt::ExprStmt(lua_table_call(
        lua_path(&["SMODS", "ConsumableType"]),
        entries,
    ));

    Chunk {
        stmts: vec![
            lua_comment(format!(" Consumable Set: {}", ct.name)),
            smods_call,
        ],
    }
}

fn build_consumable_table(
    consumable: &ConsumableDef,
    ctx: &CompileContext,
    rule_outputs: &[super::RuleOutput],
) -> Expr {
    let mut entries: Vec<TableEntry> = Vec::new();

    entries.push(kv("key", lua_str(&consumable.key)));
    entries.push(kv("set", lua_str(&consumable.set)));

    // Position
    entries.push(TableEntry::KeyValue(
        "pos".to_string(),
        lua_table(vec![
            ("x", lua_int(consumable.pos.x as i64)),
            ("y", lua_int(consumable.pos.y as i64)),
        ]),
    ));

    // Config table
    let config_extra = ctx.build_config_extra_table();
    if !config_extra.is_empty() {
        entries.push(TableEntry::KeyValue(
            "config".to_string(),
            lua_table_raw(vec![TableEntry::KeyValue(
                "extra".to_string(),
                lua_table_raw(config_extra),
            )]),
        ));
    }

    // loc_txt
    let text_entries: Vec<TableEntry> = consumable
        .description
        .iter()
        .enumerate()
        .map(|(i, d)| TableEntry::IndexValue(lua_int(i as i64 + 1), lua_str(d)))
        .collect();

    entries.push(TableEntry::KeyValue(
        "loc_txt".to_string(),
        lua_table_raw(vec![
            TableEntry::IndexValue(lua_str("name"), lua_str(&consumable.name)),
            TableEntry::IndexValue(lua_str("text"), lua_table_raw(text_entries)),
        ]),
    ));

    // Optional properties
    if let Some(cost) = consumable.cost {
        entries.push(kv("cost", lua_int(cost as i64)));
    }
    if let Some(unlocked) = consumable.unlocked {
        entries.push(kv("unlocked", lua_bool(unlocked)));
    }
    if let Some(discovered) = consumable.discovered {
        entries.push(kv("discovered", lua_bool(discovered)));
    }
    if let Some(hidden) = consumable.hidden {
        entries.push(kv("hidden", lua_bool(hidden)));
    }
    if let Some(can_repeat_soul) = consumable.can_repeat_soul {
        entries.push(kv("can_repeat_soul", lua_bool(can_repeat_soul)));
    }

    entries.push(kv("atlas", lua_str(&consumable.atlas)));

    if let Some(soul) = &consumable.soul_pos {
        entries.push(TableEntry::KeyValue(
            "soul_pos".to_string(),
            lua_table(vec![
                ("x", lua_int(soul.x as i64)),
                ("y", lua_int(soul.y as i64)),
            ]),
        ));
    }

    // loc_vars
    if let Some(f) = build_shared_loc_vars(ctx, rule_outputs) {
        entries.push(TableEntry::KeyValue("loc_vars".to_string(), f));
    }

    if let Some(f) = build_set_ability(consumable) {
        entries.push(TableEntry::KeyValue("set_ability".to_string(), f));
    }

    // calculate function (non-"card_used" triggers)
    if let Some(f) = build_shared_calculate_function(rule_outputs, ctx) {
        entries.push(TableEntry::KeyValue("calculate".to_string(), f));
    }

    // use function (card_used triggers)
    if let Some(f) = build_use_function(rule_outputs, ctx) {
        entries.push(TableEntry::KeyValue("use".to_string(), f));
    }

    // can_use function
    let can_use_fn = build_can_use_function(rule_outputs, ctx);
    entries.push(TableEntry::KeyValue("can_use".to_string(), can_use_fn));

    lua_table_raw(entries)
}

/// Build the `use` function for consumables.
/// Compiles rules with trigger == "card_used".
fn build_use_function(rule_outputs: &[super::RuleOutput], _ctx: &CompileContext) -> Option<Expr> {
    let use_rules: Vec<&super::RuleOutput> = rule_outputs
        .iter()
        .filter(|r| r.trigger == "card_used" && !r.effect_stmts.is_empty())
        .collect();

    if use_rules.is_empty() {
        return None;
    }

    let mut body: Vec<Stmt> = Vec::new();
    if use_rules
        .iter()
        .flat_map(|r| r.effect_stmts.iter())
        .any(stmt_references_used_card)
    {
        body.push(Stmt::Local(
            "used_card".to_string(),
            Some(lua_or(lua_ident("copier"), lua_ident("card"))),
        ));
    }

    for ro in &use_rules {
        // Steamodded calls a consumable's use hook directly and ignores its
        // return value. Resolve calculation effects here so func callbacks,
        // dollars and messages are actually applied when the card is used.
        let stmts = resolve_use_effect_stmts(ro.effect_stmts.clone());
        let stmts = super::wrap_rule_segment(&ro.rule_id, stmts);
        if let Some(condition) = &ro.condition_expr {
            body.push(Stmt::If {
                branches: vec![(condition.clone(), stmts)],
                else_body: None,
            });
        } else {
            body.push(Stmt::DoBlock(stmts));
        }
    }

    Some(Expr::Function {
        params: vec!["self".into(), "card".into(), "area".into(), "copier".into()],
        body,
    })
}

/// Build the `can_use` function for consumables.
fn build_can_use_function(rule_outputs: &[super::RuleOutput], _ctx: &CompileContext) -> Expr {
    let use_rules: Vec<&super::RuleOutput> = rule_outputs
        .iter()
        .filter(|r| r.trigger == "card_used")
        .collect();

    // Collect conditions from use rules
    let conditions: Vec<Expr> = use_rules
        .iter()
        .filter_map(|ro| ro.condition_expr.clone())
        .collect();

    let body = if conditions.is_empty() || use_rules.iter().any(|ro| ro.condition_expr.is_none()) {
        vec![lua_return(lua_bool(true))]
    } else {
        let combined = lua_or_chain(conditions);
        vec![lua_return(combined)]
    };

    Expr::Function {
        params: vec!["self".into(), "card".into()],
        body,
    }
}

fn resolve_use_effect_stmts(stmts: Vec<Stmt>) -> Vec<Stmt> {
    stmts
        .into_iter()
        .map(|stmt| match stmt {
            Stmt::Return(Some(effect @ Expr::Table(_))) => lua_expr_stmt(lua_call(
                "SMODS.calculate_effect",
                vec![effect, lua_ident("card")],
            )),
            Stmt::If {
                branches,
                else_body,
            } => Stmt::If {
                branches: branches
                    .into_iter()
                    .map(|(condition, body)| (condition, resolve_use_effect_stmts(body)))
                    .collect(),
                else_body: else_body.map(resolve_use_effect_stmts),
            },
            Stmt::ForRange {
                var,
                start,
                stop,
                step,
                body,
            } => Stmt::ForRange {
                var,
                start,
                stop,
                step,
                body: resolve_use_effect_stmts(body),
            },
            Stmt::ForIn {
                vars,
                iterators,
                body,
            } => Stmt::ForIn {
                vars,
                iterators,
                body: resolve_use_effect_stmts(body),
            },
            Stmt::DoBlock(body) => Stmt::DoBlock(resolve_use_effect_stmts(body)),
            // Do not descend into expression functions: their returns belong
            // to callbacks such as func, not the consumable's use hook.
            stmt => stmt,
        })
        .collect()
}

/// Typed card variables use the current-round storage shared by their
/// conditions and change-variable effects, rather than ability.extra.
fn build_set_ability(consumable: &ConsumableDef) -> Option<Expr> {
    let mut body = Vec::new();
    for variable in &consumable.user_variables {
        if variable.is_global {
            continue;
        }
        let initial = variable.initial_value.to_string_lossy();
        let (field, value) = match variable.var_type {
            UserVarType::Suit => (
                format!("{}_card", variable.name),
                lua_table(vec![("suit", lua_str(&initial))]),
            ),
            UserVarType::Rank => (
                format!("{}_card", variable.name),
                lua_table(vec![
                    ("rank", lua_str(&initial)),
                    ("id", lua_int(super::rank_to_id(&initial))),
                ]),
            ),
            UserVarType::PokerHand => (format!("{}_hand", variable.name), lua_str(&initial)),
            _ => continue,
        };
        body.push(lua_assign(
            lua_index(lua_raw_expr("G.GAME.current_round"), lua_str(field)),
            value,
        ));
    }
    if body.is_empty() {
        return None;
    }
    body.insert(
        0,
        lua_raw_stmt("if not (G and G.GAME and G.GAME.current_round) then return end"),
    );
    Some(Expr::Function {
        params: vec!["self".into(), "card".into(), "initial".into()],
        body,
    })
}

fn kv(key: &str, val: Expr) -> TableEntry {
    TableEntry::KeyValue(key.to_string(), val)
}

fn stmt_references_used_card(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Raw(s) => s.contains("used_card"),
        Stmt::Local(_, init) => init.as_ref().is_some_and(expr_references_used_card),
        Stmt::Assign(lhs, rhs) => expr_references_used_card(lhs) || expr_references_used_card(rhs),
        Stmt::MultiAssign(lhs, rhs) => {
            lhs.iter().any(expr_references_used_card) || rhs.iter().any(expr_references_used_card)
        }
        Stmt::If {
            branches,
            else_body,
        } => {
            branches.iter().any(|(cond, body)| {
                expr_references_used_card(cond) || body.iter().any(stmt_references_used_card)
            }) || else_body
                .as_ref()
                .is_some_and(|body| body.iter().any(stmt_references_used_card))
        }
        Stmt::Return(expr) => expr.as_ref().is_some_and(expr_references_used_card),
        Stmt::ExprStmt(expr) => expr_references_used_card(expr),
        Stmt::ForRange {
            start,
            stop,
            step,
            body,
            ..
        } => {
            expr_references_used_card(start)
                || expr_references_used_card(stop)
                || step.as_ref().is_some_and(expr_references_used_card)
                || body.iter().any(stmt_references_used_card)
        }
        Stmt::ForIn {
            iterators, body, ..
        } => {
            iterators.iter().any(expr_references_used_card)
                || body.iter().any(stmt_references_used_card)
        }
        Stmt::DoBlock(body) => body.iter().any(stmt_references_used_card),
        Stmt::Comment(_) | Stmt::Blank | Stmt::SegmentStart(_) | Stmt::SegmentEnd(_) => false,
    }
}

fn expr_references_used_card(expr: &Expr) -> bool {
    match expr {
        Expr::FieldBinding(inner, _) | Expr::Segment(inner, _) => expr_references_used_card(inner),
        Expr::Raw(s) => s.contains("used_card"),
        Expr::Ident(s) => s == "used_card",
        Expr::Field(base, key) => key == "used_card" || expr_references_used_card(base),
        Expr::Index(base, key) => expr_references_used_card(base) || expr_references_used_card(key),
        Expr::BinOp(lhs, _, rhs) => {
            expr_references_used_card(lhs) || expr_references_used_card(rhs)
        }
        Expr::UnaryOp(_, inner) => expr_references_used_card(inner),
        Expr::Call(func, args) => {
            expr_references_used_card(func) || args.iter().any(expr_references_used_card)
        }
        Expr::MethodCall(obj, _, args) => {
            expr_references_used_card(obj) || args.iter().any(expr_references_used_card)
        }
        Expr::Table(entries) | Expr::TableCall(_, entries) => {
            entries.iter().any(table_entry_references_used_card)
                || matches!(expr, Expr::TableCall(func, _) if expr_references_used_card(func))
        }
        Expr::Function { body, .. } => body.iter().any(stmt_references_used_card),
        Expr::Nil
        | Expr::Bool(_)
        | Expr::Int(_)
        | Expr::Number(_)
        | Expr::Str(_)
        | Expr::LongStr(_) => false,
    }
}

fn table_entry_references_used_card(entry: &TableEntry) -> bool {
    match entry {
        TableEntry::KeyValue(_, expr) | TableEntry::Value(expr) => expr_references_used_card(expr),
        TableEntry::IndexValue(k, v) => {
            expr_references_used_card(k) || expr_references_used_card(v)
        }
        TableEntry::Comment(_) | TableEntry::SegmentStart(_) | TableEntry::SegmentEnd(_) => false,
    }
}
