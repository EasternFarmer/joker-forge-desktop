# balatro-codegen test layout

This crate uses three complementary test layers:

1. Test modules under `src/` (unit tests)
- Purpose: verify private/internal compiler behavior.
- Examples: `src/compiler/tests.rs` for trigger chaining and fallback ordering,
  and the test module in `src/compiler/consumable.rs` for consumable variable behavior.

2. `tests/lua_example_snapshots.rs` (integration tests)
- Purpose: verify public API outputs against JSON inputs and expected Lua snapshots
  for each supported object family.

3. `scripts/test-codegen-lua.py` (Lua runtime regressions)
- Runs actual compiler output through a Lua 5.1/LuaJIT shared library.
- Covers every editor game variable with absent, partial and populated game state,
  tooltip callbacks, complete Joker calculations, and Card Edition matching.
- Covers flag changes/checks, consumable variable mutations including chance and
  loop groups, and global state across rounds, saved-run loads and new runs.
- Consumable effects use the bundled Steamodded effect resolver in these tests.
- Uses `examples/codegen_runtime_cases.rs` to generate fixtures, so the runner
  evaluates the current compiler instead of hand-written copies of its output.

## Shared fixtures

- Reuse existing helpers within each test module before duplicating object/rule setup.
- Snapshot-style Lua fixtures live in `tests/lua-code-examples/**`.
  - Each case uses a `*.json` input spec and a matching `*.lua` expected output.
  - The `*.lua` file starts with contributor comments, then expected generated Lua.
  - Integration test: `tests/lua_example_snapshots.rs`.

## Expansion guidelines

- Add internal algorithm regressions under `src/compiler/tests.rs` or the relevant compiler module.
- Prefer focused tests with one assertion theme each (branch order, trigger filtering, emitted section markers, etc.).
- For broad output coverage, prefer adding a new case in `lua-code-examples` instead of adding another ad-hoc `contains(...)` assertion.

## Suggested pattern for new suite files

- `*_compiles_and_has_core_hooks` tests:
  - Assert object constructor token (`SMODS.X`) and required hooks (`calculate`, `use`, `redeem`, `apply`).
- `*_same_trigger_conditional_precedes_unconditional_fallback` tests:
  - Build two same-trigger rules (one conditional, one unconditional).
  - Assert conditional payload appears before fallback payload in generated Lua.

## Commands

- From the repository root, run compiler tests and export input checks:
  - `npm run test:codegen`
- Run generated Lua against the same LuaJIT runtime shipped with Balatro:
  - `npm run test:codegen:lua -- --lua-library "C:/Program Files (x86)/Steam/steamapps/common/Balatro/lua51.dll"`
  - Requires Python and a matching-architecture Lua 5.1/LuaJIT shared library.
    Other installations can supply `--lua-library` or `BALATRO_LUA_LIBRARY`.
- Run only crate tests from the repository root:
  - `cargo test --manifest-path src-tauri/Cargo.toml -p balatro-codegen`
- Regenerate Lua snapshot bodies from JSON specs:
  - PowerShell: `$env:UPDATE_LUA_EXAMPLES='1'; cargo test --manifest-path src-tauri/Cargo.toml -p balatro-codegen lua_codegen_matches_examples`
  - Bash: `UPDATE_LUA_EXAMPLES=1 cargo test --manifest-path src-tauri/Cargo.toml -p balatro-codegen lua_codegen_matches_examples`
- Run the frontend build from the repository root:
  - `npm run build`
