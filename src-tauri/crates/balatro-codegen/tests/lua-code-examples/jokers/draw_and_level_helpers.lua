-- Purpose: Snapshot for `draw_and_level_helpers` (joker) codegen output.

SMODS.Joker {
    key = 'j_draw_and_level_helpers',
    config = {
        extra = {
            card_draw0 = 2,
            level_amount0 = 1
        }
    },
    loc_txt = {
        ['name'] = 'Draw And Level Helpers',
        ['text'] = {
            [1] = 'Uses SMODS helper functions.'
        }
    },
    pos = {
        x = 0,
        y = 0
    },
    cost = 4,
    rarity = 1,
    blueprint_compat = true,
    eternal_compat = true,
    perishable_compat = true,
    unlocked = true,
    discovered = true,
    atlas = 'Joker',
    loc_vars = function(self, info_queue, card)
        return {
            vars = {
                self.config.extra.card_draw0,
                self.config.extra.level_amount0
            }
        }
    end,
    calculate = function(self, card, context)
        if context.joker_main then
            do
                SMODS.draw_cards(card.ability.extra.card_draw0)
                local levelled_hand0 = false
                local level_hand0 = nil
                level_hand0 = context.scoring_name
                if not level_hand0 and context.full_hand and #context.full_hand > 0 then
                    level_hand0 = G.FUNCS.get_poker_hand_info(context.full_hand)
                end
                level_hand0 = level_hand0 or G.GAME.last_hand_played or 'High Card'
                if type(level_hand0) == 'string' and G.GAME.hands[level_hand0] then
                    SMODS.smart_level_up_hand(context.blueprint_card or card, level_hand0, false, card.ability.extra.level_amount0)
                    levelled_hand0 = true
                end
                return {
                    message = "+"..tostring(card.ability.extra.card_draw0)..' Cards Drawn',
                    colour = G.C.BLUE,
                    extra = {
                        message = levelled_hand0 and localize('k_level_up_ex'),
                        colour = G.C.GREEN
                    }
                }
            end
        end
    end
}
