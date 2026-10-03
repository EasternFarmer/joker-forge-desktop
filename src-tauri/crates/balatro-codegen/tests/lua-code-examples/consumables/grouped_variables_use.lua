-- Purpose: Snapshot for `grouped_variables_use` (consumable) codegen output.

SMODS.Consumable {
    key = 'grouped_variable_test',
    set = 'Tarot',
    pos = {
        x = 0,
        y = 0
    },
    config = {
        extra = {
            var_amount0 = 1,
            numerator_0 = 1,
            odds_0 = 1,
            var_amount1 = 2,
            loop_count_0 = 3,
            amount = 4
        }
    },
    loc_txt = {
        ['name'] = 'Grouped Variable Test',
        ['text'] = {
            [1] = 'Uses variables in chance groups and repeated effects.'
        }
    },
    atlas = 'Consumables',
    loc_vars = function(self, info_queue, card)
        local new_numerator0, new_denominator0 = SMODS.get_probability_vars(card, self.config.extra.numerator_0, self.config.extra.odds_0, 'c_modprefix_grouped_variable_test')
        return {
            vars = {
                card and card.ability and card.ability.extra and card.ability.extra['amount'] or self and self.config and self.config.extra and self.config.extra['amount'] or 4,
                self.config.extra.var_amount0,
                self.config.extra.var_amount1,
                self.config.extra.loop_count_0,
                new_numerator0,
                new_denominator0
            }
        }
    end,
    use = function(self, card, area, copier)
        do
            if SMODS.pseudorandom_probability(card, 'group0', card.ability.extra.numerator_0, card.ability.extra.odds_0, 'c_modprefix_grouped_variable_test', false) then
                SMODS.calculate_effect({
                    func = function()
                        card.ability.extra.amount = (card.ability.extra.amount) + card.ability.extra.var_amount0
                        return true
                    end,
                    colour = G.C.GREEN
                }, card)
            end
            for i = 1, card.ability.extra.loop_count_0 do
                SMODS.calculate_effect({
                    func = function()
                        card.ability.extra.amount = (card.ability.extra.amount) + card.ability.extra.var_amount1
                        return true
                    end,
                    colour = G.C.GREEN
                }, card)
            end
        end
        do
            SMODS.calculate_effect({
                dollars = card.ability.extra.amount,
                colour = G.C.MONEY
            }, card)
        end
    end,
    can_use = function(self, card)
        return true
    end
}
