# browser route (generated)

Read the named file and a bounded range around the anchor; do not read oversized files end to end.

## Tasks

- **Binary snapshots:** `factory-wasm/src/wire.rs; src/core/snapshotWire.ts` — `encode_snapshot_delta, decodeSnapshotDelta`
- **Worker and host boundary:** `src/core/factory.worker.ts; src/core/FactoryHost.ts` — `handle, applyDelta`
- **Frame loop and application wiring:** `src/main.ts` — `frame, update`
- **Research tree, skills and icons:** `src/ui/researchTree.ts; src/ui/researchGraph.ts; src/ui/skills.ts; src/rendering/researchIcons.ts` — `ResearchTree, SkillsView, layoutResearch`
- **Panels and keyed DOM:** `src/ui/panels.ts; src/ui/dom.ts` — `PanelController, syncChildren`
- **Input commands:** `src/core/input.ts; src/core/commands.ts; src/main.ts` — `BoundedInputQueue, enqueue`
- **Contracts, requests and guidance:** `src/data/scenarios.json; src/core/guidance.ts; factory-wasm/src/lib.rs` — `ContractDefinition, advance_contract, nextAction`
- **Title screen and save catalogue:** `src/core/saveSlots.ts; src/main.ts` — `SaveSlot, openTitleScreen, compatibility`

## Files

- `src/app/bootstrap.ts` — 399 lines / 14.1 KiB
- `src/app/buildController.ts` — 692 lines / 24.3 KiB
- `src/app/buildInfo.ts` — 25 lines / 0.7 KiB — currentBuild:6
- `src/app/buildWiring.ts` — 266 lines / 10.1 KiB
- `src/app/constructionInput.ts` — 414 lines / 13.2 KiB — eraseLine:96, deleteBuildingUnderCursorOrSelected:164, refreshDragPreview:206, rotateUnderCursorOrPending:259
- `src/app/coreView.ts` — 608 lines / 21.1 KiB
- `src/app/createApp.ts` — 24 lines / 0.7 KiB
- `src/app/inputWiring.ts` — 454 lines / 17.3 KiB
- `src/app/inspectorControls.ts` — 598 lines / 22.8 KiB
- `src/app/inspectorOverview.ts` — 669 lines / 25.5 KiB
- `src/app/lifecycle.ts` — 334 lines / 10.8 KiB
- `src/app/lifecycleWiring.ts` — 179 lines / 7.1 KiB
- `src/app/preferences.ts` — 117 lines / 3.8 KiB — PreferencesController:15
- `src/app/runtime.ts` — 390 lines / 14.4 KiB — Tool:32, BuildGroupKey:42, StockCompartment:49, StackDrag:74, …
- `src/app/saveUi.ts` — 151 lines / 5.0 KiB — SaveUi:21
- `src/app/workspaceController.ts` — 319 lines / 9.7 KiB
- `src/app/workspaceWiring.ts` — 529 lines / 19.2 KiB
- `src/app/worldSetup.ts` — 449 lines / 16.9 KiB — WorldSetup:29, exactSeed:443
- `src/audio/feedback.ts` — 220 lines / 6.9 KiB — FeedbackCue:17, FeedbackAudio:96, cueForEvent:194
- `src/contact/main.ts` — 226 lines / 7.3 KiB — paint:57, repaint:70, element:74, sharedWith:90, …
- `src/core/FactoryHost.ts` — 386 lines / 11.3 KiB — FactoryWorkerMethod:26, WorldChoice:45, FactoryTransport:47, WorkerTransport:67, …
- `src/core/availability.ts` — 135 lines / 4.8 KiB — CostLine:17, BuildAvailability:25, heldQuantity:34, costLines:42, …
- `src/core/checkpoints.ts` — 347 lines / 12.7 KiB — CheckpointContext:19, CheckpointBuilding:29, CheckpointDefinition:42, CheckpointRecord:52, …
- `src/core/commands.ts` — 327 lines / 10.7 KiB — EncodedCommand:3, MAX_AIM_COORDINATE:13, halfTransfer:23, encodeCommand:35
- `src/core/definitions.ts` — 876 lines / 30.8 KiB — supportsRecipe:14, reservationCells:47, footprintIsContiguous:65, MAX_UNDERPASS_SPAN:109, …
- `src/core/directions.ts` — 32 lines / 1.4 KiB — TRANSPORT_DIRECTIONS:4, CORNER_START:7, DIRECTION_NAMES:10, rotateAnyOrientation:22
- `src/core/factory.worker.ts` — 221 lines / 7.9 KiB — handle:54, requireFactory:184, delta:197, optionalNumber:208, …
- `src/core/fileExport.ts` — 64 lines / 1.8 KiB — downloadTextFile:16
- `src/core/frameClock.ts` — 47 lines / 1.4 KiB — FrameClockState:1, SIMULATION_TICKS_PER_SECOND:7, FrameAdvance:9, FrameClock:19
- `src/core/guidance.ts` — 593 lines / 23.8 KiB — Guidance:34, nextAction:53, stillToFind:283, expand:298, …
- `src/core/input.ts` — 90 lines / 2.8 KiB — MAX_INPUT_COMMANDS:3, MOVEMENT_KEYS:4, WALK_SCALE:23, movementIntent:25, …
- `src/core/lattice.ts` — 204 lines / 6.9 KiB — HEX_RADIUS:19, CORNERS:27, CORNER_NAMES:37, DIRECTIONS:47, …
- `src/core/recipes.ts` — 48 lines / 1.5 KiB — recipeOutputs:4, recipeYield:8, recipeShare:15, productionRoutes:23, …
- `src/core/saveSlots.ts` — 802 lines / 25.7 KiB — SAVE_VERSION:46, SAVE_CATALOG_KEY:47, LEGACY_SAVE_PREFIX:48, HXF1_PREFIX:49, …
- `src/core/skills.ts` — 114 lines / 4.3 KiB — validateSkills:4
- `src/core/snapshotDelta.ts` — 114 lines / 4.0 KiB — applySnapshotDelta:12, applyBuildingsPatch:46, applyResourcesPatch:83, applyTerrainPatch:101, …
- `src/core/snapshotWire.ts` — 791 lines / 24.9 KiB — Reader:150, decodeSnapshotDelta:227, readPlayer:452, readChunks:515, …
- `src/core/terrain.ts` — 161 lines / 4.9 KiB — TerrainInfo:13, TERRAIN_INFO:23, TERRAIN_ORDER:89, terrainAccess:100, …
- `src/core/types.ts` — 1292 lines / 44.9 KiB — BuildingKind:3, Terrain:15, Substrate:28, PlacementRule:29, …
- `src/data/definitions.json` — 2906 lines / 67.4 KiB
- `src/data/scenarios.json` — 168 lines / 4.8 KiB
- `src/data/technologies.json` — 561 lines / 16.9 KiB
- `src/input/focus.ts` — 35 lines / 1.1 KiB — isTypingTarget:2, isKeyboardFocusedControl:13, isPointerActivatedControl:23
- `src/main.ts` — 6 lines / 0.1 KiB
- `src/ui/boundaries.ts` — 673 lines / 24.3 KiB — nearestBoundaryDirection:103, edgeAnchors:113, BoundaryTool:131
- `src/ui/confirm.ts` — 131 lines / 4.8 KiB — ConfirmRow:12, ConfirmRequest:19, ConfirmDialog:31
- `src/ui/dom.ts` — 50 lines / 1.5 KiB — required:3, part:9, syncChildren:21
- `src/ui/ground.ts` — 938 lines / 36.4 KiB — GroundTool:177
- `src/ui/paint.ts` — 37 lines / 0.9 KiB — paintHexFace:3, setMeter:14, setItemGlyph:29
- `src/ui/panels.ts` — 188 lines / 6.0 KiB — PanelController:9
- `src/ui/production.ts` — 32 lines / 1.7 KiB — productionNote:5
- `src/ui/research.ts` — 36 lines / 1.2 KiB — orderTechnologies:4, technologyContext:26
- `src/ui/researchGraph.ts` — 230 lines / 6.9 KiB — RESEARCH_NODE_WIDTH:14, RESEARCH_NODE_HEIGHT:15, ResearchNode:42, ResearchEdge:49, …
- `src/ui/researchTree.ts` — 660 lines / 24.0 KiB — ResearchTree:28
- `src/ui/saveList.ts` — 76 lines / 2.9 KiB — paintSaveSlotList:15
- `src/ui/skills.ts` — 249 lines / 10.1 KiB — groupSkills:73, skillView:84, SkillsView:108
- `src/ui/stockSlots.ts` — 87 lines / 3.5 KiB — MachineStockSlot:12, machineStockSlots:39
- `src/ui/terrainLegend.ts` — 24 lines / 1.0 KiB — renderTerrainLegend:5
- `src/ui/worldParameters.ts` — 586 lines / 21.1 KiB — WorldScalar:19, NOISE_MAX:22, BAND_KEYS:24, BAND_GAP:31, …
- `src/ui/worldPreview.ts` — 556 lines / 19.9 KiB — PREVIEW_WIDTH:29, PREVIEW_HEIGHT:30, PREVIEW_BACKDROP:37, PreviewZoom:39, …
- `src/vite-env.d.ts` — 2 lines / 0.0 KiB
