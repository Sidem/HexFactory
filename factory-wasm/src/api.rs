//! api — extracted from lib.rs by scripts/rust-split.mjs.
//! Methods moved verbatim; add the imports the compiler asks for.

use super::*;

#[wasm_bindgen]
impl Factory {
    #[wasm_bindgen(constructor)]
    pub fn new(
        definitions_json: &str,
        technologies_json: &str,
        scenarios_json: &str,
        scenario_key: &str,
        seed_override: Option<u32>,
        world_params_json: Option<String>,
        creative: Option<bool>,
    ) -> Result<Factory, JsValue> {
        let definitions: DefinitionsInput = parse_json(definitions_json)?;
        let technologies: TechnologiesInput = parse_json(technologies_json)?;
        let scenarios: ScenariosInput = parse_json(scenarios_json)?;
        validate_all(&definitions, &technologies, &scenarios).map_err(js_error)?;
        let scenario = scenarios
            .scenarios
            .iter()
            .find(|scenario| scenario.key == scenario_key)
            .ok_or_else(|| js_error(format!("unknown scenario {scenario_key}")))?;
        let world_params = parse_world_params(world_params_json.as_deref())?;
        let mut core = Core::new(
            &definitions,
            &technologies,
            scenario,
            seed_override,
            world_params,
        )
        .map_err(js_error)?;
        // Set after construction rather than threaded through Core::new: creative is a switch the
        // run can throw at any time, so the opening state is the same thing as throwing it on tick
        // zero and there is one implementation of what creative does rather than two.
        core.set_creative(creative.unwrap_or(false));
        Ok(Factory {
            definitions,
            technologies,
            scenarios,
            core,
            snapshot_revision: 0,
            baseline: None,
        })
    }

    pub fn tick(&mut self, count: u32) {
        self.core.tick_many(count.min(240));
    }

    pub fn reset(&mut self) -> Result<(), JsValue> {
        // Reset restarts the run, not the mode: a creative sandbox that came back priced would be
        // the one button a creative player cannot press.
        let creative = self.core.creative;
        self.core = Core::new(
            &self.definitions,
            &self.technologies,
            &self.core.scenario,
            Some(self.core.seed),
            Some(self.core.world_params.clone()),
        )
        .map_err(js_error)?;
        self.core.set_creative(creative);
        // The core the baseline described is gone, so the next delta is a complete replacement.
        self.baseline = None;
        Ok(())
    }

    pub fn new_game(
        &mut self,
        scenario_key: &str,
        seed_override: Option<u32>,
        world_params_json: Option<String>,
        creative: Option<bool>,
    ) -> Result<(), JsValue> {
        let scenario = self
            .scenarios
            .scenarios
            .iter()
            .find(|scenario| scenario.key == scenario_key)
            .ok_or_else(|| js_error(format!("unknown scenario {scenario_key}")))?;
        let world_params = parse_world_params(world_params_json.as_deref())?;
        self.core = Core::new(
            &self.definitions,
            &self.technologies,
            scenario,
            seed_override,
            world_params,
        )
        .map_err(js_error)?;
        self.core.set_creative(creative.unwrap_or(false));
        self.baseline = None;
        Ok(())
    }

    /// The parameters this world was generated from. Not part of the per-frame delta: it changes
    /// only when a world does, so the host asks for it after `new_game` and `load` rather than
    /// paying for it on every frame that could not have changed it.
    pub fn world_params_json(&self) -> String {
        serde_json::to_string(&self.core.world_params).expect("world params serialize")
    }

    /// The shipped presets, with their full parameter sets. The new-world flow is built from this
    /// the same way the catalogue is built from the definitions: the host renders a table native
    /// owns rather than keeping a copy of its own that can drift.
    pub fn world_presets_json() -> String {
        serde_json::to_string(&world_presets()).expect("world presets serialize")
    }

    /// A rectangle of generated terrain for a parameter set nobody has played yet: one byte per
    /// preview pixel, holding the band's index in the `Terrain` declaration order that
    /// `fixtures/terrain-passability.json` already pins on both sides of the wire.
    ///
    /// This is what lets the new-world panel show a world rather than describe one. It goes through
    /// the same `terrain_at` a played hex goes through, so a preview and the world the start button
    /// generates cannot disagree — which is the whole reason it is a native export and not a second
    /// generator written in the host.
    ///
    /// `hexes_across` is the span the width frames. A pixel is square in world units, so a taller
    /// preview shows more world rather than a stretched copy of the same world.
    ///
    /// Takes `&self` for the definitions alone: the parameter set is validated against the same
    /// catalogue `Core::new` validates it against, so the panel cannot draw a world the start
    /// button would then refuse. Nothing about the run in progress is read or moved.
    pub fn world_preview_bytes(
        &self,
        world_params_json: &str,
        seed: u32,
        width: u32,
        height: u32,
        hexes_across: u32,
    ) -> Result<Vec<u8>, JsValue> {
        self.preview_cells(world_params_json, seed, width, height, hexes_across)
            .map_err(js_error)
    }

    /// Where the deposit lattice puts a site inside that same window, in preview pixels.
    ///
    /// Sites are reported as centres rather than sampled per pixel because a patch is smaller than
    /// a pixel at any zoom wide enough to frame a landform — and because a centre is the thing
    /// `site_cell` and `site_jitter` actually move, so it is the thing worth drawing.
    ///
    /// `unmet` carries the guarantees the bootstrap pass gave up on. `Core::new` refuses a world
    /// over exactly that list, so a preview that stayed quiet about it would be a picture of a
    /// world the start button then declines to generate.
    ///
    /// A window wide enough to frame a coastline holds tens of thousands of deposits, which is a
    /// texture rather than a map and a megabyte rather than a payload. Past `MAX_PREVIEW_SITES` the
    /// list is dropped and `total` and `dense` travel alone — `unmet` either way.
    pub fn world_preview_sites_json(
        &self,
        world_params_json: &str,
        seed: u32,
        width: u32,
        height: u32,
        hexes_across: u32,
    ) -> Result<String, JsValue> {
        let sites = self
            .preview_sites(world_params_json, seed, width, height, hexes_across)
            .map_err(js_error)?;
        serde_json::to_string(&sites).map_err(|error| js_error(error.to_string()))
    }

    pub fn apply_commands_json(&mut self, commands_json: &str) -> Result<(), JsValue> {
        self.core.apply_commands(commands_json).map_err(js_error)
    }

    /// One frame: the bounded command batch, `count` simulation ticks, and `player_steps` steps of
    /// player movement. The two counts are separate because the player runs on its own cadence —
    /// see `PLAYER_TICKS_PER_SECOND`, which the host reads to decide how many steps a frame is
    /// worth rather than inventing a rate of its own.
    pub fn advance_json(
        &mut self,
        commands_json: &str,
        count: u32,
        player_steps: u32,
    ) -> Result<(), JsValue> {
        self.core
            .advance(commands_json, count, player_steps)
            .map_err(js_error)
    }

    /// The player's fixed walking cadence in steps per real second. Native owns the rate; the host
    /// only converts elapsed real time into a step count with it.
    #[wasm_bindgen(js_name = playerTicksPerSecond)]
    pub fn player_ticks_per_second() -> u32 {
        PLAYER_TICKS_PER_SECOND
    }

    pub fn placement_preview_json(
        &self,
        q: i32,
        r: i32,
        definition_id: DefinitionId,
        orientation: u8,
        recipe_id: Option<RecipeId>,
    ) -> String {
        let result =
            self.core
                .placement_legality(q, r, definition_id, orientation, recipe_id, true);
        let preview = match result {
            Ok(()) => PlacementPreview {
                legal: true,
                reason: "Ready to build".into(),
            },
            Err(reason) => PlacementPreview {
                legal: false,
                reason,
            },
        };
        serde_json::to_string(&preview).expect("preview is serializable")
    }

    /// The cells, headings, and legality a construction drag between these endpoints would produce.
    pub fn line_preview_json(
        &self,
        q: i32,
        r: i32,
        to_q: i32,
        to_r: i32,
        definition_id: DefinitionId,
        orientation: u8,
        recipe_id: Option<RecipeId>,
    ) -> String {
        let cells =
            self.core
                .line_preview((q, r), (to_q, to_r), definition_id, orientation, recipe_id);
        serde_json::to_string(&cells).expect("preview is serializable")
    }

    /// The cells a removal drag between these endpoints would take back.
    pub fn erase_line_preview_json(&self, q: i32, r: i32, to_q: i32, to_r: i32) -> String {
        let cells = self.core.erase_line_preview((q, r), (to_q, to_r));
        serde_json::to_string(&cells).expect("preview is serializable")
    }

    pub fn boundary_preview_json(&self, edit_json: &str) -> Result<String, JsValue> {
        let edit: BoundaryEdit =
            serde_json::from_str(edit_json).map_err(|e| js_error(e.to_string()))?;
        serde_json::to_string(&self.core.boundary_preview(&edit))
            .map_err(|e| js_error(e.to_string()))
    }

    pub fn ground_preview_json(&self, edit_json: &str) -> Result<String, JsValue> {
        let edit: GroundEdit =
            serde_json::from_str(edit_json).map_err(|e| js_error(e.to_string()))?;
        serde_json::to_string(&self.core.ground_preview(&edit)).map_err(|e| js_error(e.to_string()))
    }

    pub fn snapshot_json(&mut self) -> String {
        let snapshot = self.core.snapshot();
        self.snapshot_revision = 0;
        self.baseline = Some(SnapshotBaseline::from_snapshot(&snapshot));
        self.core.dirty = SnapshotDirty::default();
        serde_json::to_string(&snapshot).expect("snapshot is serializable")
    }

    /// The delta the game actually ships, in the binary wire format.
    ///
    /// wasm-bindgen hands this to the worker as a `Uint8Array`, which the worker then transfers to
    /// the main thread rather than letting the structured clone copy it. `docs/BENCHMARKS.md`
    /// finding 3 is what this exists for: the boundary tracked payload bytes at about 10 µs/KB and
    /// cost more than the simulation it carried.
    pub fn snapshot_delta_bytes(&mut self) -> Vec<u8> {
        let delta = self.build_delta();
        wire::encode_delta(&delta)
    }

    /// The same delta as JSON.
    ///
    /// This is no longer the shipped path. It is retained as the oracle `snapshot_delta_bytes` is
    /// pinned against — the binary buffer must decode to exactly this object — and as the
    /// comparison the capacity ladder reports the encoding's saving against.
    pub fn snapshot_delta_json(&mut self) -> String {
        let delta = self.build_delta();
        serde_json::to_string(&delta).expect("snapshot delta is serializable")
    }

    pub fn save_string(&self) -> Result<String, JsValue> {
        self.core.save_string().map_err(js_error)
    }

    pub fn load_string(&mut self, save: &str) -> Result<(), JsValue> {
        self.core = Core::from_save(&self.definitions, &self.technologies, &self.scenarios, save)
            .map_err(js_error)?;
        self.baseline = None;
        Ok(())
    }

    pub fn checksum(&self) -> u32 {
        self.core.checksum()
    }

    pub fn tick_count(&self) -> u64 {
        self.core.tick
    }
}
