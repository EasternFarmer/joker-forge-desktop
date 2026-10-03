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
function numeric(value)
 assert(type(value)=='number', 'expected number, got '..type(value))
 assert(value==value and value~=math.huge and value~=-math.huge, 'nonfinite number')
 return value
end
SMODS = {
 Joker=function(definition) test_definition=definition end,
 Consumable=function(definition) test_definition=definition end,
 has_enhancement=function(card, key) return card.config and card.config.center and card.config.center.key==key end,
 get_enhancements=function(card)
  if card.config and card.config.center and card.config.center.key~='c_base' then
   return {[card.config.center.key]=true}
  end
  return {}
 end
}
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


def run_checks(lua, cases):
    checks = 0
    for case in cases:
        if case["kind"] == "planet":
            for highlighted in (True, False):
                source = HELPERS + POPULATED + "\ncontext=nil;G.C={GREEN=1};G.GAME.hands['Flush']={level=2,played=3};"
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
            if case["kind"] == "tooltip":
                # Tooltip callbacks have no scoring context, even during a run.
                source += "context=nil;" + case["code"] + "\nlocal result=test_definition:loc_vars({}, {ability={extra={}}});"
                source += f"assert(#result.vars=={len(case['ids'])});"
                for index, variable in enumerate(case["ids"], 1):
                    source += f"numeric(result.vars[{index}]);"
                    if scenario == "populated_game" and variable in KNOWN_VALUES:
                        source += f"assert(result.vars[{index}]=={KNOWN_VALUES[variable]}, '{variable}');"
                source += "return #result.vars"
            elif case["kind"] == "joker":
                source += case["code"] + "\nlocal card={ability=test_definition.config};"
                source += "local result=test_definition:calculate(card, {joker_main=true}); local tooltip=test_definition:loc_vars({},card);"
                source += "numeric(tooltip.vars[1]); assert(result.mult==tooltip.vars[1]); return numeric(result.mult)"
            else:
                source += "return numeric(" + case["code"] + ")"
            try:
                result = evaluate(lua, source)
            except AssertionError as error:
                raise AssertionError(f"{case['kind']} {case['name']} / {scenario}: {error}") from error
            if case["kind"] not in ("game", "tooltip"):
                expected = 9 if case["name"] == "GAMEVAR:cards_in_deck|2|3" and scenario == "populated_game" else 3 if case["name"] == "GAMEVAR:cards_in_deck|2|3" else 0
            elif case["kind"] == "game" and scenario == "populated_game":
                expected = KNOWN_VALUES.get(case["name"])
            if expected is not None:
                assert result == expected, (case["name"], scenario, result, expected)
            checks += len(case["ids"]) if case["kind"] == "tooltip" else 1
    print(f"Passed {checks} Lua 5.1 runtime checks across {len(cases)} compiler fixtures")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lua-library", default=os.environ.get("BALATRO_LUA_LIBRARY"))
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
        run_checks(lua, json.loads(fixtures.read_text(encoding="utf-8")))


if __name__ == "__main__":
    main()
