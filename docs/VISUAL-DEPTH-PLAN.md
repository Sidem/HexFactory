# Visual Depth v0.25

HexFactory's next presentation milestone replaces the flat world view with a stylized low-poly 3D
diorama. It gives terrain, machines, cargo, and the player physical presence without changing what
the simulation believes the world is. The native world remains an axial plane for this milestone;
visual height is a renderer decision, not saved state.

This brief is the implementation source of truth for v0.25. The enduring art rules remain in
`docs/ART.md`, architecture boundaries in `docs/ARCHITECTURE.md`, and measurements in
`docs/BENCHMARKS.md`.

## Implementation record

Implemented 2026-08-23 through the production cutover. Gates 0–4 are complete: the v0.24 baseline
and comparison captures are preserved; the renderer contract, Three.js camera, exact pointy-top
terrain, generated machine grammar, bounded instances, overlays, logical-plane picking, six orbits,
quality profiles, contact sheet, and interaction parity are in the shipped path. The old hybrid
renderer has no production or development-switch entry point.

Gate 5 is complete on the reference desktop. Low, Medium, and High each completed the six-tier
browser ladder at 1440×900/DPR 1; all tiers stayed below 35% of a 60 Hz frame, draw calls stayed
between 14 and 16, and raw reports live under `docs/benchmarks/`. Real-game checks covered context
restoration, reduced motion, repeated new-game/demo/load transitions, desktop/laptop-size/narrow/
mobile layouts, all six orbits and zoom extremes, native construction drag, inspect, erase, rotate,
upgrade, gather, delivery, Creative Mode, panels, title, new game, and load. The exact tile apothem
and pointy-top overlay start angle are regression-tested after visual checks found and fixed the
triangular terrain gaps and rotated hover hex.

One external validation remains: no qualifying physical Intel Iris Xe / AMD Vega-class-or-weaker
laptop was available. The desktop record is not an integrated-GPU support claim, and native
elevation/underground decisions remain closed until that evidence exists. The final `npm run
quality` gate passed with 134 TypeScript tests, 121 Rust tests, a clean audit, formatting, lint,
typecheck, Wasm build, and production Vite build.

## Decision

Build a **tilted, near-orthographic 3D renderer in Three.js**, with a camera that orbits in 60-degree
steps. Terrain bands become stepped landforms, and the existing declarative machine grammar becomes
procedural low-poly geometry. Repeated geometry is instanced or merged by render bucket; a building
never costs a draw call of its own.

The direction is an industrial field diorama rather than a miniature CAD view: chunky readable
machines, restrained materials, visible mechanical motion, strong terrain silhouettes, and small
emissive accents where state needs attention. `docs/art/world-shape-still.png` is the composition
reference, not a promise of painterly textures or unrestricted visual effects.

## Player outcome

At the ordinary play zoom, a player can:

- read shore, lowland, hills, highland, cliff, shallow water, and deep water from shape as well as
  colour;
- recognise a machine category from its mass and moving anatomy before reading its label;
- follow cargo through a line and see which machines are working, stalled, or unpowered;
- place, drag, rotate, upgrade, inspect, gather, and navigate with the same precision as the current
  renderer;
- rotate the view without losing the six edge directions or the six corner transport headings;
- run the game on integrated graphics through a deliberately simpler quality profile.

The UI remains an operational layer around the factory. Panels must not become a second visual
rewrite inside this milestone, but their open state may not leave too little world visible to judge
or control the 3D scene.

## Scope boundary

### In v0.25

- Three.js `WebGLRenderer` on the existing world canvas.
- Orthographic or very weak-perspective camera, fixed tilt, 60-degree orbit steps, pan, zoom, follow,
  and recenter.
- Presentation-only height derived from the existing terrain band.
- Chunked or merged terrain surfaces with vertical skirts where adjacent visible bands differ.
- Procedural machine meshes generated from `ShapePart[]`, `TIER_LADDER`, and `HUB_LADDER`.
- Instanced belts, risers, bridges, resources, cargo, trees, repeated machine parts, and overlay
  primitives.
- Player, selection, hover, placement preview, drag preview, construction grid, reach, progress,
  action work, field depletion, stall, power, fog, and home-bearing parity.
- Ray-based picking that still returns the native `WorldPoint` or `AxialCoordinate` expected by the
  host.
- Reduced-motion support and Low / Medium / High graphics profiles.
- A renderer-aware contact sheet and browser capacity ladder.

### Explicitly not in v0.25

- Native elevation, slopes as movement rules, foundations, stacked buildings, belt lifts, falling,
  or any other gameplay use of height.
- Underground layers, tunnels between strata, terrain editing, or voxel storage.
- A save, definition, technology, scenario, world-generator, or snapshot-wire version change.
- A hand-authored mesh or texture atlas per building definition.
- WebGPU, physics, skeletal animation, real-time global illumination, screen-space reflections,
  depth of field, or a post-processing stack.
- Free camera pitch or arbitrary orbit. Both make occlusion and directional input harder before
  either has proved player value.
- Maintaining two production renderers. The old implementation may survive behind a development
  switch during parity work, then is removed before the milestone ships.

## Invariants carried into 3D

- Rust/Wasm remains the only source of simulation truth. The renderer consumes snapshots and sends
  no position, cargo, progress, or legality back.
- A drag preview renders the cells native resolved. The 3D renderer never walks a line of its own.
- The six adjacency directions and twelve transport headings retain their pinned indices.
- Lowland omitted from `snapshot.terrain` is still surveyed lowland inside native-published chunk
  coverage, never unsurveyed world.
- Fog is presentation over `snapshot.chunks`; the renderer does not reveal or generate geography
  outside them.
- A host list carrying controls is patched in place. The renderer replacement does not reopen DOM
  reconstruction bugs.
- Visual randomness is a stable host hash of published facts and never enters saves or checksums.
- Every proportion still receives both native numbers. 3D arcs and rings do not infer maxima.
- Reduced motion removes nonessential continuous animation without hiding state.

## Renderer architecture

### Stable host contract

Extract a renderer interface before replacing the implementation. It preserves the surface already
used by `src/main.ts` and the benchmark:

- snapshot and state: `setSnapshot`, `setHome`, `setReducedMotion`, `setHover`, `setSelection`,
  `setBuildMode`, `setDragPath`, `setBuildFootprint`, `setBuildReach`, `setGathering`;
- view: `toggleGrid`, `panBy`, `zoomAt`, `recenter`, `renderFrame`, `draw`;
- input: `pick` and `pickWorld`.

Add only `orbitBy(step: -1 | 1)` and graphics-profile access. The host continues to know nothing
about matrices, raycasters, meshes, or scene nodes.

Suggested modules:

| Module                                        | Ownership                                                   |
| --------------------------------------------- | ----------------------------------------------------------- |
| `src/rendering/FactoryRenderer.ts`            | Host-facing interface and shared view types                 |
| `src/rendering/three/ThreeFactoryRenderer.ts` | Lifecycle, snapshot reconciliation, public contract         |
| `src/rendering/three/HexSceneCamera.ts`       | Projection, follow/pan/zoom/orbit, screen/world conversion  |
| `src/rendering/three/terrainMeshes.ts`        | Chunk surfaces, terrain columns/skirts, water, fog boundary |
| `src/rendering/three/machineMeshes.ts`        | `ShapePart` to reusable geometry and instance transforms    |
| `src/rendering/three/worldInstances.ts`       | Resources, vegetation, belts, bridges, cargo, player        |
| `src/rendering/three/overlays.ts`             | Grid, hover, selection, previews, reach, progress, status   |
| `src/rendering/three/materials.ts`            | Small shared material set and palette tokens                |
| `src/rendering/three/quality.ts`              | Profile limits and capability selection                     |

Module names may move when the code shows a better boundary, but lifecycle, terrain, machine
grammar, instances, and overlays must not collapse into one renderer file.

### Coordinate model

- Map native world `x/y` onto scene `x/z`; scene `y` is visual height.
- Use `@hexlife/embed/hex` for axial projection and picking conversions exactly as today.
- `visualHeight(terrain)` is one total presentation lookup. Deep water is lowest, then shallow
  water, shore/lowland, hills, highland, and cliff treatment. It never enters a command.
- The ground ray used by `pickWorld` intersects the logical axial plane, not the first machine mesh
  under the pointer. Tall geometry therefore cannot change which hex a click names.
- Orbit is an integer in `[0, 5]`. Camera and directional UI derive from it; native orientation does
  not rotate with the view.

### Scene data and batching

- Snapshot array identity remains the first cache key. Camera motion updates camera matrices, not
  terrain or entity buffers.
- Terrain is rebuilt only when chunks or terrain change. Prefer one merged geometry per visible
  chunk/material bucket over one `Mesh` per hex.
- Every `ShapePart` kind maps to reusable low-poly `BufferGeometry`. Per-part translation, rotation,
  scale, tier growth, colour, and work phase are instance data.
- Static and animated instances use separate buckets so a rotor update does not dirty every vessel.
- Materials are shared. Colour variation is instance colour or a compact attribute, not a cloned
  material.
- Transparent geometry is exceptional. Water and fog must not force all world objects into sorted
  transparency.
- Dispose replaced geometries, materials, textures, and render targets on reset and renderer
  teardown. New game/load loops may not grow `renderer.info.memory` without bound.

### Light and material budget

- One world key light plus hemisphere/ambient fill.
- Low profile: no shadow map; vertex/baked contact shading and simple projected feet.
- Medium profile: one bounded directional shadow around the camera, updated only when its contents
  move enough to matter.
- High profile may increase shadow resolution and water detail, not change game readability.
- Opaque, rough materials dominate. Emissive accents communicate work, power, crystal, or failure;
  they are not decoration on every edge.
- No full-screen post-processing in this milestone.

## Work sequence

### Gate 0 - Preserve and measure the starting point

1. Finish or safely separate the existing harvest work before renderer edits touch overlapping
   files.
2. Run `npm run quality`.
3. Run the full browser ladder on the current renderer, save the raw report under
   `docs/benchmarks/`, and update `docs/BENCHMARKS.md` as the v0.24 baseline.
4. Capture the shipped opening and factory-demo views at 1440x900, 1366x768, and 390x844. These are
   comparison evidence, not art targets.
5. Record current renderer errors, draw time, canvas dimensions, and WebGL context loss behaviour.

**Gate:** the baseline is reproducible from committed sources, and no renderer work begins on an
unknown test state.

### Gate 1 - Three.js shell and camera parity

1. Add the locked `three` dependency and the renderer interface.
2. Create the Three.js renderer on the existing canvas with a capped pixel ratio and explicit
   colour-space configuration.
3. Implement the camera, follow, recenter, pan, zoom-at-pointer, 60-degree orbit, resize, reduced
   motion, and context-loss handling.
4. Render a diagnostic axial plane and player marker.
5. Pin screen-to-world and screen-to-axial round trips at all six orbits and zoom extremes.
6. Keep the old renderer selectable only through a development query flag until parity ships.

**Gate:** movement, aim, picking, pan, zoom, follow, and recenter name the same native world points
as before; changing orbit cannot change a checksum.

### Gate 2 - Terrain, water, fog, and construction space

1. Map the seven terrain bands to a restrained height/material table.
2. Build surveyed lowland from chunk coverage, then layer published non-lowland cells and vertical
   skirts without exposing unsurveyed cells.
3. Add shore edges, shallow/deep water distinction, cliff impassability treatment, deterministic
   surface variation, and depletion scars.
4. Render fog and frontier directly from native chunk coverage.
5. Add construction grid, hover, selection, footprint, legal/illegal preview, drag path, and the
   six/twelve-direction orientation cue.
6. Keep the grid hidden outside editing unless toggled.

**Gate:** every band and both impassable categories are legible with colour desaturated; fog reveals
no unsurveyed world; placement and drag previews agree pixel-for-command with native results.

### Gate 3 - Generated machines and factory flow

1. Give every `ShapePart` a low-poly geometry interpretation and preserve its anchors, scale,
   rotation, count, glow, and phase.
2. Apply the existing tier and hub ladders before producing instance transforms.
3. Build belts, risers, bridges, containers, poles, generators, pumps, extractors, and all composer
   categories from shared geometry/material buckets.
4. Add cargo motion, machine work animation, progress, stall, power, and inventory/readiness cues.
5. Render field materials and vegetation from the existing item glyph/category language; remaining
   wood units still equal visible trees.
6. Render the player, facing, movement, gathering work, and home bearing.
7. Replace the 2D contact sheet with one renderer-driven offscreen contact scene that captures every
   definition, tier, status, and required orbit without creating a WebGL context per card.

**Gate:** every machine category is identifiable at ordinary zoom without its stamp or colour, each
tier differs from its parent in silhouette, and all snapshot-derived state visible in 2D has a 3D
equivalent.

### Gate 4 - Interaction and interface fit

1. Exercise inspect, build, drag, erase, rotate, upgrade, gather, deliver, transfer, creative grant,
   panel, title, new-game, load, and pause flows against the 3D renderer.
2. Resolve tall-object occlusion with camera composition, bounded foreground fading, or clearer
   silhouettes; do not make the player fight a free camera.
3. Ensure open rails leave a usable world viewport at desktop and laptop widths. Preserve the
   one-panel mobile rule.
4. Add compact orbit controls and keyboard bindings using familiar rotate icons and tooltips.
5. Verify touch picking and camera gestures do not conflict with the held movement pad or placement
   drag.

**Gate:** every existing command remains reachable and precise at all six orbits; labels and panels
do not occlude required controls; no list rebuild reintroduces lost clicks.

### Gate 5 - Performance, accessibility, and cutover

1. Extend the browser report with renderer name, graphics profile, draw calls, triangles,
   geometries, textures, and GPU-independent CPU preparation time.
2. Run the complete capacity ladder at Low, Medium, and High. Instance counts may scale with the
   tier; draw calls must remain bounded by visual buckets rather than entity count.
3. Test physical integrated-GPU hardware. The release claim requires at least one Intel Iris Xe /
   AMD Vega-class or weaker laptop at 1366x768 or 1920x1080, DPR 1.
4. Low profile must hold 60 Hz at the 3,072-entity tier and 30 Hz at 6,144 entities on that laptop.
   The reference desktop must keep every tier below 35% of a 60 Hz frame at 1440x900, DPR 1.
5. Verify 95th-percentile interactive frames while walking, orbiting, zooming, opening panels, and
   dragging construction. Averages alone cannot hide shader compilation or buffer-upload stalls.
6. Precompile the bounded material set, cap resolution by profile, and remove allocations from the
   steady render loop.
7. Verify reduced motion, colour-desaturated terrain/machine readability, context loss, new game,
   repeated load, and tab background/foreground recovery.
8. Run desktop and mobile browser screenshots plus canvas-pixel checks that prove the scene is
   nonblank, framed, depth-tested, and free of incoherent overlap.
9. Run `npm run quality`, then remove the old production renderer and development switch.
10. Update `docs/ART.md`, `docs/ARCHITECTURE.md`, `docs/BENCHMARKS.md`, the shipped ledger, and the
    live screenshots with what actually passed.

**Gate:** the laptop targets pass, the complete local gate is green, the renderer has functional
parity, and the old renderer is no longer shipped. If the laptop gate fails, ship no 3D claim:
reduce shadow, resolution, terrain detail, or instance complexity and measure again.

## Test plan

### Pure and unit tests

- Terrain-to-height/material lookup is total over the pinned terrain enum.
- Every `PartKind`, phase, tier step, hub step, silhouette key, and building definition produces
  valid finite geometry and transforms.
- Axial/world/scene/screen round trips stay within the existing picking tolerance at six orbits.
- Instance reconciliation handles insertion, mutation, removal, reset, new game, and load without
  retaining stale IDs.
- Reduced motion produces fixed transforms for every animated part.
- Fog and terrain builders use native chunk bounds and never synthesize coverage outside them.
- Renderer imports no worker, native core, benchmark-only code, or save path.

### Browser scenarios

- New-game opening with sparse terrain and fog.
- Factory demo with every building category, moving cargo, power, and stalls.
- Creative layout at 192, 768, 3,072, and 6,144 entities.
- All six camera orbits at minimum, ordinary, and maximum zoom.
- Construction preview across edge and corner headings, bridge water placement, and multi-cell
  footprints.
- Desktop 1440x900, laptop 1366x768, narrow 720px breakpoint, and touch 390x844.
- Reduced motion, Low profile, context loss/recovery, repeated new-game/load cycles.

### Proof that presentation stayed presentation

- The same scripted command run produces the same native checksum before and after the renderer
  replacement.
- `SAVE_VERSION`, definitions, technologies, scenarios, `WORLD_GENERATOR_VERSION`, and
  `WIRE_VERSION` do not move.
- No new per-frame command is sent for camera, orbit, animation, visual height, or quality.

## Cut lines

Cut in this order if the milestone grows beyond its purpose:

1. High-profile shadow and water improvements.
2. Decorative particles and secondary machine motion.
3. Foreground fading beyond the minimum needed for picking.
4. Organic terrain detail beyond the seven readable bands.

Do not cut picking parity, terrain/category legibility, generated machine identity, Low-profile
performance, reduced motion, or the benchmark and contact-sheet gates. Those are the milestone.

## Commit sequence

The implementation should remain reviewable in this order:

1. `chore: record the v0.24 renderer baseline`
2. `refactor: define the replaceable factory renderer contract`
3. `feat: add the Three.js scene and hex camera`
4. `feat: give the surveyed world visual depth`
5. `feat: generate the 3D factory from the shape grammar`
6. `feat: restore spatial overlays and interaction parity`
7. `perf: gate visual depth on the browser capacity ladder`
8. `feat: ship Visual Depth v0.25`

Each commit must build and keep the current native test suite green. Do not mix a save/wire change
or gameplay elevation into this sequence.

## Decision after v0.25

Only after the renderer ships and physical laptop evidence exists should height become simulation
state. The next design review chooses among:

- keep height presentation-only and proceed to Living Lattice;
- add native integer surface elevation, slope/foundation rules, and vertical transport as a new
  gameplay milestone;
- prototype underground as separate sparse axial strata, with the surface and each underground
  level retaining the current two-coordinate rules and explicit shaft/elevator graph edges joining
  them.

A free-form voxel world is not on this path. Sparse strata preserve the deterministic chunk,
occupancy, transport, save, and measurement architecture that already works.
