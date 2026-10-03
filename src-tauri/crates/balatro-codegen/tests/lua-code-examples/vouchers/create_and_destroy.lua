-- Purpose: Snapshot for `create_and_destroy` (voucher) codegen output.

SMODS.Voucher {
    key = 'v_create_and_destroy',
    pos = {
        x = 0,
        y = 0
    },
    config = {
        extra = { create_cards_count0 = 1 }
    },
    loc_txt = {
        ['name'] = 'Create And Destroy',
        ['text'] = {
            [1] = 'Voucher fixture with creation/destruction effects.'
        }
    },
    cost = 10,
    unlocked = true,
    discovered = true,
    no_collection = false,
    can_repeat_soul = false,
    atlas = 'Voucher',
    loc_vars = function(self, info_queue, card)
        return {
            vars = {
                self.config.extra.create_cards_count0
            }
        }
    end,
    redeem = function(self, card)
        local redeem_result
        do
            local apply_rule = function()
                local created_playing_cards0 = {}
                local created_playing_card0 = SMODS.add_card({
                    set = 'Base',
                    area = G.hand,
                    no_edition = true
                })
                if created_playing_card0 then
                    table.insert(created_playing_cards0, created_playing_card0)
                end
                if #created_playing_cards0 > 0 then
                    if G.deck and G.deck.config then
                        G.deck.config.card_limit = math.max(G.deck.config.card_limit, #G.playing_cards)
                    end
                    SMODS.calculate_context({
                        playing_card_added = true,
                        cards = created_playing_cards0
                    })
                end
                do
                local target_consumables = {}
                for _, consumable in ipairs((G and G.consumeables and G.consumeables.cards) or {}) do
                if consumable and not consumable.getting_sliced and not consumable.removed then
                target_consumables[#target_consumables + 1] = consumable
                end
                end
                if #target_consumables > 0 then
                local target_consumable = pseudorandom_element(target_consumables, pseudoseed('destroy_consumable'))
                if target_consumable then SMODS.destroy_cards({target_consumable}) end
                end
                end
                return {
                    message = #created_playing_cards0 > 0 and 'Added Cards!',
                    colour = G.C.GREEN,
                    extra = {
                        message = 'Destroyed Consumable!',
                        colour = G.C.RED
                    }
                }
            end
            redeem_result = apply_rule()
        end
        return redeem_result
    end
}
