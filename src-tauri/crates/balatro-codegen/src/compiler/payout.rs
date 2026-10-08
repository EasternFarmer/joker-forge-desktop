
use super::{compile_blind_reward_amount, context::CompileContext, effects::EffectOutput};
use crate::lua_ast::*;
use crate::types::{EffectDef, ObjectType};

pub(super) fn is_reward(effect: &EffectDef, ctx: &CompileContext, trigger: &str) -> bool {
    ctx.object_type == ObjectType::Joker
        && effect.effect_type == "blind_reward"
        && matches!(trigger, "round_end" | "boss_defeated")
}

pub(super) fn compile_grouped_reward(effect: &EffectDef, ctx: &mut CompileContext) -> EffectOutput {
    let amount = compile_blind_reward_amount(effect, ctx);
    EffectOutput {
        pre_return: vec![lua_if(
            lua_ident("jf_payout_evaluate"),
            vec![lua_assign(
                lua_raw_expr("card.ability.jf_blind_payout.total"),
                lua_add(
                    lua_raw_expr("card.ability.jf_blind_payout.total"),
                    lua_call("math.max", vec![amount, lua_int(0)]),
                ),
            )],
        )],
        ..Default::default()
    }
}

pub(super) fn begin_round() -> Stmt {
    lua_raw_stmt(
        r#"local jf_payout_evaluate = false
if context.end_of_round and context.main_eval and context.game_over == false
    and not context.blueprint and not context.blueprint_card
    and not context.retrigger_joker and not context.retrigger_joker_check then
    local jf_payout_owner = card.unique_val__saved_ID or card.ID or card.sort_id
    if jf_payout_owner then
        local jf_payout = card.ability.jf_blind_payout
        if type(jf_payout) ~= 'table' or jf_payout.owner ~= jf_payout_owner
            or jf_payout.round ~= G.GAME.round then
            jf_payout = {owner = jf_payout_owner, round = G.GAME.round, total = 0}
            card.ability.jf_blind_payout = jf_payout
        end
        if not jf_payout.evaluated then
            jf_payout.evaluated = true
            jf_payout_evaluate = true
        end
    end
end"#,
    )
}

pub(super) fn read_total() -> Expr {
    lua_raw_expr(
        r#"(function()
    local jf_payout_owner = card.unique_val__saved_ID or card.ID or card.sort_id
    local jf_payout = card.ability.jf_blind_payout
    if jf_payout_owner and type(jf_payout) == 'table'
        and jf_payout.owner == jf_payout_owner and jf_payout.round == G.GAME.round then
        return jf_payout.total or 0
    end
    return 0
end)()"#,
    )
}

pub(super) fn cashout_context() -> Stmt {
    lua_raw_stmt(
        r#"local jf_last_hand = SMODS.last_hand or {}
local context = {
    end_of_round = true, main_eval = true, game_over = false,
    scoring_name = jf_last_hand.scoring_name or G.GAME.last_hand_played,
    scoring_hand = jf_last_hand.scoring_hand or {},
    full_hand = jf_last_hand.full_hand or {}
}"#,
    )
}
