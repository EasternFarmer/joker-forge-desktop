"""Run compiler output in Lua 5.1/LuaJIT, including missing game state.

Usage: python scripts/test-codegen-lua.py --lua-library /path/to/lua51.dll
The library can also be supplied through BALATRO_LUA_LIBRARY.
"""
import argparse
import ctypes
import ctypes.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parent.parent


def load_lua(library):
    lua = ctypes.CDLL(library)
    signatures = {
        "luaL_newstate": ([], ctypes.c_void_p),
        "luaL_openlibs": ([ctypes.c_void_p], None),
        "luaL_loadstring": ([ctypes.c_void_p, ctypes.c_char_p], ctypes.c_int),
        "lua_pcall": ([ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int], ctypes.c_int),
        "lua_tolstring": ([ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p], ctypes.c_char_p),
        "lua_tonumber": ([ctypes.c_void_p, ctypes.c_int], ctypes.c_double),
        "lua_close": ([ctypes.c_void_p], None),
    }
    for name, (args, result) in signatures.items():
        function = getattr(lua, name)
        function.argtypes = args
        function.restype = result
    return lua


def evaluate(lua, source):
    state = lua.luaL_newstate()
    if not state:
        raise RuntimeError("Unable to create Lua state")
    lua.luaL_openlibs(state)
    try:
        status = lua.luaL_loadstring(state, source.encode("utf-8"))
        if status == 0:
            status = lua.lua_pcall(state, 0, 1, 0)
        if status:
            raise AssertionError(lua.lua_tolstring(state, -1, None).decode("utf-8"))
        return lua.lua_tonumber(state, -1)
    finally:
        lua.lua_close(state)


def lua_data(value):
    if isinstance(value, dict):
        return "{" + ",".join("[" + lua_data(key) + "]=" + lua_data(entry)
                              for key, entry in value.items()) + "}"
    if isinstance(value, list):
        return "{" + ",".join(lua_data(entry) for entry in value) + "}"
    if value is None:
        return "nil"
    return json.dumps(value, ensure_ascii=False)


HELPERS = """
function to_big(value) return value end
function lenient_bignum(value) return value end
function copy_table(value)
 if type(value)~='table' then return value end
 local copy={};for key,entry in pairs(value) do copy[key]=copy_table(entry) end;return copy
end
function numeric(value)
 assert(type(value)=='number', 'expected number, got '..type(value))
 assert(value==value and value~=math.huge and value~=-math.huge, 'nonfinite number')
 return value
end
SMODS = {
 Joker=function(definition) test_definition=definition end,
 Consumable=function(definition) test_definition=definition end,
 Voucher=function(definition) test_definition=definition end,
 Back=function(definition) test_definition=definition end,
 Enhancement=function(definition) test_definition=definition end,
 Seal=function(definition) test_definition=definition end,
 Edition=function(definition) test_definition=definition end,
 get_probability_vars=function(card,numerator,denominator) return numerator,denominator end,
 has_enhancement=function(card, key) return card.config and card.config.center and card.config.center.key==key end,
 get_enhancements=function(card)
  if card.config and card.config.center and card.config.center.key~='c_base' then
   return {[card.config.center.key]=true}
  end
  return {}
 end
}
"""


def steamodded_effect_resolver():
    """Use the bundled Steamodded implementation to resolve generated effects."""
    source = (ROOT / "public/other/smods-main/src/utils.lua").read_text(encoding="utf-8")
    individual = source.split("SMODS.calculate_individual_effect = function", 1)[1]
    individual = "SMODS.calculate_individual_effect = function" + individual.split(
        "\n-- Used to calculate a table of effects", 1
    )[0]
    calculation = source.split("SMODS.calculate_effect = function", 1)[1]
    calculation = "SMODS.calculate_effect = function" + calculation.split(
        "\nSMODS.insert_repetitions", 1
    )[0]
    return individual + calculation + """
SMODS.Scoring_Parameter_Calculation={}
SMODS.calculation_keys=SMODS.other_calculation_keys
SMODS.calculate_context=function() end
function ease_dollars(amount) G.GAME.dollars=(G.GAME.dollars or 0)+amount end
status_messages={}
function card_eval_status_text(card, kind, amount, percent, dir, extra)
 status_messages[#status_messages+1]={kind=kind,amount=amount,message=extra and extra.message}
end
function localize(key) return key end
"""


EFFECT_RESOLVER = steamodded_effect_resolver()


def steamodded_scoring_parameters():
    """Load the actual chips/mult calculators, including their supported keys."""
    source = (ROOT / "public/other/smods-main/src/game_object.lua").read_text(encoding="utf-8")
    scoring = source.split("------- API CODE GameObject.Scoring_Calculation", 1)[1]
    parameters = "SMODS.Scoring_Parameter({" + scoring.split("SMODS.Scoring_Parameter({", 1)[1]
    parameters = parameters.split("    SMODS.Calculation_Controls = {", 1)[0]
    utils = (ROOT / "public/other/smods-main/src/utils.lua").read_text(encoding="utf-8")
    repetitions = "SMODS.insert_repetitions = function" + utils.split("SMODS.insert_repetitions = function", 1)[1]
    repetitions = repetitions.split("\nSMODS.calculate_retriggers", 1)[0]
    return """
SMODS.Scoring_Parameters={}
SMODS.Scoring_Parameter=function(parameter)
 parameter.current=parameter.default_value
 SMODS.Scoring_Parameters[parameter.key]=parameter
 for _,key in ipairs(parameter.calculation_keys) do
  SMODS.Scoring_Parameter_Calculation[key]=parameter.key
 end
end
function mod_chips(amount) return amount end
function mod_mult(amount) return amount end
function update_hand_text() end
function juice_card() end
""" + parameters + """
SMODS.Calculation_Controls={chips=true,mult=true}
SMODS.calculation_keys={}
for _,keys in ipairs({SMODS.pre_scoring_calculation_keys,SMODS.scoring_parameter_keys,SMODS.other_calculation_keys}) do
 for _,key in ipairs(keys) do SMODS.calculation_keys[#SMODS.calculation_keys+1]=key end
end
function reset_score(chips,multiplier)
 hand_chips=chips;mult=multiplier
 SMODS.Scoring_Parameters.chips.current=chips
 SMODS.Scoring_Parameters.mult.current=multiplier
end
function resolve_joker(context)
 local effect=test_definition:calculate(actor,context)
 if effect then SMODS.calculate_effect(effect,context.other_card or actor) end
 return effect
end
repetition_warnings={}
function sendWarnMessage(message) repetition_warnings[#repetition_warnings+1]=message end
SMODS.optional_features={}
SMODS.calculate_quantum_enhancements=function() end
SMODS.get_card_areas=function(kind) return kind=='jokers' and {{cards={actor}}} or {} end
function eval_card(card,context)
 if card==actor then
  local effect=test_definition:calculate(actor,context)
  return effect and {jokers=effect} or {},{}
 end
 return {},{}
end
function collect_repetitions(context)
 context.repetition=true
 return SMODS.calculate_repetitions(context.other_card,context,{})
end
function message_count(kind)
 local count=0
 for _,message in ipairs(status_messages) do if message.kind==kind then count=count+1 end end
 return count
end
reset_score(0,1)
""" + repetitions


SCORING_PARAMETERS = steamodded_scoring_parameters()


def steamodded_card_destruction_runtime():
    """Keep native scoring contexts and destruction bookkeeping in the regression."""
    source = (ROOT / "public/other/smods-main/src/utils.lua").read_text(encoding="utf-8")
    sections = [
        ("SMODS.trigger_effects = function", "\nSMODS.calculate_effect = function"),
        ("function SMODS.calculate_card_areas", "\n\n-- Updates a [context]"),
        ("function SMODS.calculate_context", "\nfunction SMODS.in_scoring"),
        ("function SMODS.in_scoring", "\nfunction SMODS.calculate_main_scoring"),
        ("function SMODS.calculate_destroying_cards", "\nfunction SMODS.blueprint_effect"),
    ]
    native = "\n".join(start + source.split(start, 1)[1].split(end, 1)[0]
                       for start, end in sections)
    return native + """
SMODS.context_stack={}
SMODS.push_to_context_stack=function(context) SMODS.context_stack[#SMODS.context_stack+1]={context=context} end
SMODS.pop_from_context_stack=function() table.remove(SMODS.context_stack) end
SMODS.check_looping_context=function() return false end
SMODS.update_context_flags=function() end
SMODS.Sticker={obj_buffer={}}
SMODS.get_card_areas=function(kind) return kind=='playing_cards' and {G.play,G.hand} or {} end
SMODS.shatters=function(card) return test_definition.shatters or false end
function highlight_card() end
G.STAGE=1;G.STAGES={RUN=1};percent=0;percent_delta=0.08
function eval_card(card,context)
 if card~=actor then return {},{} end
 local effect=test_definition:calculate(card,context)
 if effect then effect.card=effect.card or card end
 return effect and {[native_effect_key]=effect} or {},{}
end
"""


CARD_DESTRUCTION_RUNTIME = steamodded_card_destruction_runtime()


def steamodded_post_trigger_runtime():
    """Use Steamodded's feature scan and patched evaluation callback together."""
    source = (ROOT / "public/other/smods-main/src/utils.lua").read_text(encoding="utf-8")
    feature_scan = "SMODS.get_optional_features = function" + source.split(
        "SMODS.get_optional_features = function", 1
    )[1].split("\n\nG.FUNCS.can_select_from_booster", 1)[0]
    patches = (ROOT / "public/other/smods-main/lovely/better_calc.toml").read_text(encoding="utf-8")
    payloads = [payload.split('"""', 1)[0] for payload in patches.split('payload = """')[1:]]
    evaluation = next(payload for payload in payloads
                      if "post_trigger = true" in payload and "return ret, post_trig" in payload)
    return """
function insert(target,values) for key,value in pairs(values) do target[key]=value end end
SMODS.optional_features={}
SMODS.get_card_areas=function() return {G.jokers} end
SMODS.calculate_retriggers=function() return {} end
SMODS.is_getter_context=function() return false end
post_contexts={}
SMODS.calculate_context=function(context)
 post_contexts[#post_contexts+1]=context
 last_post_result=test_definition:calculate(actor,context)
end
""" + feature_scan + "\nfunction evaluate_observed_joker(card,context)\nlocal ret={}\n" + evaluation + "\nend\n"


POST_TRIGGER_RUNTIME = steamodded_post_trigger_runtime()


def steamodded_probability_result_runtime():
    """Resolve real result notifications synchronously, with a bounded recursion guard."""
    source = (ROOT / "public/other/smods-main/src/utils.lua").read_text(encoding="utf-8")
    sections = [
        ("SMODS.trigger_effects = function", "\nSMODS.calculate_effect = function"),
        ("function SMODS.calculate_card_areas", "\n\n-- Updates a [context]"),
        ("function SMODS.is_getter_context", "\nfunction SMODS.get_previous_context"),
        ("function SMODS.calculate_context", "\nfunction SMODS.in_scoring"),
        ("function SMODS.blueprint_effect", "\nfunction SMODS.get_mods_scoring_targets"),
        ("function SMODS.get_probability_vars", "\nfunction SMODS.is_poker_hand_visible"),
    ]
    native = "\n".join(start + source.split(start, 1)[1].split(end, 1)[0]
                       for start, end in sections)
    return native + """
G.STAGE=1;G.STAGES={RUN=1};G.GAME.probabilities={normal=1};G.GAME.dollars=0
G.jokers.cards={};SMODS.Sticker={obj_buffer={}};SMODS.post_prob={}
SMODS.update_context_flags=function() end
SMODS.get_card_areas=function(kind) return kind=='jokers' and {G.jokers} or {} end
probability_contexts={};probability_counts={result=0,modifier=0,fixed=0}
local native_push=SMODS.push_to_context_stack
function SMODS.push_to_context_stack(context,...)
 assert(#SMODS.context_stack<24,'probability result notification recursion exceeded 24 contexts')
 if context.pseudorandom_result then
  probability_counts.result=probability_counts.result+1
  probability_contexts[#probability_contexts+1]=context
 elseif context.mod_probability then probability_counts.modifier=probability_counts.modifier+1
 elseif context.fix_probability then probability_counts.fixed=probability_counts.fixed+1 end
 return native_push(context,...)
end
function sendWarnMessage(message) error(message) end
function probability_card(definition)
 local card={definition=definition,ability=copy_table(definition.config or {extra={}}),
  config={center=definition},facing='front'}
 card.ability.extra=card.ability.extra or {}
 function card:calculate_joker(context) return self.definition:calculate(self,context) end
 function card:juice_up() end
 return card
end
function probability_blueprint(copied_card)
 local card={copied_card=copied_card,ability={},config={center={key='j_blueprint',set='Joker'}}}
 function card:calculate_joker(context) return SMODS.blueprint_effect(self,self.copied_card,context) end
 return card
end
function eval_card(card,context)
 local effect=card:calculate_joker(context)
 return effect and {jokers=effect} or {},{}
end
function set_probability_rolls(values)
 probability_rolls=values;probability_roll_count=0
end
function pseudorandom(seed)
 probability_roll_count=probability_roll_count+1
 return probability_rolls[probability_roll_count] or 0
end
function external_probability(numerator,denominator,identifier)
 local result=SMODS.pseudorandom_probability(actor,'external',numerator or 1,denominator or 2,identifier or 'external')
 SMODS.trigger_effects({},actor)
 return result
end
function assert_probability_dispatch_complete(results,rolls)
 assert(probability_counts.result==results,'unexpected probability result notifications: '..probability_counts.result)
 assert(probability_roll_count==rolls,'unexpected probability rolls: '..probability_roll_count)
 assert(#SMODS.post_prob==0 and #SMODS.context_stack==0,'probability dispatch must finish')
end
"""


PROBABILITY_RESULT_RUNTIME = steamodded_probability_result_runtime()


def rarity_shop_runtime(lua_library):
    """Use native rarity registration, Joker injection, and weighted shop polling."""
    objects = (ROOT / "public/other/smods-main/src/game_object.lua").read_text(encoding="utf-8")
    utils = (ROOT / "public/other/smods-main/src/utils.lua").read_text(encoding="utf-8")
    base = "SMODS.GameObject = Object:extend()" + objects.split("SMODS.GameObject = Object:extend()", 1)[1]
    base = base.split("    function SMODS.GameObject:process_loc_text()", 1)[0]
    rarities = "SMODS.Rarities = {}" + objects.split("SMODS.Rarities = {}", 1)[1]
    rarities = rarities.split("------- API CODE GameObject.ConsumableType", 1)[0]
    centers = "SMODS.Centers = {}" + objects.split("SMODS.Centers = {}", 1)[1]
    centers = centers.split("------- API CODE GameObject.Center.Consumable", 1)[0]
    sections = [
        ("function SMODS.merge_defaults", "\nV = require"),
        ("function SMODS.insert_pool", "\nfunction SMODS.juice_up_blind"),
        ("function SMODS.poll_rarity", "\nfunction "),
    ]
    native = "\n".join(start + utils.split(start, 1)[1].split(end, 1)[0]
                       for start, end in sections)
    executable = Path(lua_library).parent / "Balatro.exe"
    if executable.is_file():
        with zipfile.ZipFile(executable) as archive:
            object_base = archive.read("engine/object.lua").decode("utf-8")
            events = archive.read("functions/common_events.lua").decode("utf-8")
        pool = "function get_current_pool" + events.split("function get_current_pool", 1)[1]
        pool = pool.split("\nfunction ", 1)[0]
        patches = tomllib.loads((ROOT / "public/other/smods-main/lovely/rarity.toml").read_text(encoding="utf-8"))
        rarity_patch = next(patch["regex"]["payload"] for patch in patches["patches"]
                            if "regex" in patch and 'SMODS.poll_rarity("Joker"' in patch["regex"]["payload"])
        first = pool.index("            local rarity = _rarity")
        last = pool.index("            _starting_pool", first)
        pool = pool[:first] + rarity_patch + pool[last:]
    else:
        # Keep the runner portable when only a Lua shared library is available.
        # Installed-game runs above also exercise the actual game's pool filtering.
        object_base = """
Object={};Object.__index=Object
function Object:extend()
 local cls={};for k,v in pairs(self) do if k:find('__')==1 then cls[k]=v end end
 cls.__index=cls;cls.super=self;setmetatable(cls,self);return cls
end
"""
        pool = """
function get_current_pool(kind,rarity,legendary,source)
 local aliases={Common=1,Uncommon=2,Rare=3,Legendary=4}
 rarity=aliases[rarity] or rarity or SMODS.poll_rarity(kind,source)
 local result={}
 for _,center in ipairs(G.P_JOKER_RARITY_POOLS[rarity]) do
  if center.unlocked~=false then result[#result+1]=center.key end
 end
 return result,'Joker'..rarity
end
"""
    return object_base + """
SMODS.current_mod=nil
G={P_CENTERS={},P_CENTER_POOLS={Joker={}},P_JOKER_RARITY_POOLS={[1]={},[2]={},[3]={},[4]={}},
 C={RARITY={}},ARGS={TEMP_POOL={}},GAME={round_resets={ante=1},used_jokers={},pool_flags={},banned_keys={}}}
Game={init_game_object=function() return copy_table(G.GAME) end}
function HEX(value) return value end
function sendWarnMessage(message) error(message) end
function EMPTY(value) for key in pairs(value) do value[key]=nil end;return value end
function find_joker() return {} end
function pseudoseed(seed) return seed end
function pseudorandom() return rarity_roll end
""" + native + base + rarities + centers + pool + """
for rarity=1,3 do SMODS.Joker{key='vanilla_'..rarity,rarity=rarity,unlocked=true} end
SMODS.current_mod={prefix='mod'}
rarity_roll=0.99
function inject_rarity_shop()
 for _,key in ipairs(SMODS.Rarity.obj_buffer) do SMODS.Rarities[key]:inject() end
 for _,key in ipairs(SMODS.ObjectType.obj_buffer) do SMODS.ObjectTypes[key]:inject() end
 for _,key in ipairs(SMODS.Joker.obj_buffer) do SMODS.Centers[key]:inject() end
 G.GAME=Game:init_game_object()
end
"""


def description_localization_runtime(lua_library):
    """Exercise installed Balatro's parser, localization, and text-node sizing."""
    executable = Path(lua_library).parent / "Balatro.exe"
    if not executable.is_file():
        # A standalone Lua installation can still verify exported text entries.
        return "native_description_localization=false\n"
    with zipfile.ZipFile(executable) as archive:
        functions = archive.read("functions/misc_functions.lua").decode("utf-8-sig")
        definitions = archive.read("functions/UI_definitions.lua").decode("utf-8-sig")
        ui = archive.read("engine/ui.lua").decode("utf-8-sig")
    sections = [
        ("function init_localization()", "\nfunction playing_card_joker_effects"),
        ("function loc_parse_string(line)", "\n--UTF8 handler"),
        ("function localize(args, misc_cat)", "\nfunction get_stake_sprite"),
    ]
    native = "\n".join(start + functions.split(start, 1)[1].split(end, 1)[0]
                       for start, end in sections)
    rows = "function desc_from_rows" + definitions.split("function desc_from_rows", 1)[1]
    rows = rows.split("function transparent_multiline_text", 1)[0]
    sizing = "function UIBox:calculate_xywh" + ui.split("function UIBox:calculate_xywh", 1)[1]
    sizing = sizing.split("\nfunction UIBox:", 1)[0]
    utils = (ROOT / "public/other/smods-main/src/utils.lua").read_text(encoding="utf-8")
    box = "function SMODS.localize_box" + utils.split("function SMODS.localize_box", 1)[1]
    box = box.split("\nfunction SMODS.get_multi_boxes", 1)[0]
    return """
native_description_localization=true
UIBox={}
SMODS.Fonts={}
SMODS.DynaTextEffects={}
function loc_colour(colour) return 'colour:'..(colour or 'default') end
function DynaText(config) return config end
function format_ui_value(value) return value end
G.LANG={font={DESCSCALE=1,FONTSCALE=1,TEXT_HEIGHT_SCALE=1,squish=1,
 FONT={getWidth=function(self,text) return #text end,getHeight=function() return 1 end}}}
G.FONTS={};G.TILESCALE=1;G.TILESIZE=1
G.UIT={T=1,R=2,C=3,O=4,B=5,padding=0}
G.C={CLEAR={},UI={BACKGROUND_WHITE={},TEXT_LIGHT={},TEXT_DARK={}}}
""" + native + rows + sizing + box + """
assert(loc_parse_string('')==nil,'regression must preserve native empty-string behavior')
function assert_description_layout(center,vars,expected)
 G.localization={misc={v_dictionary={},v_text={},tutorial={},quips={}},descriptions={Joker={test=center}}}
 init_localization()
 assert(#center.text_parsed==#expected,'blank lines were lost during native localization parsing')
 local nodes={}
 localize{type='descriptions',set='Joker',key='test',vars=vars,nodes=nodes}
 assert(#nodes==#expected,'blank lines were lost during native localization')
 local layout=desc_from_rows(nodes)
 local rows=layout.nodes[1].nodes
 assert(#rows==#expected,'blank lines were lost from description rows')
 for i,row in ipairs(rows) do
  local text=''
  for _,node in ipairs(row.nodes) do text=text..node.config.text end
  assert(text==expected[i],'unexpected localized line '..i..': '..text)
  assert(#row.nodes>0,'blank lines must contain a measurable text node')
  for _,node in ipairs(row.nodes) do
   node.UIT=node.n;node.ARGS={}
   function node:set_values(transform) self.T=copy_table(transform) end
   local width,height=UIBox:calculate_xywh(node,{x=0,y=0,w=0,h=0})
   assert(height==0.32,'blank lines must reserve the same height as ordinary description text')
  end
  -- Steamodded's multi-box path uses its own native line builder.
  local smods_nodes=SMODS.localize_box(center.text_parsed[i],{vars=vars})
  local smods_text=''
  for _,node in ipairs(smods_nodes) do smods_text=smods_text..node.config.text end
  assert(smods_text==expected[i],'Steamodded localized line changed')
 end
end
function assert_description_format(center,vars,expected)
 G.localization={misc={v_dictionary={},v_text={},tutorial={},quips={}},descriptions={Joker={test=center}}}
 init_localization()
 assert(#center.text_parsed==#expected,'formatted line count changed during native parsing')
 -- Every new tag replaces the complete control table; every line starts empty.
 local adjacent=loc_parse_string('{C:red}{E:1}text')[1].control
 assert(adjacent.E=='1' and adjacent.C==nil,'native controls must replace, not merge')
 assert(next(loc_parse_string('text')[1].control)==nil,'native lines must start without formatting')
 assert(next(loc_parse_string('{C:red}{}text')[1].control)==nil,'empty tags must reset all controls')
 local native_nodes={}
 localize{type='descriptions',set='Joker',key='test',vars=vars,nodes=native_nodes}
 assert(#native_nodes==#expected,'native formatted rows were lost')
 local layout=desc_from_rows(native_nodes)
 assert(#layout.nodes[1].nodes==#expected,'formatted rows were lost in the description layout')
 local function collect(nodes,result,background)
  for _,node in ipairs(nodes) do
   if node.n==G.UIT.C then
    collect(node.nodes,result,node.config.colour or background)
   elseif node.n==G.UIT.O then
    local object=node.config.object
    result[#result+1]={text=object.string[1],colour=object.colours[1],scale=object.scale,
     float=object.float,bump=object.bump,spacing=object.spacing,background=background}
   else
    result[#result+1]={text=node.config.text,colour=node.config.colour,scale=node.config.scale,background=background}
   end
  end
 end
 local function check_nodes(nodes,line,smods)
  local rendered={};collect(nodes,rendered)
  assert(#rendered==#line,'formatting changed the number of text segments')
  for i,part in ipairs(line) do
   local control=part.control
   local text=part.text:gsub('#(%d+)#',function(index) return tostring(vars[tonumber(index)]) end)
   local actual=rendered[i]
   local colour=control.V and vars.colours[tonumber(control.V)] or loc_colour(control.C)
   local background=smods and control.B and vars.colours[tonumber(control.B)]
     or control.X and (smods or not control.E) and loc_colour(control.X) or nil
   assert(actual.text==text,'formatted text changed: '..tostring(actual.text)..' / '..text)
   assert(actual.colour==colour,'text colour did not reach the native render node')
   assert(math.abs(actual.scale-0.32*(tonumber(control.s) or 1))<0.000001,'text scale did not reach the native render node')
   assert(actual.background==background,'background colour did not reach the native render node')
   assert(actual.float==(control.E=='1' and true or nil),'floating effect did not reach the native render node')
   assert(actual.bump==(control.E=='2' and true or nil),'bump effect did not reach the native render node')
   assert(actual.spacing==(control.E=='2' and 1 or nil),'bump spacing did not reach the native render node')
  end
 end
 for index,line in ipairs(expected) do
  local parsed=center.text_parsed[index]
  assert(#parsed==#line,'native parser changed the number of text segments')
  for i,part in ipairs(line) do
   for key,value in pairs(part.control) do
    assert(parsed[i].control[key]==value,'formatting was lost on line '..index..': '..key)
   end
   for key,value in pairs(parsed[i].control) do
    assert(part.control[key]==value,'unexpected formatting leaked on line '..index..': '..key)
   end
  end
  check_nodes(native_nodes[index],line,false)
  check_nodes(SMODS.localize_box(parsed,{vars=vars}),line,true)
 end
end
"""


def consumable_creation_message_runtime(lua_library):
    """Resolve messages with Balatro's real dictionary and localize implementation."""
    executable = Path(lua_library).parent / "Balatro.exe"
    if executable.is_file():
        with zipfile.ZipFile(executable) as archive:
            dictionary = archive.read("localization/en-us.lua").decode("utf-8-sig")
            functions = archive.read("functions/misc_functions.lua").decode("utf-8-sig")
        localize = "function localize(args, misc_cat)" + functions.split(
            "function localize(args, misc_cat)", 1
        )[1].split("\nfunction get_stake_sprite", 1)[0]
        localization = "G.localization=(function()\n" + dictionary + "\nend)()\n" + localize
    else:
        # Preserve the game's missing-key behavior when only Lua is installed.
        localization = """
G.localization={misc={dictionary={k_plus_tarot='+1 Tarot'}}}
function localize(args,misc_cat)
 if args and type(args)~='table' then
  if misc_cat and G.localization.misc[misc_cat] then return G.localization.misc[misc_cat][args] or 'ERROR' end
  return G.localization.misc.dictionary[args] or 'ERROR'
 end
 error('this regression only uses dictionary localization')
end
"""
    utils = (ROOT / "public/other/smods-main/src/utils.lua").read_text(encoding="utf-8")
    add_card = "function SMODS.add_card(t)" + utils.split("function SMODS.add_card(t)", 1)[1]
    add_card = add_card.split("\nfunction SMODS.debuff_card", 1)[0]
    return localization + """
assert(localize('k_plus_tarot')=='+1 Tarot')
assert(localize('k_plus_consumable')=='ERROR','regression must retain native missing-key behavior')
created_cards={}
G.consumeables={cards={},config={card_limit=10}}
function G.consumeables:emplace(card) self.cards[#self.cards+1]=card end
function SMODS.create_card(params)
 assert(params.area==G.consumeables,'creation must target the consumable area')
 local center=params.key and G.P_CENTERS[params.key]
 if not center then
  assert(not params.key,'specific consumable key must survive export')
  center=assert(G.P_CENTER_POOLS[params.set] and G.P_CENTER_POOLS[params.set][1],'unknown consumable set')
 end
 local card=option_card(center.key,center.set)
 function card:add_to_deck() self.added_to_deck=true end
 created_cards[#created_cards+1]=card
 return card
end
function register_source_consumable(set)
 local center={key='c_mod_runtime_test',set=set,consumeable=true}
 G.P_CENTERS[center.key]=center
 G.P_CENTER_POOLS[set]=G.P_CENTER_POOLS[set] or {center}
 actor.config={center=center};actor.ability.set=set
 G.consumeables:emplace(actor)
end
function assert_consumable_creation_message(count,key,set,messages)
 assert(#created_cards==count,'unexpected created-card count')
 local source_count=actor.config and actor.config.center.consumeable and 1 or 0
 assert(#G.consumeables.cards==count+source_count,'native SMODS.add_card must emplace each created consumable')
 for _,card in ipairs(created_cards) do
  assert(card.config.center.key==key and card.ability.set==set,'created key or set changed')
  assert(card.added_to_deck,'native SMODS.add_card must add the card to the deck')
 end
 assert(#status_messages==messages,'unexpected status-message count')
 for _,status in ipairs(status_messages) do
  assert(status.message=='Created Consumable!','unexpected creation status: '..tostring(status.message))
 end
end
""" + add_card


def steamodded_playing_card_transform_runtime():
    """Apply editions with Steamodded's real Card implementation, not a setter stub."""
    source = (ROOT / "public/other/smods-main/src/overrides.lua").read_text(encoding="utf-8")
    method = "function Card:set_edition(" + source.split("function Card:set_edition(", 1)[1]
    method = method.split("\n-- _key = key value for random seed", 1)[0]
    utils = (ROOT / "public/other/smods-main/src/utils.lua").read_text(encoding="utf-8")
    sections = [
        ("SMODS.trigger_effects = function", "\nSMODS.calculate_effect = function"),
        ("function SMODS.calculate_card_areas", "\n\n-- Updates a [context]"),
        ("function SMODS.score_card", "\nfunction SMODS.calculate_main_scoring"),
    ]
    scoring = "\n".join(start + utils.split(start, 1)[1].split(end, 1)[0]
                        for start, end in sections)
    return method + "\n" + scoring + """
SMODS.enh_cache={write=function() end}
SMODS.Sticker={obj_buffer={}}
G.CONTROLLER={locks={}}
G.C.ORANGE=1
for key,config in pairs({e_foil={chips=50},e_holo={mult=10},e_polychrome={x_mult=1.5},
 e_negative={card_limit=1},e_mod_shiny={x_mult=2}}) do
 G.P_CENTERS[key]={key=key,set='Edition',config=config,discovered=true,
  sound={sound='edition',per=1,vol=1}}
end
function play_sound() end
function discover_card() end
function check_for_unlock() end
function playing_card()
 local card={ability={set='Default',card_limit=0,extra_slots_used=0},
  config={center={key='c_base',set='Default'}},base={id=2,suit='Hearts',nominal=2},
  ignore_base_shader={},ignore_shadow={},base_cost=1,facing='front',area=G.play}
 function card:juice_up() self.juice_calls=(self.juice_calls or 0)+1 end
 return setmetatable(card,Card)
end
observed_card=playing_card()
other_playing_card=playing_card()
G.play={cards={observed_card,other_playing_card}}
observed_card.area=G.play;other_playing_card.area=G.play
G.hand={cards={}}
SMODS.get_card_areas=function(kind) return kind=='jokers' and {{cards={actor}}} or {} end
SMODS.check_looping_context=function() return false end
SMODS.calculate_quantum_enhancements=function() end
SMODS.calculate_repetitions=function() end
function eval_card(card,context)
 if card~=actor then return {},{} end
 local effect=test_definition:calculate(card,context)
 return effect and {jokers=effect} or {},{}
end
"""


PLAYING_CARD_TRANSFORM_RUNTIME = steamodded_playing_card_transform_runtime()


def steamodded_booster_open_runtime():
    """Use native opening payloads so saved modifiers must affect real pack choices."""
    patches = tomllib.loads((ROOT / "public/other/smods-main/lovely/booster.toml").read_text(encoding="utf-8"))
    payloads = [patch["pattern"]["payload"] for patch in patches["patches"] if "pattern" in patch]
    choices = next(payload for payload in payloads if "G.GAME.pack_choices = math.min" in payload)
    size = next(payload for payload in payloads if "local _size = math.max(1," in payload)
    return """
G.GAME.modifiers={}
G.C.BLUE=1
SMODS.Centers={}
G.P_CENTERS.p_first.config.extra=8
G.P_CENTERS.p_second.config.extra=10
function booster_card(key)
 local center=G.P_CENTERS[key]
 return {config={center=center},ability=copy_table(center.config)}
end
existing_first_pack=booster_card('p_first')
existing_second_pack=booster_card('p_second')
function open_booster(self)
""" + choices + "\n" + size + """
 return G.GAME.pack_choices,_size
end
function assert_booster(card,choices,size)
 local actual_choices,actual_size=open_booster(card)
 assert(actual_choices==choices,'expected '..choices..' choices, got '..actual_choices)
 assert(actual_size==size,'expected '..size..' cards, got '..actual_size)
end
function assert_booster_centers_unchanged()
 assert(G.P_CENTERS.p_first.config.choose==1 and G.P_CENTERS.p_first.config.extra==8)
 assert(G.P_CENTERS.p_second.config.choose==2 and G.P_CENTERS.p_second.config.extra==10)
end
"""


BOOSTER_OPEN_RUNTIME = steamodded_booster_open_runtime()


def deck_card_runtime(lua_library):
    """Use native edition/base setters and the game's shuffle on starting-card subsets."""
    source = (ROOT / "public/other/smods-main/src/utils.lua").read_text(encoding="utf-8")
    change_base = "function SMODS.change_base(" + source.split("function SMODS.change_base(", 1)[1]
    change_base = change_base.split("\n-- Modify a card's rank", 1)[0]
    executable = Path(lua_library).parent / "Balatro.exe"
    if executable.is_file():
        with zipfile.ZipFile(executable) as archive:
            functions = archive.read("functions/misc_functions.lua").decode("utf-8")
        shuffle = "function pseudoshuffle" + functions.split("function pseudoshuffle", 1)[1]
        shuffle = shuffle.split("\nfunction ", 1)[0]
    else:
        shuffle = """
function pseudoshuffle(cards,seed)
 math.randomseed(seed)
 for i=#cards,2,-1 do local j=math.random(i);cards[i],cards[j]=cards[j],cards[i] end
end
"""
    return change_base + shuffle + """
function pseudoseed(seed)
 local value=0;for i=1,#seed do value=(value*31+string.byte(seed,i))%2147483647 end
 return value
end
SMODS.Suits={};SMODS.Ranks={};G.P_CARDS={}
for _,suit in ipairs({'Hearts','Spades','Diamonds','Clubs'}) do
 SMODS.Suits[suit]={key=suit,card_key=string.sub(suit,1,1)}
end
for id,rank in ipairs({'2','3','4','5','6','7','8','9','10','Jack','Queen','King','Ace'}) do
 SMODS.Ranks[rank]={key=rank,card_key=tostring(id+1),id=id+1}
 for suit,spec in pairs(SMODS.Suits) do
  G.P_CARDS[spec.card_key..'_'..(id+1)]={suit=suit,value=rank,id=id+1,nominal=id+1}
 end
end
G.P_CENTERS.c_base={key='c_base',set='Default'}
G.P_CENTERS.m_stone={key='m_stone',set='Enhanced'}
G.P_CENTER_POOLS.Enhanced[#G.P_CENTER_POOLS.Enhanced+1]=G.P_CENTERS.m_stone
function SMODS.poll_seal() return 'Gold' end
local native_set_edition=Card.set_edition
function Card:set_edition(...)
 self.edition_calls=(self.edition_calls or 0)+1
 return native_set_edition(self,...)
end
function initialize_starting_cards()
 G.deck={cards={}};G.playing_cards={};initial_deck_cards={}
 for index,spec in ipairs({{'Hearts','2'},{'Hearts','King'},{'Spades','King'},
  {'Clubs','Ace'},{'Hearts','King'},{'Diamonds','3'},{'Spades','2'},{'Hearts','Ace'}}) do
  local card=playing_card();card.sort_id=index;card.area=G.deck
  card.config.center=G.P_CENTERS.c_base
  card.base=copy_table(G.P_CARDS[SMODS.Suits[spec[1]].card_key..'_'..SMODS.Ranks[spec[2]].card_key])
  card.original_suit=spec[1];card.original_rank=spec[2]
  function card:is_suit(suit) return self.base.suit==suit end
  function card:set_base(base) assert(base);self.base=copy_table(base) end
  function card:set_ability(center)
   assert(center,'enhancement center must exist');self.config.center=center
   self.ability.set=center.set;self.enhancement_calls=(self.enhancement_calls or 0)+1
  end
  function card:set_seal(seal) self.seal=seal;self.seal_calls=(self.seal_calls or 0)+1 end
  function card:remove()
   self.removed=true
   for i=#G.playing_cards,1,-1 do if G.playing_cards[i]==self then table.remove(G.playing_cards,i) end end
   for i=#G.deck.cards,1,-1 do if G.deck.cards[i]==self then table.remove(G.deck.cards,i) end end
  end
  G.playing_cards[index]=card;G.deck.cards[index]=card;initial_deck_cards[index]=card
 end
end
function assert_starting_card_targets(ids,enhancement,edition,seal,suit,rank)
 local expected={};for _,id in ipairs(ids) do expected[id]=true end
 for id,card in ipairs(initial_deck_cards) do
  assert(G.playing_cards[id]==card and G.deck.cards[id]==card,'targeting must preserve deck order '..id)
  local selected=expected[id] or false
  assert((card.enhancement_calls or 0)==(selected and enhancement and 1 or 0),'enhancement target '..id)
  assert((card.edition_calls or 0)==(selected and edition and 1 or 0),'edition target '..id)
  assert((card.seal_calls or 0)==(selected and seal and 1 or 0),'seal target '..id)
  assert(card.config.center.key==(selected and enhancement or 'c_base'),'enhancement value '..id)
  assert((card.edition and card.edition.key)==(selected and edition or nil),'edition value '..id)
  assert(card.seal==(selected and seal or nil),'seal value '..id)
  assert(card.base.suit==(selected and suit or card.original_suit),'suit value '..id)
  assert(card.base.value==(selected and rank or card.original_rank),'rank value '..id)
 end
 assert(G.GAME.starting_deck_size==#G.playing_cards)
end
function assert_random_starting_targets(count,target_suit,target_rank)
 local ids={}
 for id,card in ipairs(initial_deck_cards) do
  if card.enhancement_calls or card.edition_calls then
   assert(not target_suit or card.original_suit==target_suit,'random suit filter '..id)
   assert(not target_rank or card.original_rank==target_rank,'random rank filter '..id)
   ids[#ids+1]=id
  end
 end
 assert(#ids==count,'expected '..count..' distinct cards, got '..#ids)
 assert_starting_card_targets(ids,'m_bonus','e_polychrome')
end
G.playing_cards=nil;G.deck=nil
"""


def card_area_selection_runtime(lua_library):
    """Exercise installed CardArea selection when Balatro is beside its Lua DLL."""
    executable = Path(lua_library).parent / "Balatro.exe"
    if executable.is_file():
        with zipfile.ZipFile(executable) as archive:
            source = archive.read("cardarea.lua").decode("utf-8")
            common_events = archive.read("functions/common_events.lua").decode("utf-8")
        method = "function CardArea:add_to_highlighted" + source.split(
            "function CardArea:add_to_highlighted", 1
        )[1].split("\nfunction ", 1)[0]
        copy_card_method = "function copy_card(" + common_events.split(
            "function copy_card(", 1
        )[1].split("\nfunction ", 1)[0]
    else:
        # Portable Lua-library runs retain the Joker-area selection contract.
        method = """
function CardArea:add_to_highlighted(card, silent)
 if #self.highlighted>=self.config.highlighted_limit then
  self:remove_from_highlighted(self.highlighted[1])
 end
 self.highlighted[#self.highlighted+1]=card;card:highlight(true)
end
"""
        copy_card_method = """
function copy_card(other,new_card)
 new_card.ability=copy_table(other.ability);new_card.config=copy_table(other.config)
 return new_card
end
"""
    return """
CardArea={};CardArea.__index=CardArea
function CardArea:remove_from_highlighted(card)
 for i,selected in ipairs(self.highlighted) do
  if selected==card then table.remove(self.highlighted,i);card:highlight(false);return end
 end
end
function make_joker_selection_area(limit)
 return setmetatable({config={type='joker',highlighted_limit=limit},cards=owned_jokers,highlighted={}},CardArea)
end
function select_jokers()
 for _,joker in ipairs(owned_jokers) do G.jokers:add_to_highlighted(joker,true) end
end
function clone_card_shell(id,sort_id)
 local clone={ID=id,sort_id=sort_id,ability={},config={}}
 function clone:set_ability(center) self.config.center=center end
 function clone:set_base(base) self.config.card=base end
 function clone:set_edition(edition) self.edition=edition end
 function clone:set_seal(seal) self.seal=seal end
 return clone
end
function check_for_unlock() end
for _,joker in ipairs(owned_jokers) do
 function joker:highlight(highlighted) self.highlighted=highlighted end
end
""" + method + copy_card_method


JOKER_CREATION_STATE = """
G={GAME={joker_buffer=0},C={GREEN=1}}
event_queue={};created_cards={}
function Event(event) return event end
G.E_MANAGER={add_event=function(self,event) event_queue[#event_queue+1]=event end}
function run_events()
 while #event_queue>0 do local event=table.remove(event_queue,1);assert(event.func()) end
end
function joker_area(limit,count)
 local area={cards={},config={card_limit=limit}}
 for i=1,count do area.cards[i]={existing=true} end
 return area
end
function SMODS.add_card(params)
 assert(G.jokers and G.jokers.cards and G.jokers.config, 'Joker area must exist at creation')
 local card={params=params,buffer_at_add=G.GAME.joker_buffer or 0}
 -- SMODS.add_card emplaces synchronously before returning to its caller.
 G.jokers.cards[#G.jokers.cards+1]=card
 created_cards[#created_cards+1]=card
 return card
end
"""
RULE_OPTIONS_STATE = """
Card={};Card.__index=Card
function Card:set_cost()
 self.cost_calls=(self.cost_calls or 0)+1
 self.cost=math.max(0,math.floor(self.base_cost*(1-(G.GAME.discount_percent or 0)/100)))
 self.sell_cost=math.max(1,math.floor(self.cost/2))+(self.ability.extra_value or 0)
 self.sell_cost_label=self.facing=='back' and '?' or self.sell_cost
end
function option_card(key,set)
 local center={key=key,set=set,consumeable=set=='Tarot' or set=='Planet' or set=='Spectral',unlocked=true}
 local c={config={center=center},ability={set=set,extra={}},facing='front',base_cost=20,cost=20}
 function c:set_edition(edition) self.edition=edition;self.edition_calls=(self.edition_calls or 0)+1 end
 function c:add_sticker(sticker) self.ability[sticker]=true end
 function c:remove_sticker(sticker) self.ability[sticker]=false end
 function c:set_sell_value()
  self.sell_update_calls=(self.sell_update_calls or 0)+1
  self.sell_cost=math.max(1,math.floor(self.cost/2))+(self.ability.extra_value or 0)
  self.sell_cost_label=self.facing=='back' and '?' or self.sell_cost
 end
 return setmetatable(c,Card)
end
owned_jokers={option_card('j_first','Joker'),option_card('j_second','Joker'),option_card('j_third','Joker')}
owned_consumables={option_card('c_first','Tarot'),option_card('c_second','Planet'),option_card('c_third','Spectral'),option_card('c_mod_custom','Tarot')}
local first={key='j_first',set='Joker',unlocked=false,rarity=1}
local second={key='j_second',set='Joker',unlocked=true,rarity=3}
local third={key='j_third',set='Joker',unlocked=true,rarity=2}
G={GAME={current_round={},round_resets={ante=1},used_vouchers={}},
 STATES={SHOP=1,SMODS_BOOSTER_OPENED=2,SMODS_REDEEM_VOUCHER=3,PLAY_TAROT=4,HAND_PLAYED=5},
 C={FILTER=1,DARK_EDITION=1,MONEY=1,RED=1,GREEN=1,SUITS={Hearts=1,Spades=1,Diamonds=1,Clubs=1},SECONDARY_SET={Tarot=1}},
 jokers={cards=owned_jokers,highlighted={owned_jokers[2]}},
 consumeables={cards=owned_consumables},
 P_CENTER_POOLS={
  Joker={first,second,third},mod_chosen_pool={second,third},
  Consumeables={owned_consumables[1].config.center,owned_consumables[2].config.center,owned_consumables[3].config.center},
  Tarot={owned_consumables[1].config.center},Planet={owned_consumables[2].config.center},Spectral={owned_consumables[3].config.center},
  Enhanced={{key='m_bonus',set='Enhanced'},{key='m_mult',set='Enhanced'}},
  Edition={{key='e_foil',set='Edition'},{key='e_holo',set='Edition'}},
  Seal={{key='Gold',set='Seal'},{key='Red',set='Seal'}},
  Voucher={{key='v_first',set='Voucher'},{key='v_second',set='Voucher'}},
  Booster={{key='p_first',set='Booster',kind='Arcana',config={extra=3,choose=1}},{key='p_second',set='Booster',kind='Celestial',config={extra=5,choose=2}}},
  Tag={{key='tag_first',set='Tag'},{key='tag_second',set='Tag'}}
 },P_CENTERS={m_bonus={key='m_bonus'},e_foil={key='e_foil'}},P_TAGS={},I={CARD={}}}
G.GAME.hands={['High Card']={visible=true,played=1},['Pair']={visible=true,played=3},['Flush Five']={visible=false,played=0}}
G.handlist={'High Card','Pair','Flush Five'}
event_queue={}
function Event(event) return event end
G.E_MANAGER={add_event=function(self,event) event_queue[#event_queue+1]=event end}
function run_events()
 while #event_queue>0 do local event=table.remove(event_queue,1);assert(event.func()) end
end
for _,pool in pairs(G.P_CENTER_POOLS) do
 for _,center in ipairs(pool) do G.P_CENTERS[center.key]=center end
end
SMODS.Stickers={eternal={},rental={},perishable={}}
SMODS.Rarities={rare={key='rare',original_key=3},common={key='common',original_key=1}}
SMODS.ConsumableTypes={Tarot={},Planet={},Spectral={},mod_customset={}}
SMODS.Seals={Gold={key='Gold'},Red={key='Red'}}
SMODS.Tags={}
function SMODS.poll_edition() return 'e_holo' end
function SMODS.find_card(key)
 local result={};for _,c in ipairs((G.jokers and G.jokers.cards) or {}) do if c.config.center.key==key and not c.debuff then result[#result+1]=c end end;return result
end
function get_current_pool(kind) return kind=='Voucher' and {'UNAVAILABLE','v_second'} or G.P_CENTER_POOLS[kind] end
function pseudoseed(seed) return seed end
function pseudorandom_element(pool)
 if pool[1] then return pool[1] end
 local keys={};for k in pairs(pool) do keys[#keys+1]=k end;table.sort(keys);return keys[1] and pool[keys[1]] or nil
end
destroyed={}
function SMODS.destroy_cards(cards) for _,c in ipairs(cards) do destroyed[#destroyed+1]=c.config.center.key end end
observed_card={config={center={key='m_bonus'}}}
observed_joker=owned_jokers[2]
context={other_joker=observed_joker}
shop_cards={}
for _,spec in ipairs({{'planet','Planet'},{'tarot','Tarot'},{'spectral','Spectral'},
 {'enhanced','Enhanced'},{'plain','Default'},{'joker','Joker'},{'voucher','Voucher'},
 {'arcana','Booster','Arcana'},{'celestial','Booster','Celestial'},{'spectral_pack','Booster','Spectral'},
 {'standard_pack','Booster','Standard'},{'buffoon','Booster','Buffoon'},{'custom','mod_Runes'}}) do
 local c=option_card(spec[1],spec[2]);c.config.center.kind=spec[3]
 if spec[1]=='custom' then c.config.center.consumeable=true end
 shop_cards[#shop_cards+1]=c
end
G.I.CARD=shop_cards
"""
DECK_RUN_STATE = """
-- Back:apply_to_run executes before Game:start_run copies starting_params into
-- round counters and constructs fresh CardAreas. Use the installed game's
-- defaults and the same overwrites, including the discard reduction on stake 5.
G={GAME={starting_params={hands=4,discards=3,consumable_slots=2,joker_slots=5},
 round_resets={hands=4,discards=3},current_round={hands_left=4,discards_left=3},
 interest_cap=25,interest_amount=1,modifiers={},dollars=100},
 C={GREEN=1,RED=1,BLUE=1,MONEY=1}}
event_queue={}
function Event(event) return event end
G.E_MANAGER={add_event=function(self,event) event_queue[#event_queue+1]=event end}
function run_events()
 while #event_queue>0 do local event=table.remove(event_queue,1);assert(event.func()) end
end
function finish_deck_startup()
 G.GAME.round_resets.hands=G.GAME.starting_params.hands
 G.GAME.round_resets.discards=G.GAME.starting_params.discards
 G.consumeables={cards={},config={card_limit=G.GAME.starting_params.consumable_slots}}
 G.jokers={cards={},config={card_limit=G.GAME.starting_params.joker_slots}}
 G.GAME.current_round.discards_left=G.GAME.round_resets.discards
 G.GAME.current_round.hands_left=G.GAME.round_resets.hands
end
function ease_hands_played(amount) G.GAME.current_round.hands_left=G.GAME.current_round.hands_left+amount end
function ease_discard(amount) G.GAME.current_round.discards_left=G.GAME.current_round.discards_left+amount end
function hand_payout() return G.GAME.current_round.hands_left*(G.GAME.modifiers.money_per_hand or 1) end
function interest_payout() return G.GAME.interest_amount*math.min(math.floor(G.GAME.dollars/5),G.GAME.interest_cap/5) end
SMODS.pseudorandom_probability=function() return true end
"""
POPULATED = """
local c1={base={id=2,suit='Hearts',nominal=2},config={center={key='m_bonus',rarity=1}},edition={key='e_foil',foil=true},seal='Gold',sell_cost=4}
local c2={base={id=14,suit='Spades',nominal=11},config={center={key='c_base',rarity=2}},sell_cost=7}
local c3={}
card=c1
context={scoring_name='Pair',scoring_hand={c1,c2},full_hand={c1,c2,c3}}
G={
 deck={cards={c1,c2,c3}},playing_cards={c1,c2,c3},play={cards={c1,c2,c3}},
 hand={cards={c1,c2},config={card_limit=8}},discard={cards={c3}},
 jokers={cards={c1,c2},config={card_limit=5}},consumeables={cards={c1},config={card_limit=2}},
 GAME={dollars=25,starting_deck_size=52,current_round={hands_left=3,discards_left=2,hands_played=1,discards_used=1},round_resets={ante=2,hands=4,discards=3},
 hands={['Pair']={played=3,level=2,visible=true},['High Card']={played=1,level=1,visible=true}},last_hand_played='Pair',
 blind={chips=600,mult=2},interest_amount=1,skips=2,
 consumeable_usage_total={tarot=2,planet=3,spectral=1},consumeable_usage={foo={set='Tarot',count=2}}},
 SETTINGS={profile=1},PROFILES={{high_scores={hand={amt=100},furthest_round={amt=4},furthest_ante={amt=3},poker_hand={amt=2},win_streak={amt=1},collection={amt=5,tot=100}},
 progress={overall_tally=2,overall_of=10,discovered={tally=5,of=100},challenges={tally=1,of=20},joker_stickers={tally=2,of=150},deck_stakes={tally=1,of=40}},
 career_stats={},joker_usage={foo={count=3}},consumeable_usage={},voucher_usage={}}},P_CENTERS={foo={}}
}
"""
GAME_STATES = {
    "no_game": "G=nil;context=nil;card=nil;",
    "empty_game": "G={};context={};card={};",
    "collection_game": "G={SETTINGS={profile=1},PROFILES={{}},hand={},deck={},jokers={},consumeables={}};context=nil;card=nil;",
    "partial_game": "G={GAME={},hand={},deck={},jokers={},consumeables={},SETTINGS={},PROFILES={}};context={};card={};",
    "populated_game": POPULATED,
}
EDITION_STATES = {
    "missing_card": ("context={};", None),
    "ordinary_card": ("context={other_card={}};", None),
    "foil_card": ("context={other_card={edition={key='e_foil',foil=true}}};", "e_foil"),
    "holo_card": ("context={other_card={edition={key='e_holo',holo=true}}};", "e_holo"),
    "custom_card": ("context={other_card={edition={key='e_mod_sparkle',mod_sparkle=true}}};", "e_mod_sparkle"),
    "external_card": ("context={other_card={edition={key='e_other_shiny',other_shiny=true}}};", "e_other_shiny"),
}
KNOWN_VALUES = {
    "cards_in_deck": 3, "total_playing_cards": 3, "current_hand_size": 8,
    "cards_in_hand": 2, "cards_in_discard": 1, "joker_count": 2,
    "current_money": 25, "dollars": 25, "hands_remaining": 3, "discards_remaining": 2,
    "twos_in_deck": 1, "aces_in_hand": 1, "foil_cards_in_deck": 1,
    "bonus_cards_in_deck": 1, "gold_sealed_cards_in_hand": 1,
    "pair_level": 2, "pair_played": 3, "cumulative_chips": 13,
    "round_winned_joker1": 3, "round_winned_joker5": 0,
    "progress": 20, "money_per_5": 5,
}


def run_checks(lua, cases, lua_library):
    checks = 0
    for case in cases:
        if case["kind"] in ("description_layout", "description_format"):
            for scenario in ("collection_game", "populated_game"):
                for tooltip_card in ("nil", "{}", "{ability={extra={}}}"):
                    source = HELPERS + GAME_STATES[scenario] + "\ncontext=nil;" + case["code"]
                    source += "\n" + description_localization_runtime(lua_library)
                    source += f"\nlocal vars=test_definition:loc_vars({{}},{tooltip_card}).vars;"
                    expected_value = 53 if scenario == "populated_game" else 3
                    source += f"assert(vars[1]=={expected_value} and vars[2]=={expected_value},'game variables must resolve before localization');"
                    if case.get("localization"):
                        source += "local translations=(function()\n" + case["localization"] + "\nend)();"
                        source += "local center=translations.descriptions.Joker.j_mod_runtime_test;"
                    else:
                        source += "local center=test_definition.loc_txt;"
                    expected = [line.replace("#1#", str(expected_value)).replace("#2#", str(expected_value))
                                for line in case["expected"]]
                    source += "local expected=" + "{" + ",".join(json.dumps(line, ensure_ascii=False) for line in expected) + "};"
                    source += "assert(#center.text==#expected,'exported description line count changed');"
                    for index, line in enumerate(case["expected"], 1):
                        source += f"assert(center.text[{index}]=={json.dumps(line, ensure_ascii=False)},'exported description line changed');"
                    if case["kind"] == "description_format":
                        source += "vars.colours={'dynamic_text','dynamic_background'};"
                        source += "local expected_parts=" + lua_data(case["parts"]) + ";"
                        source += "if native_description_localization then assert_description_format(center,vars,expected_parts) end;return 1"
                    else:
                        source += "if native_description_localization then assert_description_layout(center,vars,expected) end;return 1"
                    try:
                        evaluate(lua, source)
                    except AssertionError as error:
                        raise AssertionError(f"{case['kind']} {case['name']} / {scenario} / card={tooltip_card}: {error}") from error
                    checks += 1
            continue
        if case["kind"] == "rarity_shop":
            source = HELPERS + rarity_shop_runtime(lua_library) + "\n" + case["code"]
            source += "\n" + case["invoke"] + "\n" + case["verify"] + "\nreturn 1"
            try:
                evaluate(lua, source)
            except AssertionError as error:
                raise AssertionError(f"rarity_shop {case['name']}: {error}") from error
            checks += 1
            continue
        if case["kind"] in ("tooltip", "description_game"):
            for scenario, setup in GAME_STATES.items():
                for tooltip_card in ("nil", "{}", "{ability={extra={}}}"):
                    # Collection previews can pass no Card or an incomplete Card,
                    # and loc_vars never receives a scoring context.
                    source = HELPERS + setup + "\ncontext=nil;" + case["code"]
                    source += f"\nlocal result=test_definition:loc_vars({{}}, {tooltip_card});"
                    source += f"assert(#result.vars=={len(case['ids'])});"
                    for index, variable in enumerate(case["ids"], 1):
                        source += f"numeric(result.vars[{index}]);"
                        if case["kind"] == "description_game":
                            base = (2 if variable == "hand_level" else KNOWN_VALUES[variable]) if scenario == "populated_game" else 0
                            expected = case["starts_from"] + base * case["multiplier"]
                            source += f"assert(result.vars[{index}]=={expected}, '{variable}');"
                        elif scenario == "populated_game" and variable in KNOWN_VALUES:
                            source += f"assert(result.vars[{index}]=={KNOWN_VALUES[variable]}, '{variable}');"
                    source += "return #result.vars"
                    try:
                        evaluate(lua, source)
                    except AssertionError as error:
                        raise AssertionError(f"{case['kind']} {case['name']} / {scenario} / card={tooltip_card}: {error}") from error
                    checks += len(case["ids"])
            continue
        if case["kind"] in ("rule_options", "joker_creation", "scoring", "deck_settings", "deck_cards"):
            state = {"joker_creation": JOKER_CREATION_STATE, "deck_settings": DECK_RUN_STATE}.get(case["kind"], RULE_OPTIONS_STATE)
            source = HELPERS + EFFECT_RESOLVER + state + case.get("setup", "") + "\n" + case["code"]
            if case.get("card_selection"):
                source += "\n" + card_area_selection_runtime(lua_library)
            if case.get("post_trigger_runtime"):
                source += "\n" + POST_TRIGGER_RUNTIME
            if case.get("probability_result_runtime"):
                source += "\n" + PROBABILITY_RESULT_RUNTIME
            if case.get("consumable_creation_message_runtime"):
                source += "\n" + consumable_creation_message_runtime(lua_library)
            if case["kind"] == "scoring":
                source += "\n" + SCORING_PARAMETERS
            if case.get("card_destruction_runtime"):
                source += "\n" + CARD_DESTRUCTION_RUNTIME
            if case.get("playing_card_transform_runtime"):
                source += "\n" + PLAYING_CARD_TRANSFORM_RUNTIME
            if case["kind"] == "deck_cards":
                source += "\n" + PLAYING_CARD_TRANSFORM_RUNTIME + deck_card_runtime(lua_library)
            if case.get("booster_open_runtime"):
                source += "\n" + BOOSTER_OPEN_RUNTIME
            if case["kind"] in ("deck_settings", "deck_cards"):
                source += "\nactor={effect={center=test_definition,config=copy_table(test_definition.config or {})}};\n"
            else:
                source += "\nactor={ability=copy_table(test_definition.config or {extra={}})};actor.ability.extra=actor.ability.extra or {};\n"
            source += case.get("prepare", "") + "\n" + case["invoke"] + "\n" + case["verify"] + "\nreturn 1"
            try:
                evaluate(lua, source)
            except AssertionError as error:
                raise AssertionError(f"{case['kind']} {case['name']}: {error}") from error
            checks += 1
            continue
        if case["kind"] == "planet":
            for highlighted in (True, False):
                source = HELPERS + EFFECT_RESOLVER + POPULATED + "\ncontext=nil;G.C={GREEN=1};G.GAME.hands['Flush']={level=2,played=3};"
                source += "G.hand.highlighted=G.hand.cards;" if highlighted else "G.hand=nil;"
                source += "function localize(key) return key end; SMODS.smart_level_up_hand=function(card,hand,instant,amount) captured_amount=amount end;"
                source += case["code"] + "\ntest_definition:use({ability={extra={}}},nil,nil);return numeric(captured_amount)"
                base = 2 if case["name"] == "hand_level" else 3 if case["name"] in ("times_hand_played", "current_hand_played_count") else 13 if highlighted and case["name"] == "cumulative_chips" else 2 if highlighted else 0
                try:
                    result = evaluate(lua, source)
                except AssertionError as error:
                    raise AssertionError(f"planet {case['name']} / highlighted={highlighted}: {error}") from error
                assert result == 1 + base * 2, (case["name"], highlighted, result, base)
                checks += 1
            continue
        if case["kind"] == "edition":
            for scenario, (setup, key) in EDITION_STATES.items():
                selected = case["name"]
                if selected == "any":
                    expected = key is not None
                elif selected == "none":
                    expected = key is None and scenario != "missing_card"
                else:
                    selected = selected if selected.startswith("e_") else "e_mod_sparkle" if selected == "sparkle" else "e_" + selected
                    expected = selected == key
                if case["negate"]:
                    expected = not expected
                result = evaluate(lua, setup + "return ((" + case["code"] + ") and 1 or 0)")
                assert result == int(expected), (case["name"], scenario, result, expected)
                checks += 1
            continue
        for scenario, setup in GAME_STATES.items():
            source = HELPERS + setup + "\n"
            expected = None
            if case["kind"] == "joker":
                source += case["code"] + "\nlocal card={ability=test_definition.config};"
                source += "local result=test_definition:calculate(card, {joker_main=true}); local tooltip=test_definition:loc_vars({},card);"
                source += "numeric(tooltip.vars[1]); assert(result.mult==tooltip.vars[1]); return numeric(result.mult)"
            else:
                source += "return numeric(" + case["code"] + ")"
            try:
                result = evaluate(lua, source)
            except AssertionError as error:
                raise AssertionError(f"{case['kind']} {case['name']} / {scenario}: {error}") from error
            if case["kind"] != "game":
                expected = 9 if case["name"] == "GAMEVAR:cards_in_deck|2|3" and scenario == "populated_game" else 3 if case["name"] == "GAMEVAR:cards_in_deck|2|3" else 0
            elif case["kind"] == "game" and scenario == "populated_game":
                expected = KNOWN_VALUES.get(case["name"])
            if expected is not None:
                assert result == expected, (case["name"], scenario, result, expected)
            checks += 1
    print(f"Passed {checks} Lua 5.1 runtime checks across {len(cases)} compiler fixtures")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lua-library", default=os.environ.get("BALATRO_LUA_LIBRARY"))
    parser.add_argument("--filter", help="Run only compiler fixtures whose names contain this text")
    args = parser.parse_args()
    library = args.lua_library or ctypes.util.find_library("lua5.1") or ctypes.util.find_library("luajit-5.1")
    if not library:
        parser.error("Supply a Lua 5.1/LuaJIT shared library with --lua-library or BALATRO_LUA_LIBRARY")
    lua = load_lua(library)
    with tempfile.TemporaryDirectory(prefix="jokerforge-codegen-") as directory:
        fixtures = Path(directory) / "cases.json"
        subprocess.run([
            "cargo", "run", "--offline", "--quiet", "--manifest-path", str(ROOT / "src-tauri/Cargo.toml"),
            "-p", "balatro-codegen", "--example", "codegen_runtime_cases", "--", str(fixtures),
        ], cwd=ROOT, check=True)
        cases = json.loads(fixtures.read_text(encoding="utf-8"))
        if args.filter:
            cases = [case for case in cases if args.filter in case["name"]]
            if not cases:
                parser.error("No compiler fixture names matched --filter")
        run_checks(lua, cases, library)


if __name__ == "__main__":
    main()
