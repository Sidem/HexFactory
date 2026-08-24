# Native-core route

Start at `docs/AGENT-MAP.md` and localize declarations with `rg -n`. Do not read all of `lib.rs`.

- Simulation truth, commands, saves, checksums, and snapshot dirty marks stay native.
- `runtime.rs` is derived hot-path indexing: rebuild after topology changes; never save or hash it.
- `save_migrations.rs` is the only envelope migration entry. Add adjacent, tested version steps.
- `wire.rs` and `fixtures/snapshot-delta-wire.json` move together.
- Preserve stable-id arbitration and integer time/quantities.
- Run the narrow Rust test first, then `npm run test:rust`; wire or balance changes need their
  fixture regeneration commands from root `AGENTS.md`.
