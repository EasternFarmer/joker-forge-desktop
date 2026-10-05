use super::context::CompileContext;
use crate::lua_ast::*;
use crate::types::ObjectType;

pub(super) fn return_loc_vars(
    ctx: &CompileContext,
    entries: Vec<TableEntry>,
    text_slots: &[usize],
) -> Vec<Stmt> {
    if text_slots.is_empty() {
        return vec![lua_return(lua_table_raw(entries))];
    }

    let default_set = match ctx.object_type {
        ObjectType::Joker => "Joker",
        ObjectType::Consumable | ObjectType::ConsumableType => "Tarot",
        ObjectType::Enhancement => "Enhanced",
        ObjectType::Edition => "Edition",
        ObjectType::Voucher => "Voucher",
        ObjectType::Deck => "Back",
        ObjectType::Seal | ObjectType::Booster | ObjectType::Rarity => "Other",
    };
    let mut body = vec![
        lua_local("description_result", lua_table_raw(entries)),
        lua_local("description_fallback_key", lua_str(ctx.smods_key())),
        lua_local("description_fallback_set", lua_str(default_set)),
        lua_local(
            "description_text_slots",
            lua_table_raw(
                text_slots
                    .iter()
                    .map(|slot| TableEntry::IndexValue(lua_int(*slot as i64), lua_bool(true)))
                    .collect(),
            ),
        ),
    ];
    body.push(lua_raw_stmt(
        r#"if not (G and G.localization and G.localization.descriptions and type(loc_parse_string) == 'function' and type(description_result.vars) == 'table') then
    return description_result
end
local description_key = self and self.key
if type(description_key) ~= 'string' then description_key = description_fallback_key end
local description_set = self and self.set
if type(description_set) ~= 'string' then description_set = description_fallback_set end"#,
    ));
    if ctx.object_type == ObjectType::Seal {
        body.push(lua_raw_stmt(
            "description_set = 'Other'\ndescription_key = description_key:lower() .. '_seal'",
        ));
    }
    body.push(lua_raw_stmt(
        r#"local description_entries = G.localization.descriptions[description_set]
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
description_result.name_key = description_key"#,
    ));
    body.push(lua_return(lua_ident("description_result")));
    body
}
