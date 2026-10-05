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
        local description_result = {
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
        local description_fallback_key = 'c_modprefix_variable_test'
        local description_fallback_set = 'Tarot'
        local description_text_slots = {
            [3] = true
        }
        if not (G and G.localization and G.localization.descriptions and type(loc_parse_string) == 'function' and type(description_result.vars) == 'table') then
            return description_result
        end
        local description_key = self and self.key
        if type(description_key) ~= 'string' then description_key = description_fallback_key end
        local description_set = self and self.set
        if type(description_set) ~= 'string' then description_set = description_fallback_set end
        local description_entries = G.localization.descriptions[description_set]
        local description_source = description_entries and description_entries[description_key]
        if type(description_source) ~= 'table' or type(description_source.text) ~= 'table' then
            return description_result
        end
        local description_vars = {}
        for key, value in pairs(description_result.vars) do description_vars[key] = value end
        if description_result.colours then description_vars.colours = description_result.colours end
        description_result.vars = description_vars
        local description_hash_slot = #description_vars + 1
        local description_hash_token = '#' .. description_hash_slot .. '#'
        local function description_normalize_breaks(value)
            return value:gsub('\r\n', '\n'):gsub('\r', '\n'):gsub('%[s%]', '\n'):gsub('<[bB][rR]%s*/?%s*>', '\n')
        end
        local function description_control_state(value, in_control)
            for i = 1, #value do
                local char = value:sub(i, i)
                if char == '{' then in_control = true
                elseif char == '}' or char == '\n' then in_control = false end
            end
            return in_control
        end
        local function description_insert_text(value, in_control)
            local parts = {}
            value = description_normalize_breaks(tostring(value))
            for i = 1, #value do
                local char = value:sub(i, i)
                if char == '{' then in_control = true
                elseif char == '}' or char == '\n' then in_control = false end
                if char == '#' and not in_control then
                    description_vars[description_hash_slot] = '#'
                    parts[#parts + 1] = description_hash_token
                else
                    parts[#parts + 1] = char
                end
            end
            return table.concat(parts), in_control
        end
        local function description_expand_line(line)
            local parts, start, in_control = {}, 1, false
            while true do
                local first, last, index = line:find('#(%d+)#', start)
                if not first then
                    parts[#parts + 1] = line:sub(start)
                    break
                end
                local prefix = line:sub(start, first - 1)
                parts[#parts + 1] = prefix
                in_control = description_control_state(prefix, in_control)
                local slot = tonumber(index)
                if description_text_slots[slot] and description_vars[slot] ~= nil then
                    local inserted
                    inserted, in_control = description_insert_text(description_vars[slot], in_control)
                    parts[#parts + 1] = inserted
                else
                    parts[#parts + 1] = line:sub(first, last)
                end
                start = last + 1
            end
            return description_normalize_breaks(table.concat(parts))
        end
        local description_copy = {}
        for key, value in pairs(description_source) do description_copy[key] = value end
        description_copy.text, description_copy.text_parsed = {}, {}
        local description_active_tag = ''
        for _, source_line in ipairs(description_source.text) do
            local expanded = description_expand_line(tostring(source_line))
            for line in (expanded .. '\n'):gmatch('(.-)\n') do
                local formatted = line
                if description_active_tag ~= '' and line:sub(1, 1) ~= '{' then
                    formatted = description_active_tag .. line
                end
                for tag in line:gmatch('{[^}]*}') do
                    description_active_tag = tag == '{}' and '' or tag
                end
                if line:gsub('{[^}]*}', ''):match('^%s*$') then formatted = '{s:1} ' end
                description_copy.text[#description_copy.text + 1] = formatted
                description_copy.text_parsed[#description_copy.text_parsed + 1] = loc_parse_string(formatted)
            end
        end
        local description_dynamic_key = 'jf_text_variables:' .. description_key
        description_entries[description_dynamic_key] = description_copy
        description_result.key = description_dynamic_key
        description_result.name_key = description_key
        return description_result
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
