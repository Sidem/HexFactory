# v0.42.0 — Straight Walls and Yards

Phase 6 is delivered. Boundaries leave the hex edge chain for the vertex lattice, and the two
rectangle tools the change makes possible ship with it. Phase 7, supported floors and vertical
transport, remains next; no later phase was started.

## Delivery

- A boundary is a **chord of one hex between two of its corners**, not one of its three shared
  edges. `CHORDS` is 15: `0..3` are the edges the hex owns, `3..6` the other three edges rewritten
  onto the neighbour that owns them, `6..12` the six short diagonals and `12..15` the three long
  diagonals. `Segment::new` folds every spelling of a chord onto one canonical record, so a segment
  still has a single identity from either side.
- **Twelve headings at 30° run dead straight.** `straight_boundary_runs_hold_all_twelve_headings_for_twenty_segments`
  asserts a twenty-segment run in each of the twelve, which is the phase's stated acceptance.
  `chord_chain` reaches any other vertex as well — it steps greedily, minimising drift before
  closing distance — so an off-heading drag staircases toward the far end rather than being refused.
  The tray says which of the two the player is about to get before the run is paid for.
- **A rectangular yard closes from two picked corners.** `yard_rect` snaps both to the vertex ladder
  — columns one hex wide, rows following the alternating one-and-two-radius rise that repeats every
  three — so all four corners land on lattice vertices and all four sides are exactly straight runs.
  A rectangle that snaps to zero width or zero height is refused rather than drawn as a line.
- **Ground works takes the same two corners.** `hexes_touching_rect` is an integer separating-axis
  test over five axes; every hex whose prism meets the rectangle is taken in, including one that
  meets it only at a corner. That is the requested rule, and it is why a yard is paved generously
  and fenced exactly: the same two anchors give a surface that runs under the fence line and a fence
  that sits on the rectangle itself. Both tools stay bounded at 32 (`MAX_GROUND_CELLS`,
  `MAX_BOUNDARY_SEGMENTS`), refused before anything is priced.
- **The trays are shaped around the pick, not around the data.** Walls offers three selections — one
  side of a hex, straight run, rectangular yard — where the first is a host-only convenience that
  sends native a one-segment `line`. Picking a corner raises gold anchor pins in the scene on the
  first click, before there is any run to price, and the ground tray raises the same pins on the same
  vertices. A gate is one segment at a time, so on a run or a yard its card is shown disabled and
  labelled `Gate · one side at a time` rather than hidden. Precise-placement fields swap with the
  shape: hexes for a line, hexes and corners for a rectangle.
- **Drawing measures each rail off the chord it spans.** A long diagonal is twice an edge and a short
  diagonal about 1.73 times one; rails, panels and braces take their length from the segment
  endpoints, and a segment's height is the highest terrain under the three hexes its anchor touches.

**Save 32 → 33 and wire 17 → 18.** Definitions 26, technologies 14, scenarios 7 and world 10 are
unchanged. The save step is a version stamp only: the three shared edges are the first three chords
under the numbers they always had, so a version-32 boundary is already a version-33 boundary. The
field is read through `#[serde(alias = "direction")]` rather than rewritten, which leaves the
checksum's input untouched.

## Verification

The full gate ran green: audit, agent-map check, prettier, eslint, tsc, the vitest suite, the cargo
suite, the wasm-pack build and the vite build. Focused coverage added with the change: chord identity
and canonicalisation, the twelve-heading straight-run assertion, rectangle snapping and its
degenerate refusal, the hex-touching separating-axis test, the nine- and ten-argument command layouts
with corner validation, the wire fixture at version 18, and a rendering assertion that a long
diagonal's rail is more than 1.9× the width of an edge's.

Browser session, Windows Chromium, dev server, Low profile:

1. Resumed a save and read **Save 33** in the title footer.
2. In Walls, each selection swapped the precise-placement fields correctly and carried its own
   prompt. `Timber gate` was disabled on a run and on a yard, labelled `Gate · one side at a time`.
3. A two-click straight run reported `Choose the far end. Preview: 4 segments.` then `4 of 4
segments will change. Floor space stays free.`, with `Heading north · dead straight.` above it.
   Native refusals surfaced verbatim, including `Hex 2, -1: Boundaries need dry, buildable ground on
both sides`, `Walk closer: boundary is outside build reach` and `Not enough materials in your pack
for this entire selection`.
4. In Ground works, the first corner of a rectangle raised one anchor pin and native answered `Drag
out a rectangle at least one hex across`. A rectangle from north of −2, −1 to south of −1, 0
   previewed **14 hexes** and applied — `Prepared 14 hexes of ground` — after which the same
   selection read `This ground already matches. Nothing to spend, dig or recover.`
5. A rectangle overlapping a cliff was refused by the offending hex: `Hex 1, -1: Ground works need
dry, buildable land`.
6. The same two corners in the Walls tray as a rectangular yard previewed **8 segments** and applied
   — `Updated 8 boundary segments` — drawing a closed timber rectangle sitting on the paved yard.

## Measurement and limits

**No performance claim is made for this release and no benchmark harness was run for it.** The
committed capacity ladder and the v0.41.0 paving harness are unchanged and are the only measured
numbers that stand.

- A hex that meets the rectangle at a single corner is still taken in. "Touches" is non-strict, by
  design and by request, so a rectangle can reach one hex further than it looks.
- Off-heading runs are not refused; they staircase, within half a hex of the intended line. The
  bounded-deviation claim is the model's, not a measurement of a particular drag.
- Gates remain one segment per placement. Opening a whole run of them is still one click each.
- Roofs, rebar and steel frames are not here. They are structural and belong to phase 7.
- Occupancy is unchanged: a segment blocks movement and transport across itself and reserves nothing
  else, so a hex a wall cuts through remains buildable.
- No screen-reader audit and no physical-touch audit is claimed for the new trays.
- The browser session above was one desktop run on the dev server. It is verification that the paths
  work end to end, not a qualification run.
