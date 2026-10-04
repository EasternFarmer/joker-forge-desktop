-- Purpose: Snapshot for `edit_card_random_helpers` (joker) codegen output.

SMODS.Joker {
    key = 'j_edit_card_random_helpers',
    loc_txt = {
        ['name'] = 'Edit Card Random Helpers',
        ['text'] = {
            [1] = 'Uses SMODS random polling helpers.'
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
    calculate = function(self, card, context)
        if context.joker_main then
            do
                local edited_card = context and context.other_card
                if edited_card then
                G.E_MANAGER:add_event(Event({
                func = function()
                if not edited_card or edited_card.removed or edited_card.destroyed or edited_card.shattered or edited_card.getting_sliced then return true end
                        local random_enhancement = SMODS.poll_enhancement({ key = 'edit_card_enhancement', guaranteed = true, no_replace = true })
                        if random_enhancement then edited_card:set_ability(G.P_CENTERS[random_enhancement]) end
                        local random_edition = SMODS.poll_edition({ key = 'edit_card_edition', no_negative = true, guaranteed = true })
                        if random_edition then
                            edited_card:set_edition(random_edition, true)
                        end
                card_eval_status_text(edited_card, 'extra', nil, nil, nil, {message = 'Card Modified!', colour = G.C.ORANGE})
                return true
                end
                }))
                end
                return {
                    colour = G.C.BLUE
                }
            end
        end
    end
}
