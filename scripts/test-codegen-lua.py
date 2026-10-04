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
        if case["kind"] in ("rule_options", "joker_creation", "scoring", "deck_settings"):
            state = {"joker_creation": JOKER_CREATION_STATE, "deck_settings": DECK_RUN_STATE}.get(case["kind"], RULE_OPTIONS_STATE)
            source = HELPERS + EFFECT_RESOLVER + state + case.get("setup", "") + "\n" + case["code"]
            if case.get("card_selection"):
                source += "\n" + card_area_selection_runtime(lua_library)
            if case.get("post_trigger_runtime"):
                source += "\n" + POST_TRIGGER_RUNTIME
            if case["kind"] == "scoring":
                source += "\n" + SCORING_PARAMETERS
            if case.get("card_destruction_runtime"):
                source += "\n" + CARD_DESTRUCTION_RUNTIME
            if case["kind"] == "deck_settings":
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
