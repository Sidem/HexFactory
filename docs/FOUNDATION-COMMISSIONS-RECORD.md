# Foundation commissions — v0.31.0

Phase 1 delivery, 2026-08-27. This is the opening-grant and typed-effects slice of the
[progression plan](PROGRESSION-PLAN.md). It is not the finite insight economy, separate skill
budget, OR prerequisite groups, or remaining industrial construction bills.

## What ships

Completing **Prove the line** — the first founding-contract stage, three components made at the
manual workshop — grants Field Logistics, Automated Extraction, Storage Planning and On-site
Power. Native issues the grant as it closes the stage. Insight is neither charged nor refunded.
Those four technologies can no longer be bought. Creative still unlocks the whole tree.

The four nodes remain independent graph roots. Spending insight on a favourite later branch cannot
miss this backbone, because the backbone is not for sale. Composition, Material Processing and
everything beyond still cost insight with the same prices and AND prerequisites as before.

Each technology now carries a typed `effects` list: unlock a building, add cargo slots, or extend
build reach. Corner headings stay on the building definition. Grant-only nodes declare
`grant: { kind: "contract_stage", key, name }` and cost 0. The research map, list, hover and
guidance consume that catalog plus native completion: they do not offer a purchase button, and
they name the commission instead of an insight price.

Guidance for the opening names the workshop and the delivery, not Field Logistics. After the
grant, the foundry stage names Material Processing when kiln work is next.

## Economy and compatibility

Purchasable research still totals 137 insight (was 153). Request rows, recipes and construction
bills are unchanged. Regenerating `fixtures/balance.json` drops the now-granted starter
automation from opening insight: first smelter 15→6, first power 4→0, first extractor 9→0, first
composer 17→8 and first circuit 23→14. Their modeled gather totals are respectively 35, 28, 36, 46
and 74 (previously 43, 36, 46, 58 and 82). The foundry contract opening moves 15→6 insight and
108→100 modeled gathers. Machine, item, power, construction and request figures that do not depend
on those four prices stay the same. These harness estimates exclude walking and are not timed
player sessions.

Save 23 / technologies 11 / scenarios 6; definitions 18, world 8 and wire 13 are unchanged.
The adjacent save migration advances technology 10 to 11 and scenario 5 to 6. A factory whose
`contract_stage` is already past Prove the line receives the four IDs if they were missing;
stage-zero factories, insight, stock, jobs and checksums are otherwise untouched. The picker
exposes the exact released migration chain from save 14 onward.

## Verification and limits

Native tests grant the four technologies on stage completion without charging insight, refuse
insight purchases of grant-only nodes, preserve a stage-zero factory's checksum through the
migration, and merge the grants onto a completed-commission save. Existing research, contract,
replay, wire and full-diff tests remain.

TypeScript checks typed-effect validation, grant-key references, native-only purchase
availability (including `purchasable`), guidance that never names a grant-only research step,
and the released save-picker chain through save 23.

No opening-time, player-study or performance improvement is claimed. Finite insight projects,
separate skills, OR groups, and the industrial bills for smelter, kiln, cutter, crusher and pump
remain unshipped.
