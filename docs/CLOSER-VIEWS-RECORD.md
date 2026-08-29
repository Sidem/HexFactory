# v0.43.0 — Closer Views and Field Survey

Two camera changes and one player skill, all requested directly. No phase was opened or closed by
this release: phase 6 was completed by v0.42.0, and phase 7, supported floors and vertical
transport, remains next.

## Delivery

- **Twelve 30-degree orbit stops instead of six 60-degree ones.** `ORBIT_STEPS` is 12 and the orbit
  index is an integer in `[0, 11]`. The six hex headings are still stops; the six half-steps between
  them are new, and a half-step is presentation only — it changes where the scene is looked at from
  and nothing about the six or twelve native heading indices.
- **The turn rate is unchanged.** `ORBIT_STEP_MS` went 460 → 230, half the duration for half the
  angle, so the view sweeps at exactly the speed it always did. A held `,` or `.` still crosses the
  full circle in the same time; each individual stop simply arrives twice as often. Queued steps
  still extend the sweep already running rather than restarting it, capped by `ORBIT_MAX_MS` at one
  second, and reduced motion still lands on the same heading with no sweep at all.
- **Zoom reaches 4×.** `MAX_ZOOM` went 2.2 → 4 with `MIN_ZOOM` unchanged at 0.55, which is close
  enough to read one machine and the hexes it stands on. Nothing baked is scaled up by it: this
  route draws meshes, and the flat `CanvasFactoryRenderer` route keeps its own 0.55–2.2 clamp
  because its sprite atlas is baked at `BASE_HEX_SIZE × 2.2`.
- **Field Survey is a third one-point skill.** `SkillEffect::SurveyRange` adds a ring of chunks to
  the neighbourhood the world opens around wherever the player reaches. `ensure_neighborhood` is now
  a hex disc of `survey_rings()` rings on the chunk lattice; at one ring that disc is exactly the
  chunk and its six `DIRECTIONS` neighbours the game always opened, so a save without the skill
  generates the identical world.
- **The range is derived, never stored.** `Core::survey_rings()` reads the purchased set, under the
  same rule every other derived value follows — and for a second reason: the surveyed world is
  `generated_chunks`, which is a checksum input, so a stored radius would be a second, unhashed
  account of the same fact. `BASE_SURVEY_RINGS` is 1 and `MAX_SURVEY_RING_BONUS` is 2, enforced in
  both `validate_skills` implementations, because a survey of `n` rings generates `3n(n+1)+1` chunks
  and the bound is a cost ceiling on world generation rather than a taste.
- **It pays out when it is learned, not on the next step.** `purchase_skill` re-surveys where the
  player is standing; generation is idempotent per chunk, so re-surveying a half-open neighbourhood
  adds only what is missing. Loading deliberately does not re-survey: generating during a load would
  move `generated_chunks` under a file that was just verified without it.
- **The skills panel is three branches wide.** Its copy is a table keyed by effect kind rather than
  a two-way ternary, so each upgrade names its own unit — cargo slots, hexes of reach, rings of
  surveyed ground — with the singular written out beside the plural. The grid is `auto-fit` rather
  than a fixed two columns, and `surveying` has its own accent and emblem.

**Save 33 → 34 and technologies 14 → 15.** Definitions 26, scenarios 7, world 10 and wire 18 are
unchanged; a new `SkillEffect` variant does not move the wire layout. The save step is version
stamps only. An unlearned skill is an id absent from `skills.purchased`, which is what every
version-33 file already says, so the state is left exactly as written.

## Verification

The full gate ran green: audit, agent-map check, prettier, cargo fmt, eslint, tsc, the vitest suite,
the cargo suite, the wasm-pack build and the vite build. Focused coverage added with the change:

- `tests/visualDepth.test.ts` — the full circle closes in twelve stops of exactly 30°, one sweep is
  30° over 230ms with the midpoint pinned, a step pressed mid-sweep extends it to 60°, reduced
  motion snaps 30°, and zoom clamps at 4 and 0.55.
- `factory-wasm` — `the_field_survey_opens_two_rings_where_one_was_opened_before` asserts the exact
  chunk counts (7 for one ring, 19 for two), that learning re-surveys where you stand, that a second
  survey of the same ground moves neither the count nor the checksum, and that a reload rebuilds the
  range from the purchased set with the checksum intact.
- `factory-wasm` — `version_thirty_three_offers_the_survey_skill_without_learning_it` asserts the
  stamps move and the purchased set, the unspent point and the pack are untouched.
- The balance fixture moved exactly twice: `technology_version` 14 → 15 and `skill_cost` 2 → 3.
  Three milestones fund three one-point upgrades, so the journey pays for the whole ladder and not a
  point more.

Browser session, Windows Chromium, dev server, High profile: the Skills panel drew all three cards
in one row of three equal columns, with `01 / Carrying · +4 cargo slots · 8 → 12 slots`,
`02 / Construction reach · +3 hexes of reach · 5 → 8 hexes`, and
`03 / Surveying range · +1 ring of surveyed ground · 1 → 2 rings`, each with its own emblem and the
surveying card in its own accent. The orbit controls read `Orbit view left 30° (,)` and
`Orbit view right 30 degrees`.

## Measurement and limits

**No performance claim is made for this release and no benchmark harness was run for it.** The
committed capacity ladder and the v0.41.0 paving harness are unchanged and are the only measured
numbers that stand.

- Zooming to 4× draws the same meshes closer; no extra detail, texture tier or LOD was authored for
  the near end, and the reference draw-call figures were measured at the old ceiling.
- The flat `CanvasFactoryRenderer` route still stops at 2.2. Its sprites are baked for that clamp,
  so the closer zoom is the Three.js route only.
- The contact sheet still renders six orbits. It is a self-contained diagnostic that covers the full
  circle at 60°, and the new half-steps add no machine pose it does not already show.
- Field Survey is one rank. The catalogue is bounded at two extra rings and no second rank is
  authored, so the widest survey a player can reach today is two.
- Turning the camera in the hidden preview pane could not be exercised end to end: the sweep
  advances on `requestAnimationFrame`, which does not run while the pane is hidden. The camera is
  covered by unit tests over the same code the renderer calls, and the panel and control labels were
  read from the live DOM.
