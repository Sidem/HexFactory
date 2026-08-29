# v0.44.0 — Emblems and Clarity

Phase 7, the icon pass, closed. The catalogue used to identify a machine with a three-letter stamp
cut from its own name, which is a label rather than a picture: it reads at the speed of text and
tells a player nothing they did not already know from the name printed beside it. This release
replaces every one of those stamps with a drawing, under a contract strict enough that twelve
unrelated glyphs read as one set.

Nothing in it is simulation. No emblem key reaches a save, a checksum, a native definition or the
wire, and every envelope is unchanged.

## Delivery

- **One contract, published at the top of `src/rendering/emblems.ts`.** Every emblem is a 32×32
  `viewBox` with `fill="none"`, `stroke="currentColor"`, stroke-width 1.7 and round caps and joins;
  it is drawn straight on in elevation, not in perspective; its ink stays inside 3–29 on both axes
  and its machines sit on a shared ground line near y=28. No glyph carries a colour, a gradient, an
  external reference or baked text. The rank on a tiered machine is a UI overlay, not part of the
  drawing.
- **Coverage is the whole interface, not the catalogue alone.** Twenty-six base drawings cover all
  twenty-eight buildable machines, seven cover the recipe categories, and twelve cover the branches —
  the nine technology branches in `src/data/technologies.json` plus the three skill branches
  `skills.rs` validates. The same library paints the build cards, the hotbar, the recipe rows, the
  research detail and the skills panel.
- **A tier is the same machine.** `emblemBaseKey` strips the roman suffix, so `pole-iii` resolves to
  the `pole` drawing and differs from it only by a rank badge. The first tier carries no badge: a
  lone "I" on every base emblem would be noise.
- **An unknown key is a plainer button, never a broken one.** The fallback is a generic plate plus
  the definition's own short text — exactly what the whole catalogue used to look like — so a machine
  added without a drawing degrades to the old behaviour rather than to a blank square. The admin
  diagnostics page now raises a warning naming any buildable machine in that state, because a machine
  the player has to read instead of recognise is worth saying out loud.
- **Colour belongs to the caller.** The same drawing takes a category accent in a recipe row and a
  branch accent in the research pane, because it is stroked in `currentColor` and nothing else.
  `researchBranchColor` gained accents for `carrying`, `construction` and `surveying`, and lost a
  dead entry for a `field-capabilities` branch that no longer exists in the data.
- **The build catalogue gained a search.** Typing a name looks past progressive disclosure: a search
  is an explicit request for that machine, and answering it with silence because the machine is still
  locked would be answering a question nobody asked. The reach toggle steps out of the way while a
  query is present, since it would be a control with no effect, and an empty result names the query
  it failed rather than showing an empty panel.
- **The tool shelf says when it has been cut off.** The scrollbar was hidden with nothing in its
  place, so on a narrow layout the last slots simply were not there. The edge with content behind it
  now fades and grows a nudge button, both driven by `overflow-start` / `overflow-end` set from the
  real scroll position. When the shelf fits, none of it is visible.
- **The guide names a step the player's hands can take.** It used to name the primitive furnace to a
  player holding nothing, which is a step the rules refuse; it now names the material the furnace is
  built from and reaches the furnace once that material is in hand.

**Every envelope is unchanged** — save 34, definitions 26, technologies 15, scenarios 7, world 10,
wire 18. There is no migration step in this release and none is needed: every save v0.43.0 loaded
still loads, unmigrated, and a save written by this build loads in v0.43.0 unchanged.

## Verification

The full gate ran green: audit, agent-map check, prettier, cargo fmt, eslint, tsc, the vitest suite
at 300 tests across 21 files, the cargo suite, the wasm-pack build and the vite build.

`tests/emblems.test.ts` states the contract as eight tests over every emblem the library can emit,
each named so a failure says which drawing broke it:

- one frame per emblem, and exactly one — a glyph that smuggled in its own `<svg>` would nest and
  escape every other rule;
- no `fill=`, `style=`, `stroke=`, `<text`, `url(` or gradient inside any glyph;
- ink inside the safe area, by walking the pen;
- every buildable machine in `definitions.json` has a drawing, and every recipe category and branch
  the data declares has one;
- a tier resolves to its base drawing and carries a rank, with `emblemRank(0)` and
  `emblemRank(undefined)` empty and `emblemRank(1)` reading `II`;
- an unknown key falls back rather than throwing;
- and every emblem key names something the data already names, so the library invents no vocabulary
  the simulation would then have to learn.

The safe-area walker earned its place immediately: it found three glyphs — `landing-hub`, `smelting`
and `refining` — whose strokes reached y=2 and would have clipped at 16px. All three were redrawn.

Two existing tests moved with the change rather than around it. `tests/guidance.test.ts` was banking
two hardcoded items for every `gather:` step, so a step could never satisfy its own prerequisite; it
now looks up the item the step actually named, which also asserts that a gather step names a real,
uncrafted item. And the patch-in-place test in `tests/host.test.ts` reads a fixed window after
`renderBuildPanel`, which the new search code had pushed `syncChildren(` out of; the scope logic was
extracted into `renderBuildScope` rather than widening the window, because the window is the point.

Browser session, Windows Chromium, dev server, live DOM:

- all 28 buildable machines drew a real emblem, with zero generic fallbacks;
- the four tiered machines drew their base emblem with a badge — `extractor-ii`, `container-ii` and
  `pole-ii` reading `II`, `pole-iii` reading `III` — and locked cards kept the dashed border;
- searching `smelt` left two cards under `processing` with the reach toggle hidden, `zzzznothing`
  produced `Nothing in the catalogue matches "zzzznothing".`, and clearing restored all 15 cards;
- 23 recipe rows painted with their category accents;
- the research detail drew the `logistics` emblem at 18px in `#88bfff` beside "Logistics ·
  Foundations", and the skills panel drew all three branch emblems in their own accents;
- no console errors at any point.

The shelf affordance was exercised at a 900×820 viewport, where the shelf genuinely overflows
(scrollWidth 857, clientWidth 445). The forward nudge moved the shelf 0 → 160 and the dock gained
`overflow-start` while keeping `overflow-end`; four presses reached the 412 maximum, where
`overflow-end` and the forward button drop away; five presses back returned it to 0, where
`overflow-start` and the back button drop away. The fades follow the same two classes.

## Measurement and limits

**No performance claim is made for this release and no benchmark harness was run for it.** The
committed capacity ladder and the v0.41.0 paving harness are unchanged and remain the only measured
numbers that stand. This release adds inline SVG markup to cards that already existed and paints it
only when a panel renders; that is a reason not to expect a regression, not a measurement of one.

- **The safe-area walk visits landing points, not control points.** Curve and arc handles are not
  checked, so a bulge between two legal endpoints would pass. Every glyph in the library today is
  straight lines and small radii, and the endpoints are what pin a drawing to its frame.
- **Accessibility is structural, and was read from the DOM rather than through assistive
  technology.** Every emblem `<svg>` is `aria-hidden="true"` and sits beside real text — the card
  name, the hotbar `<small>` and its explicit label, the recipe row, the research context line, the
  skill branch label. No accessible name was added or removed by this release, and no emblem is the
  only carrier of any meaning. It has not been exercised in a screen-reader session.
- **The nudge glides, and stops gliding when asked to.** The smooth scroll comes from
  `scroll-behavior: smooth` on `.tool-shelf`, which the global `prefers-reduced-motion` block already
  overrides to `auto !important`. That override was read in the stylesheet, not exercised under a
  live reduced-motion setting; the instant behaviour it produces was confirmed directly.
- **Rank badges run II to V.** A sixth tier would need a numeral added to `emblemRank`. Nothing in
  the catalogue goes past III today.
- **There is no emblem contact sheet, deliberately.** `vite.config.ts` takes only `index.html` and
  `admin.html` as build inputs, and a third shipped HTML entry for a diagnostic is a poor trade when
  the family rules — one frame, no baked colour, no baked text, ink inside the safe area — are
  checked programmatically over every emblem on every run. Reviewing the set by eye means reading the
  test output or opening the game.
- **Sizes were checked at the sizes the game uses.** The emblems were seen at the catalogue, hotbar,
  recipe-row and 18px panel sizes. They are authored to survive 16px and are not claimed beyond that.
