# Practical projects — v0.35.0

Recorded 2026-08-28. This phase 2 delivery makes hub demand finite. It is the correction the
progression brief asked for and did not get in v0.23: a project pays its insight once and then
retires, so no amount of repeated ordinary delivery buys unlimited research. It ships the browsable
catalogue that finiteness requires and the budget proof that makes finiteness safe. It does not
ship evidence counters or separate player skills, which stay in slice 5.

## What was actually wrong

v0.23 shipped `repeat_insight`: a raw row paid 10 the first time and 2 for ever after. That is a
decay, not a bound. Two ratios say so:

| Quantity                                    | Value |
| ------------------------------------------- | ----- |
| Total research the catalogue must fund      | 137   |
| First-fill income from all 22 requests      | 572   |
| First-fill income from the 8 raw rows alone | 73    |

The game was never short of income — it had 4× the research bill in first fills alone. What it
lacked was an end. A 2-insight floor slows a farm; it does not stop one, and a bottom rung that is
infinite makes every gradient above it advisory. Making demand finite reaches the goal without a
floor that has to be tuned against a curve nobody has measured in play.

## Finite demand

`repeat_insight` is deleted from all eight rows that carried it and from `RequestDefinition`,
`requestsView`, and the definition validator. `request_payout` is gone with it; a project pays
`insight`, once.

Progress moved with the payout. It used to live on the board slot as `RequestState.delivered`, which
was affordable when a skipped row would come back: under finite demand, forfeiting delivered goods
destroys value that cannot be earned again. It now lives on the project, in
`Core::request_delivered`, keyed by request id. Passing a part-filled project keeps every unit put
into it, and the same total is waiting when the project is posted again.

Two guards, both of which a test drives:

- **Posting refuses a complete project.** `post_request` rejects complete, unreachable and
  already-posted projects by name.
- **The till refuses one too.** `credit_requests` re-checks completeness before paying. Posting
  already gates this, so reaching it means an edited save or a slot that survived a migration it
  should not have — and the failure mode is minting insight without bound, which is the one thing
  finite demand exists to prevent. Cheaper to refuse it at the till than to trust every path in.

## A browsable, pinnable catalogue

A finite board can hide the only route forward, which a repeating board could not. So the snapshot
now publishes the whole catalogue rather than the three posted slots: every project carries a
`ProjectState` of `locked`, `available`, `posted` or `complete`, posted rows first in slot order and
the rest in catalogue order. The projects panel lists all 22 with a running `done/total` count and
the insight still on the table, and any `available` row can be posted by name.

`locked` is shown greyed rather than hidden, deliberately. A finite catalogue should read as a bill
of work with an end — that is the point of it being finite, and a hidden row is indistinguishable
from a row that does not exist.

Posting displaces the posted slot with the least delivered against it, ties broken by slot index, so
the choice costs the player the least committed work available. The displaced row's `request_rounds`
advances as if it had been skipped. Guidance now names only posted rows: a step naming a finished
project, or one the player cannot make yet, is an instruction they cannot carry out.

The command names a project by id, matching `research { technology_id }` beside it, so the numeric
opcode channel stays coherent. Opcode 28.

## The budget, proved in native

`validate_research_budget` runs inside `validate_all`, so a definition edit that breaks the budget
fails at load rather than in play:

| Quantity                         | Value  |
| -------------------------------- | ------ |
| Projects in the catalogue        | 22     |
| Total insight the catalogue pays | 572    |
| Purchasable research cost        | 137    |
| Surplus                          | 435    |
| Surplus ratio                    | 4.175× |
| Insight reachable by hand alone  | 73     |
| Technologies granted, not bought | 4      |

The four grants — field logistics, automated extraction, storage planning, on-site power — come from
the founding commission and are excluded from the 137, as v0.31.0 established.

The invariant requires at least 1.25×. The margin is deliberately wide: at 4.175× a player can buy
in a wrong order, strand insight in a branch they do not need, and still finish the tree, which is
what stops purchase order from being a puzzle with one solution. It is generous, and it is not
repriced here — tightening it needs measured play this milestone does not have, and repricing
without that would be a claim with no measurement behind it.

**Hand-gathering cannot substitute for processing.** The eight hand-reachable projects pay 73
against 137 of research, so the raw catalogue cannot buy the tree even if a player exhausts it. Per
gather the ladder also still holds: the best raw row pays 1250 milli-insight per gather against 1300
for the cheapest processed one. Per _minute_ raw rows still pay better — a gather is quick and a
furnace is not — and that is now fine, because finiteness rather than rate is what bounds them. The
per-minute clause the old test carried only ever passed because it compared against the 5×-lower
repeat rates, and it came out with them.

## Envelopes and migration

Save 26 advances to 27, definitions 21 to 22, wire 13 to 14. Technologies 11, scenarios 7 and
world 8 stay put.

`practical_projects_26_to_27` lifts each slot's non-zero `delivered` onto
`state.request_delivered`, keyed by project id, and drops it from the slot. A project already in
`request_fills` is skipped: it has been paid, it is complete, and crediting it progress would show a
finished project as part-filled. Slots keep their `request_id`. Zero-delivered slots contribute
nothing.

The checksum moved from 2_222_187_037 to 3_614_679_184 because `request_delivered` is now hashed
under its own section. The workload's shape, entity count and delivered total did not move; only
where the count is stored did.

`ProjectState` serializes lowercase in both the JSON snapshot and the binary delta, so the two
transports agree. The Rust decoder for it is `#[cfg(test)]` — the shipping reader is
`snapshotWire.ts` — following the precedent every other wire enum sets.

## Limits

- **One item per project.** A project still asks for a quantity of a single item. Multi-item bills
  were considered and deferred; they are a definition-schema change with its own envelope step and
  no evidence yet that the single-item bill is what limits the board.
- **No evidence counters or provenance.** Slice 4 as scoped also wanted insight to require evidence
  of how an item was produced. Nothing here distinguishes a plate a player carried in from one a
  smelter made. That remains open.
- **The 572 is not repriced.** See above: the surplus is generous by choice and by lack of measured
  play, not by accident.
- **No player-skill separation.** Expanded Pack and Surveyed Construction still spend factory
  insight. Slice 5.
- **The catalogue is not sized against playtime.** 22 projects funding 137 of research is an
  arithmetic claim about the budget. It is not a claim about how long a run takes, how long the
  board stays interesting, or what happens to a player who exhausts it — the game has no content
  after the last project beyond the factory itself, and that is a known edge, not a solved one.

## Verification

The local quality gate passed dependency audit, generated-map consistency, formatting, lint, types,
255 TypeScript tests and 198 Rust tests, and a production Wasm/Vite build. The existing Vite
large-chunk warning remains.

New coverage: a filled project never pays again; a part-filled project keeps its progress across a
pass; posting displaces the least committed slot; progress against an unposted project survives a
save round trip; the board empties once the catalogue is exhausted; the finite catalogue funds the
whole research tree, counted as whole distinct projects; and the save 26 → 27 migration lifts
delivered progress onto the project while leaving an already-paid project and a zero-delivered slot
alone.

No performance or play-feel claim is made by this release.
