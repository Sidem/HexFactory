//! world — extracted from lib.rs by scripts/rust-split.mjs.
//! Methods moved verbatim; add the imports the compiler asks for.

use super::*;

impl Core {
    pub(crate) fn generate_chunk(&mut self, chunk_q: i32, chunk_r: i32) {
        if !self.generated_chunks.insert((chunk_q, chunk_r)) {
            return;
        }
        self.ground_spine
            .cache_chunk(chunk_q, chunk_r, self.scenario.chunk_size);
        // A departure may be waiting on the far side of the old surveyed frontier. It was stored
        // without asking for this chunk's bed; now the player has opened the chunk, the bed exists
        // in the surveyed cache and the same bounded solver can continue from the first cells the
        // flux entered. Merely querying water still cannot reach this path — only survey does.
        let size = self.scenario.chunk_size;
        let resumed: Vec<(i32, i32)> = self
            .water
            .iter()
            .map(|(&(q, r), _)| (q, r))
            .filter(|&(q, r)| floor_div(q, size) == chunk_q && floor_div(r, size) == chunk_r)
            .collect();
        if !resumed.is_empty() {
            let report = self.settle_water(&resumed);
            if !report.settled {
                self.events.push(format!(
                    "Water front paused at its bound after {} cells",
                    report.cells
                ));
            }
        }
        // New tiles can cover an existing extractor, so every resolved deposit reference is stale —
        // and so is every extractor status derived from one. The two must be invalidated together:
        // dropping the entity marks would make snapshot correctness depend on generated deposits
        // never reaching an existing extractor, which nothing here enforces. Generation is rare, and
        // marks that turn out to change nothing are filtered against the baseline before they ship.
        self.deposit_links.clear();
        self.mark_all_entities_dirty();
        self.dirty.chunks = true;
        // Every cell of a new chunk is a cell the host has never seen a height for, so the whole
        // chunk is the mark. It is not narrowed to "interesting" cells the way it was when the band
        // was the only payload: a plain lowland tile now carries an elevation, a substrate and a
        // water depth that nothing else in the frame can supply.
        self.dirty.terrain.push((chunk_q, chunk_r));
        for local_r in 0..size {
            for local_q in 0..size {
                let q = chunk_q * size + local_q;
                let r = chunk_r * size + local_r;
                // Resources still narrow to a cell that actually appears in the group. Generation
                // is the only path that adds one, and resending them whole keeps the host's order
                // exactly the native one, so later patches can address field cells in place.
                self.dirty.resources_replace |= self.field_at(q, r).is_some();
            }
        }
    }

    /// Every entity snapshot is now suspect. Used by the rare paths that can change what a snapshot
    /// derives from state outside the entity itself: the compiled graph behind `next_id`, and the
    /// deposits behind an extractor's status.
    pub(crate) fn mark_all_entities_dirty(&mut self) {
        for index in 0..self.entities.len() {
            let id = self.entities[index].id;
            self.dirty.entities.push(id);
        }
    }

    pub(crate) fn terrain_at(&self, q: i32, r: i32) -> Terrain {
        self.generated_ground_at(q, r).presentation
    }

    /// What is naturally on this hex, if anything is reachable.
    ///
    /// A paved hex reports nothing. Covering is *suppression*, not harvesting: the tile overlay
    /// keeps whatever quantity was left, the surface hides it from hands, extractors, the snapshot
    /// and regrowth alike, and stripping the surface hands back exactly the deposit that was sealed.
    pub(crate) fn field_at(&self, q: i32, r: i32) -> Option<ResourceState> {
        if self.surface_at(q, r) != 0 {
            return None;
        }
        self.buried_field_at(q, r)
    }

    /// The deposit a hex holds regardless of what has been laid over it.
    ///
    /// A sealed deposit is invisible to every consumer, which is the point — but sealing and
    /// unsealing both have to know a deposit is there to decide whether the published field and the
    /// regrowth roster just changed, and lifting a surface has to put back exactly what went under
    /// it. This is the one view that still sees through the paving.
    pub(crate) fn buried_field_at(&self, q: i32, r: i32) -> Option<ResourceState> {
        if let Some(resource) = self
            .tiles
            .get(&(q, r))
            .and_then(|tile| tile.resource.as_ref())
        {
            return Some(ResourceState {
                item_id: resource.item_id,
                quantity: resource.initial_quantity,
                initial_quantity: resource.initial_quantity,
            });
        }
        if let Some(resource) = self.scenario_resources.get(&(q, r)) {
            return Some(resource.clone());
        }
        self.fields.field_at(
            q,
            r,
            self.scenario.generated_environment,
            &self.ground_spine,
        )
    }

    pub(crate) fn ensure_tile(&mut self, q: i32, r: i32) {
        let size = self.scenario.chunk_size;
        self.generate_chunk(floor_div(q, size), floor_div(r, size));
    }

    /// How far a survey opens around the player's own hex, in cells.
    ///
    /// Rings are the unit the skills speak in, but a ring of the chunk lattice is not a distance
    /// from the player. Standing at a chunk's edge left the frontier one cell ahead and fifteen
    /// behind, and because a chunk is an axial parallelogram rather than a disc, the opened world
    /// read as a stepped, lopsided blot instead of a horizon. The radius restates the same
    /// envelope as a distance instead, so the buffer is equal in every direction wherever inside a
    /// chunk the player happens to stand.
    ///
    /// `rings * size + size / 2` is that restatement, and it is deliberately area-preserving: at
    /// one ring it is 12 cells, a disc of 469 cells against the 448 the seven-chunk opening
    /// covered, and it stays within a few per cent at two and three rings as well. The surveying
    /// skill still widens it and nothing else changed hands.
    pub(crate) fn survey_radius(&self) -> i32 {
        let size = self.scenario.chunk_size;
        self.survey_rings() as i32 * size + size / 2
    }

    /// Survey the world around a point: every chunk holding a cell within [`Core::survey_radius`]
    /// of it.
    ///
    /// Chunks stay the unit of generation, so the outermost opened cell still lands on a chunk
    /// boundary. What is uniform is the guarantee — no direction is ever surveyed less far than
    /// the radius — and that guarantee is the part a player reads as an even frontier.
    pub(crate) fn ensure_neighborhood(&mut self, x: i32, y: i32) {
        let size = self.scenario.chunk_size;
        let (q, r) = world_to_axial(x, y);
        let radius = self.survey_radius();
        let center = (floor_div(q, size), floor_div(r, size));
        // A cell within `radius` differs by at most `radius` on each axis, so no chunk further
        // than this many chunks away can hold one. Candidates outside the disc are then dropped.
        let span = radius.div_euclid(size) + 1;
        for dq in -span..=span {
            for dr in -span..=span {
                let (chunk_q, chunk_r) = (center.0 + dq, center.1 + dr);
                if hexes_in_chunk(chunk_q, chunk_r, size)
                    .any(|cell| axial_distance((q, r), cell) <= radius)
                {
                    self.generate_chunk(chunk_q, chunk_r);
                }
            }
        }
    }
}
