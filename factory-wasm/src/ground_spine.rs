//! Phase 8's production ground spine and the prepared physical source.
//!
//! The seven shipped [`Terrain`] bands currently answer four different questions at once: height,
//! material, water and presentation. This module separates those answers. New worlds select
//! [`GroundSpine::physical`]; a save from before the 25 m² hex is refused rather than mixed.
//! [`GroundSpine::generated_uncached_at`] is the full source oracle; the cache only holds surveyed
//! chunks and must always echo that oracle.
//!
//! Heights in this slice are deliberately generator-native units. The legacy source produces the
//! shipped presentation steps; the physical source will produce 0.25 m quanta when it activates.
//! No caller may run a height through `scale` merely because it has this type.

use super::*;

/// A signed absolute height in the active ground source's native unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct GroundElevation(i32);

impl GroundElevation {
    pub(super) const fn new(value: i32) -> Self {
        Self(value)
    }

    pub(super) const fn get(self) -> i32 {
        self.0
    }

    fn add(self, delta: GroundDelta) -> Self {
        Self(self.0 + i32::from(delta.0))
    }
}

/// A sparse change to generated bed height. Earthworks and erosion remain separate instances.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct GroundDelta(i16);

impl GroundDelta {
    pub(super) const fn new(value: i16) -> Self {
        Self(value)
    }

    pub(super) const fn get(self) -> i16 {
        self.0
    }
}

/// What the bed is made of, independent of whether water stands above it.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Substrate {
    Sand,
    Meadow,
    Soil,
    Rock,
}

/// Initial water above the generated bed. Discharge is zero because ridge-noise water is not a
/// live flow model; the field exists now so the physical source has an independent output to fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct InitialHydrology {
    /// Standing depth in 0.25 m quanta for the physical source. The legacy adapter gives one native
    /// unit to deep water and none to a shallow, whose bed is already plain level; the presentation
    /// band remains the compatibility distinction between a ford and dry ground.
    pub(super) depth_quanta: i32,
    pub(super) surface: GroundElevation,
    pub(super) discharge_class: u8,
}

/// The pure generated facts for one cell. `presentation` is a compatibility output, not a source
/// of finished elevation, substrate or hydrology.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct GeneratedGround {
    pub(super) bed: GroundElevation,
    pub(super) substrate: Substrate,
    pub(super) hydrology: InitialHydrology,
    pub(super) presentation: Terrain,
}

impl GeneratedGround {
    pub(super) fn from_legacy_band(terrain: Terrain) -> Self {
        let bed = GroundElevation::new(natural_elevation(terrain));
        let substrate = match terrain {
            Terrain::DeepWater | Terrain::ShallowWater => Substrate::Soil,
            Terrain::Lowland => Substrate::Meadow,
            Terrain::Shore => Substrate::Sand,
            Terrain::Hills => Substrate::Soil,
            Terrain::Highland | Terrain::Cliff => Substrate::Rock,
        };
        // Only deep water stands above its bed here. A legacy shallow is a ford — `natural_elevation`
        // puts its bed at plain level on purpose, so any depth at all would draw a lake standing
        // proud of its own shore. The band is what tells a ford from a meadow in this source.
        let depth_quanta = i32::from(matches!(terrain, Terrain::DeepWater));
        // These are compatibility surfaces in legacy steps, not physical water levels. Nothing
        // reads them for simulation before the physical source activates.
        let surface = GroundElevation::new(bed.get() + depth_quanta);
        Self {
            bed,
            substrate,
            hydrology: InitialHydrology {
                depth_quanta,
                surface,
                discharge_class: 0,
            },
            presentation: terrain,
        }
    }
}

/// Above this bed height a cell is bare highland however gently it rolls: 4,800 quanta, 1,200 m.
///
/// Chosen so the term names ground the continental field reaches rarely rather than ground it
/// clears by default. It is a material rule and nothing else reads it: walking, building and water
/// all keep answering to the height itself.
const ALPINE_QUANTA: i32 = 4_800;

/// How far apart the two samples of a landform gradient are taken, in cells.
const MATERIAL_STENCIL: i32 = 3;

/// The gradient a cell sits on, in height quanta per cell.
///
/// Deliberately measured across three cells rather than one. A single neighbour step is dominated
/// by the fine relief term — ±0.75 m of grain that exists to break flats so they drain, not to
/// describe a hillside — so a material chosen from it speckles the ground with noise instead of
/// shading it by landform. Over three cells the grain is uncorrelated and averages away while a
/// real slope accumulates, so this reads the hillside rather than the gravel lying on it.
///
/// The neighbour step keeps its own separate job: it is what walking, building and the cliff face
/// are defined against, and those are facts about a step, not about a hillside.
fn landform_gradient(terra: &mut crate::terra::Terra, bed: i32, source: (i32, i32)) -> i32 {
    DIRECTIONS
        .iter()
        .map(|&(dq, dr)| {
            let far = (
                source.0 + dq * MATERIAL_STENCIL,
                source.1 + dr * MATERIAL_STENCIL,
            );
            (bed - terra.head(far.0, far.1)).abs()
        })
        .max()
        .unwrap_or(0)
        / MATERIAL_STENCIL
}

impl GeneratedGround {
    fn from_physical(
        terra: &mut crate::terra::Terra,
        origin: (i32, i32),
        q: i32,
        r: i32,
        generated_environment: bool,
    ) -> Self {
        if !generated_environment {
            return Self::from_legacy_band(Terrain::Lowland);
        }
        let source = (q + origin.0, r + origin.1);
        let bed = terra.head(source.0, source.1);
        let water = terra.water(source.0, source.1);
        let depth_quanta = water.depth();
        let max_step = DIRECTIONS
            .iter()
            .map(|&(dq, dr)| (bed - terra.head(source.0 + dq, source.1 + dr)).abs())
            .max()
            .unwrap_or(0);
        let gradient = landform_gradient(terra, bed, source);
        // This is the first physical material policy, deliberately stated against native facts.
        // Slice 3 may tune the thresholds from the opening survey; it may not turn a height back
        // into the old seven-band authority.
        //
        // The elevation term used to be `bed >= 600` — 150 m — which the continental field clears
        // almost everywhere: the surveyed world averages about 800 m, so that one clause selected
        // Soil for every cell that was not an outright cliff and the whole material map collapsed
        // to a single band. `npm run survey` measured the result at 889 per mille Hills with no
        // Lowland and no Highland at all, which is the monochrome ground, and no amount of relief
        // in the height field could have shown through it. Elevation still earns a say, but it has
        // to name high ground rather than name the world, so it moved to [`ALPINE_QUANTA`] and
        // slope became the term that decides the ordinary case.
        let substrate = if bed <= crate::scale::SEA_LEVEL_QUANTA + 8 {
            Substrate::Sand
        } else if gradient > crate::scale::MAX_WALK_STEP_QUANTA || bed >= ALPINE_QUANTA {
            Substrate::Rock
        } else if gradient > crate::scale::MAX_BUILD_STEP_QUANTA {
            Substrate::Soil
        } else {
            Substrate::Meadow
        };
        let wet_neighbour = DIRECTIONS
            .iter()
            .any(|&(dq, dr)| terra.water(source.0 + dq, source.1 + dr).is_wet());
        let presentation = if depth_quanta >= crate::scale::WADE_LIMIT_QUANTA {
            Terrain::DeepWater
        } else if depth_quanta > 0 {
            Terrain::ShallowWater
        } else if wet_neighbour {
            Terrain::Shore
        } else {
            match substrate {
                Substrate::Sand => Terrain::Shore,
                Substrate::Meadow => Terrain::Lowland,
                Substrate::Soil => Terrain::Hills,
                Substrate::Rock if max_step > crate::scale::MAX_WALK_STEP_QUANTA => Terrain::Cliff,
                Substrate::Rock => Terrain::Highland,
            }
        };
        Self {
            bed: GroundElevation::new(bed),
            substrate,
            hydrology: InitialHydrology {
                depth_quanta,
                surface: GroundElevation::new(bed + depth_quanta),
                discharge_class: water.discharge_class(),
            },
            presentation,
        }
    }
}

/// All facts about one finished cell. The generated bed and the two sparse deltas stay distinct
/// even though callers normally need their sum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FinishedGround {
    pub(super) generated: GeneratedGround,
    pub(super) earthwork: GroundDelta,
    pub(super) erosion: GroundDelta,
    pub(super) surface: DefinitionId,
}

impl FinishedGround {
    pub(super) fn elevation(self) -> GroundElevation {
        self.generated.bed.add(self.earthwork).add(self.erosion)
    }

    pub(super) fn cliff_quarried(self) -> bool {
        self.generated.presentation == Terrain::Cliff && self.earthwork.get() < 0
    }

    pub(super) fn blocks_movement(self) -> bool {
        self.generated.presentation.blocks_movement() && !self.cliff_quarried()
    }

    pub(super) fn blocks_construction(self) -> bool {
        self.generated.presentation.blocks_construction() && !self.cliff_quarried()
    }
}

/// The current band's generated height, in shipped presentation steps.
pub(super) const fn legacy_band_elevation(terrain: Terrain) -> i32 {
    match terrain {
        Terrain::DeepWater => -1,
        Terrain::ShallowWater | Terrain::Shore | Terrain::Lowland => 0,
        Terrain::Hills => 1,
        Terrain::Highland => 2,
        Terrain::Cliff => 3,
    }
}

/// Pure generated ground plus a cache restricted to surveyed chunks.
enum GroundSource {
    Legacy,
    Physical {
        terra: RefCell<crate::terra::Terra>,
        origin: (i32, i32),
    },
}

pub(super) struct GroundSpine {
    params: WorldParams,
    seed: u32,
    generated_environment: bool,
    source: GroundSource,
    cache: RefCell<BTreeMap<(i32, i32), GeneratedGround>>,
}

impl GroundSpine {
    #[allow(dead_code)]
    pub(super) fn legacy(params: &WorldParams, seed: u32, generated_environment: bool) -> Self {
        Self {
            params: params.clone(),
            seed,
            generated_environment,
            source: GroundSource::Legacy,
            cache: RefCell::new(BTreeMap::new()),
        }
    }

    /// New-world ground: drainage-first absolute bed, translated so the opening sits on a dry shelf.
    pub(super) fn physical(params: &WorldParams, seed: u32, generated_environment: bool) -> Self {
        let mut terra = crate::terra::Terra::new(seed);
        let landing = terra.landing_site();
        Self {
            params: params.clone(),
            seed,
            generated_environment,
            source: GroundSource::Physical {
                terra: RefCell::new(terra),
                origin: (landing.q, landing.r),
            },
            cache: RefCell::new(BTreeMap::new()),
        }
    }

    /// Whether the height this spine publishes is a physical 0.25 m quantum rather than a legacy
    /// presentation band step. Nothing in a snapshot says which, and the renderer has to know: the
    /// same integer means seventeen times as much ground once the physical source answers.
    pub(super) fn is_physical(&self) -> bool {
        matches!(self.source, GroundSource::Physical { .. })
    }

    /// Generated drainage destination in world coordinates. Live erosion may read it only after
    /// checking that both ends are surveyed; this pure oracle never opens a gameplay chunk.
    pub(super) fn downstream_at(&self, q: i32, r: i32) -> Option<(i32, i32)> {
        match &self.source {
            GroundSource::Legacy => None,
            GroundSource::Physical { terra, origin } => terra
                .borrow_mut()
                .downstream(q + origin.0, r + origin.1)
                .map(|(down_q, down_r)| (down_q - origin.0, down_r - origin.1)),
        }
    }

    pub(super) fn presentation_at(&self, q: i32, r: i32) -> Terrain {
        self.generated_at(q, r).presentation
    }

    pub(super) fn wet_at(&self, q: i32, r: i32) -> bool {
        let ground = self.generated_at(q, r);
        ground.hydrology.depth_quanta > 0 || ground.presentation.is_water()
    }

    pub(super) fn generated_at(&self, q: i32, r: i32) -> GeneratedGround {
        self.cache
            .borrow()
            .get(&(q, r))
            .copied()
            .unwrap_or_else(|| self.generated_uncached_at(q, r))
    }

    /// Read through the cache only while it still describes the Core's current source. Tests and
    /// migration code can replace a scenario or generator identity directly; that must fall back
    /// to the full oracle rather than returning a plausible stale band.
    pub(super) fn generated_from(
        &self,
        params: &WorldParams,
        seed: u32,
        generated_environment: bool,
        q: i32,
        r: i32,
    ) -> GeneratedGround {
        if self.params == *params
            && self.seed == seed
            && self.generated_environment == generated_environment
        {
            return self.generated_at(q, r);
        }
        GeneratedGround::from_legacy_band(terrain_at(params, seed, q, r, generated_environment))
    }

    /// The full oracle. It never reads or populates the cache.
    pub(super) fn generated_uncached_at(&self, q: i32, r: i32) -> GeneratedGround {
        match &self.source {
            GroundSource::Legacy => GeneratedGround::from_legacy_band(terrain_at(
                &self.params,
                self.seed,
                q,
                r,
                self.generated_environment,
            )),
            GroundSource::Physical { terra, origin } => GeneratedGround::from_physical(
                &mut terra.borrow_mut(),
                *origin,
                q,
                r,
                self.generated_environment,
            ),
        }
    }

    /// Cache exactly one gameplay chunk. Querying an unsurveyed cell remains a pure uncached read
    /// and cannot grow `generated_chunks` or this cache.
    pub(super) fn cache_chunk(&self, chunk_q: i32, chunk_r: i32, size: i32) {
        let rows: Vec<_> = hexes_in_chunk(chunk_q, chunk_r, size)
            .map(|(q, r)| ((q, r), self.generated_uncached_at(q, r)))
            .collect();
        self.cache.borrow_mut().extend(rows);
    }

    /// Rebuild after load from the saved surveyed-chunk set. The cache itself is never saved or
    /// checksummed, and this explicit route makes changing its source impossible to overlook.
    pub(super) fn rebuild_cache(&self, chunks: &BTreeSet<(i32, i32)>, size: i32) {
        self.cache.borrow_mut().clear();
        for &(chunk_q, chunk_r) in chunks {
            self.cache_chunk(chunk_q, chunk_r, size);
        }
    }

    #[cfg(test)]
    fn cached_cells(&self) -> usize {
        self.cache.borrow().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_adapter_separates_facts_without_changing_height() {
        let cases = [
            (Terrain::DeepWater, -1, Substrate::Soil, 1, 0),
            (Terrain::ShallowWater, 0, Substrate::Soil, 0, 0),
            (Terrain::Shore, 0, Substrate::Sand, 0, 0),
            (Terrain::Lowland, 0, Substrate::Meadow, 0, 0),
            (Terrain::Hills, 1, Substrate::Soil, 0, 1),
            (Terrain::Highland, 2, Substrate::Rock, 0, 2),
            (Terrain::Cliff, 3, Substrate::Rock, 0, 3),
        ];
        for (terrain, elevation, substrate, depth_quanta, water_surface) in cases {
            let ground = GeneratedGround::from_legacy_band(terrain);
            assert_eq!(ground.bed.get(), elevation);
            assert_eq!(ground.substrate, substrate);
            assert_eq!(ground.hydrology.depth_quanta, depth_quanta);
            assert_eq!(ground.hydrology.surface.get(), water_surface);
            assert_eq!(ground.hydrology.discharge_class, 0);
            assert_eq!(ground.presentation, terrain);
        }

        // Finished ground carries the same facts with two independent deltas on top: a quarried
        // legacy cliff has lost a quantum of bed and stopped being a wall, without either delta
        // being folded into the other or into the generated height above.
        let finished = FinishedGround {
            generated: GeneratedGround::from_legacy_band(Terrain::Cliff),
            earthwork: GroundDelta::new(-1),
            erosion: GroundDelta::new(0),
            surface: 0,
        };
        assert_eq!(finished.elevation().get(), 2);
        assert!(finished.cliff_quarried());
        assert!(!finished.blocks_movement());
        assert!(!finished.blocks_construction());
        assert_eq!(finished.surface, 0);
    }

    /// The cache is an optimisation and must never be a second generator. Both sources are walked
    /// here because they answer through different code and only the physical one ships.
    #[test]
    fn the_surveyed_cache_is_only_an_echo_of_the_full_oracle() {
        let params = default_world_params();
        let seed = 1_213_486_160;
        for spine in [
            GroundSpine::legacy(&params, seed, true),
            GroundSpine::physical(&params, seed, true),
        ] {
            assert_eq!(spine.cached_cells(), 0);
            spine.cache_chunk(0, 0, 16);
            assert_eq!(spine.cached_cells(), 256);
            for q in -2..=18 {
                for r in -2..=18 {
                    assert_eq!(
                        spine.generated_at(q, r),
                        spine.generated_uncached_at(q, r),
                        "cache drifted at {q},{r}"
                    );
                }
            }
            assert_eq!(
                spine.cached_cells(),
                256,
                "unsurveyed queries do not populate the cache"
            );
        }

        // And a changed source identity has to bypass the cache entirely rather than answer from
        // it, or a world edit would be shown the world it replaced.
        let spine = GroundSpine::legacy(&params, seed, true);
        spine.cache_chunk(0, 0, 16);
        let (q, r) = hexes_in_chunk(0, 0, 16)
            .find(|&(q, r)| spine.generated_at(q, r).presentation != Terrain::Lowland)
            .expect("the generated chunk should contain a non-lowland band");
        assert_eq!(
            spine
                .generated_from(&params, seed, false, q, r)
                .presentation,
            Terrain::Lowland,
            "the old cached environment must not survive a source change"
        );
        assert_eq!(
            spine.cached_cells(),
            256,
            "fallback does not rewrite the cache"
        );
    }

    #[test]
    fn physical_source_translates_the_world_to_a_dry_buildable_shelf() {
        let params = default_world_params();
        let spine = GroundSpine::physical(&params, 1_213_486_160, true);
        let pad = hexes_in_radius((0, 0), crate::terra::LANDING_PAD_RADIUS);
        let clear = hexes_in_radius((0, 0), crate::terra::LANDING_CLEAR_RADIUS);
        let heights: Vec<_> = pad
            .iter()
            .map(|&(q, r)| spine.generated_uncached_at(q, r).bed.get())
            .collect();
        assert!(
            heights.iter().max().unwrap() - heights.iter().min().unwrap()
                <= crate::scale::MAX_BUILD_STEP_QUANTA
        );
        assert!(clear
            .iter()
            .all(|&(q, r)| { spine.generated_uncached_at(q, r).hydrology.depth_quanta == 0 }));
    }
}
