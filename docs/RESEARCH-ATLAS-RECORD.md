# Research Atlas — v0.30.0

The user requested a central icon map, then clarified the reference: several independent trees
with their own starting points, downward branches, lock markers and text shown on inspection.
This pulls research presentation and independent entry points forward; it does not complete the
remaining Phase 1 economy or construction work.

## Player experience

- Research opens a large central modal. Four trees begin with Field Logistics, Storage Planning,
  Automated Extraction and On-site Power. Icons connect downward; shared knowledge between
  production and electricity remains visible as cross-branch prerequisites.
- All 19 technologies have distinct original SVG emblems. The map shows no node labels, prices
  or prose until hover or keyboard focus. Lock/completion markers remain visible. Clicking or
  tapping opens the full details pane; only its explicit Research button spends insight.
- Hover previews include effects, costs and prerequisite names. The detail pane also provides
  prerequisite/next-node jumps, current and post-purchase balances, shortfalls and a hub-income
  link. Back to map restores the open workspace.
- Search includes names, descriptions, disciplines, unlocked machines and player effects.
  Discipline and In reach filters retain map context; Clear filters recovers from an empty result.
- Background drag and normal scrolling pan. Zoom, Fit all and reset/center are buttons. Arrow
  keys navigate icons. An optional list exposes readable labels using the same keyed controls.
  Phones retain the map and offer the list; tapping opens details without requiring hover.
- The factory keeps running. Held player actions stop, world shortcuts yield to the modal, and
  closing returns focus to the opener or the visible research toggle if a cross-link is hidden.

## State and compatibility

`ResearchTree` owns presentation only. Authored icon landmarks never determine unlock order;
`layoutResearch` draws every actual prerequisite and validates the DAG. Positions remain fixed
when research or insight changes. Unknown future technology keys get a fallback emblem/location.
Ordinary native snapshots do not rebuild cards or reset the view. Native `research_availability`
controls purchases; the host submits the existing bounded research command.

Technology catalog 10 removes the Field Logistics prerequisite from Automated Extraction, Storage
Planning and On-site Power. Extraction and power are classified as foundation entry points.
No prices, effects, recipes, construction bills or request rewards change. All 19 technologies
still total 153 insight. Existing knowledge is neither refunded nor revoked.

Save 22 adds the adjacent save-21 migration, updating technology 9 to 10 without changing saved
state. Existing factories, insight, inventories and manual jobs retain their checksums. The picker
exposes the exact released migration chain from save 14 onward. Other envelopes remain definitions
18 / scenarios 5 / world 8 / wire 13; the transport schema is unchanged.

## Balance evidence

The regenerated native balance fixture removes the now-optional belt research from five measured
openings. First smelter insight falls 18→15, first power 7→4, first extractor 12→9, first composer
20→17 and first circuit 26→23. Their modeled gather totals are respectively 43, 36, 46, 58 and 82
(previously 47, 36, 52, 58 and 88). The foundry contract opening moves 18→15 insight and 112→108
modeled gathers. All non-opening machine, item, power, construction and request figures remain
unchanged. These harness estimates exclude walking and are not timed player sessions.

## Verification

The complete local quality gate passes 249 TypeScript tests and 182 Rust tests, dependency audit,
formatting, lint, type checking, map freshness and the production build. The existing Vite
bundle-size advisory remains.

Automated coverage includes all 19 unique icons, deterministic nonoverlapping positions, every
prerequisite edge, no connector crossing an unrelated icon, ancestor paths, arrow navigation,
effect search, malformed graph rejection, modal dismissal and focus restoration. Native tests
verify each of the four roots can be bought independently, old save state/checksums survive,
locked forged commands fail, and purchases are atomic with no second charge.

Browser checks use dedicated validation saves: central modal, icon-only display, four initial
branches, hover costs/effects, shared prerequisites, search/empty recovery, discipline and In reach
filters, zoom/fit, keyboard navigation, Escape/focus return and responsive map/list/details.

This is functional and visual verification, not a user study or a performance measurement. No
new frame-rate, opening-time, physical-phone or integrated-GPU claim is made. Larger discipline
families, evidence projects, target tracking, player skills and broader icon assets remain planned.
