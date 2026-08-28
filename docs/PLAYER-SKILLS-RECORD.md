# Player Skills — v0.36.0

The next Phase 2 slice of `PROGRESSION-PLAN.md`: personal capabilities now have their own
definitions, native state, purchase command, currency and Skills window. This completes the
research-map / finite-project / initial-skills phase. Construction materials and ground works
are next; later skill branches, evidence counters and generated icon families remain future work.

## Rules and budget

Expanded Pack adds four slots; Surveyed Construction adds three hexes of construction reach.
Both cost one Skill Point and either can be bought first. The ordinary new-game base remains
eight slots and five hexes. Placement, obstruction, inventory and work rules remain native.
The existing build-mode ring displays the resulting native reach.

Three different native completion events each award one point, once per save:

| Milestone      | Completion event                                   |
| -------------- | -------------------------------------------------- |
| Made by hand   | A manual workshop finishes a batch                 |
| Prove the line | The `components` founding commission completes     |
| Power at work  | An electrically powered processor finishes a batch |

Failed work, gathering, idling, rebuilding, repeated batches and repeated deliveries do not
pay additional points. Completion is observed where native work finishes, without scanning
entities or world cells for achievements. Stable milestone IDs and completion flags are saved.
The third point provides an alternate route and covers older saves whose completed commission
cannot pay again. There is no conversion to insight, respec or infinitely repeatable rank.

`fixtures/balance.json` records 3 points against 2 points of skill costs. Removing the two
personal purchases from research reduces its cost by 17 insight: 137 → 120. Finite project
income stays 572, with 73 available from raw projects. The new research surplus is 452 and
the income/cost ratio is 4.767 to three decimals. Construction bills, recipe yields, opening
accounting and production rates are unchanged in the fixture. These are arithmetic results,
not measured playtime or a claim that the whole progression curve has been playtested.

## Interface

Skills is a sibling of Research, accessible from the command bar or **K**. Two labelled cards
show the exact native current/resulting capacity, benefit, cost, availability and purchase
control. The complete milestone list explains how points are earned; a direct shortcut jumps
to it. Research and Skills cross-link without mixing currencies.

Cards and milestones are keyed and patched in place. An acquired upgrade keeps its focused
button, marked `aria-disabled`, and a polite status announces the new balance. A native modal
contains focus; closing restores the opener. The header and navigation stay visible while
the content scrolls. Cards stack at phone widths. The command-bar button retains an accessible
name when narrow layouts hide its visual label. Existing pack/reach emblems are reused; this
is not the deferred generated-icon pass.

## Native ownership, validation and transport

`skills` and `skill_milestones` are separate typed arrays in technology envelope 12. They are
not technology rows and cannot be bought with research commands. Technology effects no longer
accept player-capability bonuses. Both validators reject duplicate IDs, missing or cyclic
prerequisites, invalid costs/effects and insufficient point budgets. Lists are bounded at 64.
Milestones use a closed set of native event types; ranks use supported bounded player effects.

The native `purchase_skill` command and snapshot share one availability function, including
the exact resulting capability. This also handles a legacy pack already widened beyond the
earned floor. Points, purchases, grants, milestone completions and sandbox provenance are
checksummed. Derived availability is not. Wire 15 adds a bounded skills group; ordinary frames
reuse it unless skills or player capacities change. The cross-language wire fixture includes
non-empty skill state, multi-byte IDs and missing prerequisites.

## Saves and Creative

Save 28 / technologies 12 / wire 15; definitions 22 / scenarios 7 / world 8 remain unchanged.
The adjacent save-27 migration changes only envelope versions before the original checksum is
verified. After verification, old technology IDs 18 and 19 become equivalent granted skills and
leave the research set. Bonuses are neither lost nor doubled. Insight is unchanged and the
conversion gives no spendable points. Saving and reloading does not replay the conversion.

Completed historical commissions, including those closed by older bill adjustments during load,
are marked without payment. No manual-workshop or powered-craft
history is invented: those milestones can be earned through newly observed work. Thus an older
save missing either skill can still earn both from the two manufacturing milestones.

Turning on Creative grants remaining skills separately from earned purchases and permanently
marks this save as a sandbox for skill rewards. Turning it off retains those capabilities but
cannot mint points from the prepared factory. Existing earned points and purchases remain.
Older saves only recorded the current Creative flag; the migration cannot reconstruct an
unrecorded historical Creative session. No stronger provenance claim is made for those files.

## Verification

Native tests cover atomic failures, either purchase order, isolated currencies, repeated events,
actual workshop/powered production and commission completion, capability bounds, save round trips,
all four legacy ownership combinations, checksum tampering, creative return, and dirty-delta/full
snapshot equivalence. The adjacent historical migration chain remains exercised. TypeScript tests
cover definitions, native availability presentation, command encoding, delta application, wire
decoding, separate budgets and save-picker compatibility through save 27.

Browser checks use the real worker/Wasm build and native-generated save fixtures, not edited
browser state. A native workshop fixture demonstrates a one-point purchase and an 8 → 12 slot
pack with unchanged insight. Desktop and 390px layouts, milestone scrolling, retained purchase
focus, research navigation and legacy-save loading are checked before release. The test-only
`UPDATE_SKILL_BROWSER_FIXTURES=1` option exports synthetic fixtures under Rust `target/skills-browser`;
no user saves or generated binaries are committed.

The complete `npm run quality` gate passed: 259 TypeScript tests, 204 Rust tests, dependency
audit (zero vulnerabilities), map/format/lint/type checks, Wasm and production builds. Vite retains
its large-chunk advisory; this is not a new performance measurement. Browser keyboard events were
untrusted synthetic events, so native Enter/Space button activation is not claimed as tested;
Escape handling and focus restoration were verified. No performance, hardware-capacity,
timed-opening or comprehensive screen-reader claim is made by this release.
