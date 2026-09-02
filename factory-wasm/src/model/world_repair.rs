/// The share of the opening a band is widened to when a guarantee cannot find it. A starting point
/// rather than a rule: what decides a repair is the verification, and this is only where the search
/// for one begins.
const REPAIR_BAND_SHARE: usize = 15;
/// Deposit spacings a repair will try, widest first, so a fix settles on the largest lattice that
/// still opens the world rather than the smallest one that certainly does.
const REPAIR_SPACINGS: [i32; 5] = [32, 24, 16, 12, 8];
/// Seeds a repair will try before it touches a parameter at all. A seed is the one thing on the
/// form the player did not choose, so rerolling it is the fix that costs them nothing — but a
/// world that drowns every material drowns them under every seed, which is why the list is short.
const REPAIR_SEEDS: u32 = 8;

/// A seed that opens this world with every parameter left alone.
fn repair_seed(params: &WorldParams, seed: u32) -> Option<u32> {
    (1..=REPAIR_SEEDS)
        .map(|step| seed.wrapping_add(step))
        .find(|&candidate| bootstraps(params, candidate))
}

/// One way a repair may turn a knob. Takes the bands the failed guarantees were looking for,
/// because a repair that widened every band would be a reset rather than a fix.
type RepairMove = fn(&WorldParams, u32, &[Terrain]) -> WorldParams;

/// Ways to repair a world, fewest knobs first. Every rung is verified, so a rung that does not open
/// the world is simply never offered — which is what lets the list stay a list of guesses.
const REPAIR_LADDER: [&[RepairMove]; 4] = [
    &[],
    &[repair_cuts],
    &[repair_landform],
    &[repair_cuts, repair_landform, repair_rivers],
];

/// A parameter set that opens this world at the seed the player is on, or none that was found.
///
/// The search is a ladder rather than a solver: a handful of candidates, ordered so the first one
/// that works is also the one that does least to what the player asked for. Deposit spacing is the
/// outer loop because it is the knob a repair would rather not touch — a player who set it to an
/// expedition per material meant it — so everything else is tried at their spacing first.
fn repair_params(params: &WorldParams, seed: u32) -> Option<WorldParams> {
    let spine = GroundSpine::physical(params, seed, true);
    let unmet = bootstrap_sites(params, seed, &spine).1;
    let mut needed: Vec<Terrain> = unmet
        .iter()
        .flat_map(|&(item_id, _)| bootstrap_bands(params, item_id))
        .collect();
    needed.sort_unstable();
    needed.dedup();
    let spacings = std::iter::once(params.site_cell).chain(
        REPAIR_SPACINGS
            .into_iter()
            .filter(|&cell| cell < params.site_cell),
    );
    for site_cell in spacings {
        for moves in REPAIR_LADDER {
            let mut candidate = WorldParams {
                site_cell,
                ..params.clone()
            };
            for step in moves {
                candidate = step(&candidate, seed, &needed);
            }
            // The unchanged set is the one that is already known to fail, and a candidate native
            // would refuse is not a fix — `Core::new` would decline it on arrival.
            if candidate != *params
                && candidate.band_levels_ascend()
                && bootstraps(&candidate, seed)
            {
                return Some(candidate);
            }
        }
    }
    None
}

/// Give the landform back the scale the opening was tuned against.
///
/// Below `LANDING_SCALE_CELL` there is no opening blend at all and the ground near the hub is a
/// mosaic at the regional cell: every band is present and none of them holds a patch big enough to
/// stand an extractor on. Raising the cell is what turns that mosaic back into country.
fn repair_landform(params: &WorldParams, _seed: u32, _needed: &[Terrain]) -> WorldParams {
    WorldParams {
        elevation_coarse_cell: params.elevation_coarse_cell.max(LANDING_SCALE_CELL),
        ..params.clone()
    }
}

/// Narrow rivers to the creeks the bootstrap was measured against. A river is shallow water
/// whatever the elevation cuts say, so a wide enough channel drowns an opening the cuts alone
/// cannot rescue.
fn repair_rivers(params: &WorldParams, _seed: u32, _needed: &[Terrain]) -> WorldParams {
    WorldParams {
        river_width: params
            .river_width
            .min(river_width_for(params.river_cell, 1)),
        ..params.clone()
    }
}

/// Move the band cuts so every band a failed guarantee was looking for has room in the ground the
/// opening actually holds.
///
/// The cuts are quantiles of the elevation around the landing site, which is what makes "give
/// highland a share of the ground" a thing that can be computed rather than guessed: a band is a
/// slice of that distribution and not a number on a slider. Each starving band takes its room from
/// *below*, so its own lower cut is the one that moves — a world missing highland is repaired
/// without raising the sea, and a drowned world is repaired by lowering the cut that drowned it.
fn repair_cuts(params: &WorldParams, seed: u32, needed: &[Terrain]) -> WorldParams {
    let mut samples: Vec<i32> = bootstrap_cells(params, seed)
        .iter()
        .map(|&(_, _, center)| elevation_at(params, seed, center.0, center.1))
        .collect();
    if samples.is_empty() {
        return params.clone();
    }
    samples.sort_unstable();
    let room = (samples.len() * REPAIR_BAND_SHARE / 100).max(1);
    let at = |index: usize| samples[index.min(samples.len() - 1)];
    let mut next = params.clone();
    // Top down, so each band is measured against a ceiling that has already stopped moving.
    if needed.contains(&Terrain::Highland) {
        let above = samples.len() - samples.partition_point(|&e| e <= next.highland_level);
        if above < room {
            next.highland_level = at(samples.len() - room) - 1;
        }
    }
    if needed.contains(&Terrain::Hills) {
        let ceiling = samples.partition_point(|&e| e <= next.highland_level);
        if ceiling
            - samples
                .partition_point(|&e| e <= next.hills_level)
                .min(ceiling)
            < room
        {
            next.hills_level = at(ceiling.saturating_sub(room)) - 1;
        }
    }
    if needed.contains(&Terrain::Lowland) {
        let ceiling = samples.partition_point(|&e| e <= next.hills_level);
        if ceiling
            - samples
                .partition_point(|&e| e < next.shore_level)
                .min(ceiling)
            < room
        {
            next.shore_level = at(ceiling.saturating_sub(room));
        }
    }
    if needed.contains(&Terrain::Shore) {
        let ceiling = samples.partition_point(|&e| e < next.shore_level);
        if ceiling
            - samples
                .partition_point(|&e| e < next.water_level)
                .min(ceiling)
            < room
        {
            next.water_level = at(ceiling.saturating_sub(room));
        }
    }
    for level in [
        &mut next.water_level,
        &mut next.shore_level,
        &mut next.hills_level,
        &mut next.highland_level,
    ] {
        *level = (*level).clamp(0, NOISE_MAX);
    }
    // A band that took its room from below can leave a cut stranded above the one over it — a sea
    // higher than its own shore, which `validate` refuses. Each cut follows the one above it back
    // down, which keeps the rule the moves are built on: the sea falls, it never rises. A chain
    // that bottoms out at zero simply fails to ascend, and an unascending candidate is discarded
    // rather than offered.
    next.hills_level = next.hills_level.min(next.highland_level.saturating_sub(1));
    next.shore_level = next.shore_level.min(next.hills_level.saturating_sub(1));
    next.water_level = next.water_level.min(next.shore_level.saturating_sub(1));
    next
}

/// The resource field of one world: a pure function of parameters, seed, and hex, with the lattice
/// those answers are derived from cached.
///
/// The cache is the site lattice and never the field. `field_at` is not only called during
/// `generate_chunk` — `deposit_candidates` walks a whole disc, and `resource_at_world`, both
/// gathers, and every snapshot build reach it — and the naive form evaluates every lattice cell
/// within reach per hex, each one deciding a band, which is roughly 350 noise samples per hex and
/// is not shippable. A site cell is `site_cell²` hexes, so the map stays small and every hex in a
/// chunk hits it warm.
///
/// Both the lattice and the bootstrap table are derived state under the existing invariant: never
/// saved, never hashed, never checksummed, rebuilt whenever the world changes, exactly as
/// `deposit_links` is.
struct WorldFields {
    params: WorldParams,
    seed: u32,
    /// How far from the cell holding a hex a site may still reach it, in lattice cells.
    ///
    /// A site's centre sits inside its own cell plus `site_jitter`, and `axial_distance <= radius`
    /// implies each axial component is at most `radius`, so a cell more than
    /// `(radius_max + site_jitter + site_cell - 1) / site_cell` away cannot cover the hex. That is
    /// a derivation rather than a margin: a reach one cell short loses deposits silently.
    reach: i32,
    bootstrap: BootstrapTable,
    /// Guarantees the bootstrap pass could not place, with the distance it gave up at, so a caller
    /// can refuse the world instead of shipping a world that cannot be opened.
    unmet: Vec<(ItemId, i32)>,
    sites: RefCell<BTreeMap<(i32, i32), Option<Site>>>,
}

impl WorldFields {
    fn new(params: &WorldParams, seed: u32, spine: &GroundSpine) -> Self {
        let (bootstrap, unmet) = bootstrap_sites(params, seed, spine);
        let radius_max = params
            .site_rules
            .iter()
            .map(|rule| rule.radius_max as i32)
            .max()
            .unwrap_or(0);
        Self {
            reach: (radius_max + params.site_jitter + params.site_cell - 1) / params.site_cell,
            params: params.clone(),
            seed,
            bootstrap,
            unmet,
            sites: RefCell::new(BTreeMap::new()),
        }
    }

    fn site_at(&self, cell: (i32, i32), spine: &GroundSpine) -> Option<Site> {
        if let Some(&site) = self.sites.borrow().get(&cell) {
            return site;
        }
        let site = self.site_uncached(cell, spine);
        self.sites.borrow_mut().insert(cell, site);
        site
    }

    /// The same answer with the cache bypassed. The survey and the tests call the generator without
    /// a warm lattice, and one test asserts the two paths agree over a disc.
    fn site_uncached(&self, cell: (i32, i32), spine: &GroundSpine) -> Option<Site> {
        self.bootstrap
            .get(&cell)
            .copied()
            .or_else(|| natural_site(&self.params, self.seed, cell, spine))
    }

    /// What the bootstrap pass actually placed, per guaranteed material: the walk from the landing
    /// site to the nearest hex of the patch, and how many hexes the patch holds once the member
    /// test has clipped it. A guarantee the pass gave up on is simply absent, which is the shape
    /// every caller wants — the survey prints it as `none` and `Core::new` refuses the world.
    #[cfg(not(target_arch = "wasm32"))]
    fn guarantees(&self, spine: &GroundSpine) -> Vec<(ItemId, u32, u32)> {
        self.bootstrap
            .values()
            .map(|site| {
                (
                    self.params.site_rules[site.rule].item_id,
                    (axial_distance((0, 0), site.center) - site.radius).max(0) as u32,
                    member_hexes(&self.params, self.seed, site, spine),
                )
            })
            .collect()
    }

    fn field_at(
        &self,
        q: i32,
        r: i32,
        generated_environment: bool,
        spine: &GroundSpine,
    ) -> Option<ResourceState> {
        if !generated_environment {
            return None;
        }
        // The clearing is a promise rather than a landscape, and its field suppression is what the
        // bootstrap windows are measured against.
        if axial_distance((0, 0), (q, r)) <= LANDING_CLEAR_RADIUS {
            return None;
        }
        // No rule may name a water band — `validate` refuses one that tries — so the cheap water
        // test comes before the lattice scan and before the seven elevations a band costs.
        if spine.wet_at(q, r) {
            return None;
        }
        let band = spine.presentation_at(q, r);
        let cell = (
            floor_div(q, self.params.site_cell),
            floor_div(r, self.params.site_cell),
        );
        let mut best: Option<((i32, i32, i32), Site)> = None;
        for step_q in -self.reach..=self.reach {
            for step_r in -self.reach..=self.reach {
                let candidate = (cell.0 + step_q, cell.1 + step_r);
                let Some(site) = self.site_at(candidate, spine) else {
                    continue;
                };
                let Some(distance) = site_covers(&self.params, self.seed, &site, q, r, band, spine)
                else {
                    continue;
                };
                // Nearest centre wins, and the lattice cell breaks the tie. Ties must be broken
                // explicitly: a tie resolved by iteration order is a tie resolved by nothing, and
                // this is a checksum input.
                let key = (distance, candidate.0, candidate.1);
                if best.as_ref().is_none_or(|(current, _)| key < *current) {
                    best = Some((key, site));
                }
            }
        }
        let ((distance, _, _), site) = best?;
        let rule = &self.params.site_rules[site.rule];
        // Linear from core to rim, so the middle of a field is worth aiming an extractor at.
        let span = site.radius.max(1);
        let core = rule.yield_core as i32;
        let rim = rule.yield_rim as i32;
        let interpolated = rim + (core - rim) * (span - distance) / span;
        let quantity =
            interpolated.max(1) as u32 + coordinate_hash(self.seed, q, r) % rule.yield_jitter;
        Some(ResourceState {
            item_id: rule.item_id,
            quantity,
            initial_quantity: quantity,
        })
    }
}
