-- Purpose: Snapshot for `user_variables_use` (consumable) codegen output.

SMODS.Consumable {
    key = 'variable_test',
    set = 'Tarot',
    pos = {
        x = 0,
        y = 0
    },
    config = {
        extra = {
            internal_variable_value_value0 = 0,
            var_amount0 = 2,
            amount = 4,
            chosen = 'c_fool',
            label = 'Ready',
            suitvar = 'Hearts',
            rankvar = 'K',
            handvar = 'Flush'
        }
    },
    loc_txt = {
        ['name'] = 'Variable Test',
        ['text'] = {
            [1] = 'Uses and updates variables when consumed.'
        }
    },
    atlas = 'Consumables',
    loc_vars = function(self, info_queue, card)
        return {
            vars = {
                card and card.ability and card.ability.extra and card.ability.extra['amount'] or self and self.config and self.config.extra and self.config.extra['amount'] or 4,
                card and card.ability and card.ability.extra and card.ability.extra['chosen'] or self and self.config and self.config.extra and self.config.extra['chosen'] or 'c_fool',
                card and card.ability and card.ability.extra and card.ability.extra['label'] or self and self.config and self.config.extra and self.config.extra['label'] or 'Ready',
                localize(G and G.GAME and G.GAME.current_round and G.GAME.current_round['suitvar_card'] and G.GAME.current_round['suitvar_card'].suit or card and card.ability and card.ability.extra and card.ability.extra['suitvar'] or self and self.config and self.config.extra and self.config.extra['suitvar'] or 'Hearts', 'suits_singular'),
                localize(G and G.GAME and G.GAME.current_round and G.GAME.current_round['rankvar_card'] and G.GAME.current_round['rankvar_card'].rank or card and card.ability and card.ability.extra and card.ability.extra['rankvar'] or self and self.config and self.config.extra and self.config.extra['rankvar'] or 'K', 'ranks'),
                localize(G and G.GAME and G.GAME.current_round and G.GAME.current_round['handvar_hand'] or card and card.ability and card.ability.extra and card.ability.extra['handvar'] or self and self.config and self.config.extra and self.config.extra['handvar'] or 'Flush', 'poker_hands'),
                self.config.extra.internal_variable_value_value0,
                self.config.extra.var_amount0
            }
        }
    end,
    set_ability = function(self, card, initial)
        if not (G and G.GAME and G.GAME.current_round) then return end
        G.GAME.current_round['suitvar_card'] = { suit = 'Hearts' }
        G.GAME.current_round['rankvar_card'] = {
            rank = 'K',
            id = 13
        }
        G.GAME.current_round['handvar_hand'] = 'Flush'
    end,
    use = function(self, card, area, copier)
        if card.ability.extra.amount > card.ability.extra.internal_variable_value_value0 and card.ability.extra.chosen == 'c_fool' and card.ability.extra.label == 'Ready' and (G and G.GAME and G.GAME.current_round and G.GAME.current_round['suitvar_card'] and G.GAME.current_round['suitvar_card'].suit == 'Hearts') and (function() local selected = G and G.GAME and G.GAME.current_round and G.GAME.current_round['rankvar_card']; if type(selected) == 'string' then local rank_names = {A = 'Ace', K = 'King', Q = 'Queen', J = 'Jack'}; local rank_ids = {Ace = 14, King = 13, Queen = 12, Jack = 11, ['2'] = 2, ['3'] = 3, ['4'] = 4, ['5'] = 5, ['6'] = 6, ['7'] = 7, ['8'] = 8, ['9'] = 9, ['10'] = 10}; local normalized = rank_names[selected] or selected; selected = {rank = normalized, id = rank_ids[normalized] or (SMODS and SMODS.Ranks and SMODS.Ranks[normalized] and SMODS.Ranks[normalized].id)} end; if type(selected) ~= 'table' then return false end; local expected = 13; return (expected ~= nil and selected.id ~= nil and selected.id == expected) or (selected.id == nil and (selected.rank == 'King' or selected.rank == 'King')) end)() and (G and G.GAME and G.GAME.current_round and G.GAME.current_round['handvar_hand']) == 'Flush' then
            SMODS.calculate_effect({
                func = function()
                    card.ability.extra.amount = (card.ability.extra.amount) + card.ability.extra.var_amount0
                    return true
                end,
                colour = G.C.GREEN,
                extra = {
                    dollars = card.ability.extra.amount,
                    colour = G.C.MONEY
                }
            }, card)
        end
    end,
    can_use = function(self, card)
        return card.ability.extra.amount > card.ability.extra.internal_variable_value_value0 and card.ability.extra.chosen == 'c_fool' and card.ability.extra.label == 'Ready' and (G and G.GAME and G.GAME.current_round and G.GAME.current_round['suitvar_card'] and G.GAME.current_round['suitvar_card'].suit == 'Hearts') and (function() local selected = G and G.GAME and G.GAME.current_round and G.GAME.current_round['rankvar_card']; if type(selected) == 'string' then local rank_names = {A = 'Ace', K = 'King', Q = 'Queen', J = 'Jack'}; local rank_ids = {Ace = 14, King = 13, Queen = 12, Jack = 11, ['2'] = 2, ['3'] = 3, ['4'] = 4, ['5'] = 5, ['6'] = 6, ['7'] = 7, ['8'] = 8, ['9'] = 9, ['10'] = 10}; local normalized = rank_names[selected] or selected; selected = {rank = normalized, id = rank_ids[normalized] or (SMODS and SMODS.Ranks and SMODS.Ranks[normalized] and SMODS.Ranks[normalized].id)} end; if type(selected) ~= 'table' then return false end; local expected = 13; return (expected ~= nil and selected.id ~= nil and selected.id == expected) or (selected.id == nil and (selected.rank == 'King' or selected.rank == 'King')) end)() and (G and G.GAME and G.GAME.current_round and G.GAME.current_round['handvar_hand']) == 'Flush'
    end
}
