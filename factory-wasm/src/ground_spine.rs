//! Phase 8's production ground spine, behind the shipped presentation.
//!
//! The seven shipped [`Terrain`] bands currently answer four different questions at once: height,
//! material, water and presentation. This module separates those answers without activating the
//! Phase 8 physical generator. [`GroundSpine::generated_uncached_at`] is the full legacy oracle;
//! the cache only holds surveyed chunks and must always echo that oracle. Slice 3 can replace the
//! source without teaching walking, placement or ground transactions another route to height.
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Substrate {
    Sand,
    Meadow,
    Soil,
    Rock,
}

/// The legacy water distinction, separated from bed height and substrate.
///
/// Slice 4 replaces this compatibility class with numeric depth and discharge. Keeping the class
/// explicit now prevents `DeepWater` and `ShallowWater` from remaining hidden altitude values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WaterDepthClass {
    Dry,
    Shallow,
    Deep,
}

/// Initial water above the generated bed. Discharge is zero because ridge-noise water is not a
/// live flow model; the field exists now so the physical source has an independent output to fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct InitialHydrology {
    pub(super) depth: WaterDepthClass,
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
        let depth = match terrain {
            Terrain::DeepWater => WaterDepthClass::Deep,
            Terrain::ShallowWater => WaterDepthClass::Shallow,
            _ => WaterDepthClass::Dry,
        };
        // These are compatibility surfaces in legacy steps, not physical water levels. Nothing
        // reads them for simulation before the physical source activates.
        let surface = match depth {
            WaterDepthClass::Deep => GroundElevation::new(0),
            WaterDepthClass::Shallow => GroundElevation::new(1),
            WaterDepthClass::Dry => bed,
        };
        Self {
            bed,
            substrate,
            hydrology: InitialHydrology {
                depth,
                surface,
                discharge_class: 0,
            },
            presentation: terrain,
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
pub(super) struct GroundSpine {
    params: WorldParams,
    seed: u32,
    generated_environment: bool,
    cache: RefCell<BTreeMap<(i32, i32), GeneratedGround>>,
}

impl GroundSpine {
    pub(super) fn legacy(params: &WorldParams, seed: u32, generated_environment: bool) -> Self {
        Self {
            params: params.clone(),
            seed,
            generated_environment,
            cache: RefCell::new(BTreeMap::new()),
        }
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
        GeneratedGround::from_legacy_band(terrain_at(
            &self.params,
            self.seed,
            q,
            r,
            self.generated_environment,
        ))
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
            (
                Terrain::DeepWater,
                -1,
                Substrate::Soil,
                WaterDepthClass::Deep,
                0,
            ),
            (
                Terrain::ShallowWater,
                0,
                Substrate::Soil,
                WaterDepthClass::Shallow,
                1,
            ),
            (Terrain::Shore, 0, Substrate::Sand, WaterDepthClass::Dry, 0),
            (
                Terrain::Lowland,
                0,
                Substrate::Meadow,
                WaterDepthClass::Dry,
                0,
            ),
            (Terrain::Hills, 1, Substrate::Soil, WaterDepthClass::Dry, 1),
            (
                Terrain::Highland,
                2,
                Substrate::Rock,
                WaterDepthClass::Dry,
                2,
            ),
            (Terrain::Cliff, 3, Substrate::Rock, WaterDepthClass::Dry, 3),
        ];
        for (terrain, elevation, substrate, depth, water_surface) in cases {
            let ground = GeneratedGround::from_legacy_band(terrain);
            assert_eq!(ground.bed.get(), elevation);
            assert_eq!(ground.substrate, substrate);
            assert_eq!(ground.hydrology.depth, depth);
            assert_eq!(ground.hydrology.surface.get(), water_surface);
            assert_eq!(ground.hydrology.discharge_class, 0);
            assert_eq!(ground.presentation, terrain);
        }
    }

    #[test]
    fn surveyed_cache_is_only_an_echo_of_the_full_oracle() {
        let params = default_world_params();
        let spine = GroundSpine::legacy(&params, 1_213_486_160, true);
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

    #[test]
    fn a_changed_source_identity_bypasses_the_surveyed_cache() {
        let params = default_world_params();
        let seed = 1_213_486_160;
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
    fn finished_ground_keeps_deltas_separate_and_quarries_one_legacy_cliff() {
        let generated = GeneratedGround::from_legacy_band(Terrain::Cliff);
        let finished = FinishedGround {
            generated,
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
}
