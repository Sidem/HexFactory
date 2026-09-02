/// The value every noise channel is bounded by. `value_noise` interpolates `u16` lattice samples,
/// so every channel lands in `0..=NOISE_MAX` and every threshold below is a point on this scale.
const NOISE_MAX: i32 = 65_535;

/// A gate that admits everything. Noise is never negative, so a rule carrying this on a channel is
/// not asking about that channel at all. Zero would *almost* mean the same thing and would be
/// wrong at exactly the lattice points where a channel samples zero, which is the kind of defect
/// that shows up once in a billion hexes and never reproduces.
const ANY: i32 = -1;

/// One row of the resource table: what a *deposit* is made of, how wide it is, and where its
/// centre is allowed to stand.
///
/// v0.21 moved the unit of a deposit from the hex to the **site**. The row this replaced decided
/// each hex on its own from three noise channels, so a patch's size and a patch's purity were
/// emergent accidents of channel cell size and gate height — neither controllable, nor
/// defaultable, nor measurable. The mixed-material case was the proof: iron gated on richness and
/// coal on vein, two *independent* channels, so wherever both ran high the two alternated hex by
/// hex and an extractor placed there covered both and cleanly worked neither. No pair of numbers
/// fixes that, because the two numbers are not asking one question.
///
/// Rows no longer compete per hex. The lattice picks one rule per site, so **one material per
/// patch** is a property of the model rather than a figure that was tuned into place.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct SiteRule {
    /// The band the site's *centre* must stand in for this rule to be eligible.
    terrain: Terrain,
    item_id: ItemId,
    /// Relative share among the eligible rules for a band. Zero means never — which is how a
    /// preset drops a material from a band without deleting the row that documents it.
    weight: u32,
    /// Inclusive radius range, in hexes. A disc of radius R holds 3R² + 3R + 1 hexes: 7, 19, 37,
    /// 61, 91, 127 at radius 1 through 6.
    radius_min: u32,
    radius_max: u32,
    /// Exclusive lower gate on the richness channel at the *centre*, so the world still has rich
    /// and poor country. `ANY` disables it, on the same reasoning `ANY` already carries.
    #[serde(default = "any_gate")]
    site_min: i32,
    /// Yield at the centre and at the rim, interpolated linearly by distance and then jittered.
    yield_core: u32,
    yield_rim: u32,
    /// Per-hex jitter on the interpolated yield, at least 1: `base + hash % spread` semantics.
    /// Keep it small enough that the core still reads as a core.
    yield_jitter: u32,
    /// Bands a hex must itself be in to belong to this site. Empty means the rule's own band. This
    /// is the clipping that makes a beach a strip and a scree field hug its cliffs.
    #[serde(default)]
    member: Vec<Terrain>,
    /// If set, a member hex must also be within this many hexes of water. `0` disables it.
    #[serde(default)]
    member_water_within: u32,
    /// If set, the centre must stand against *ocean* rather than against any pond: the coarse
    /// elevation octave alone — which is what makes a body big, established and proved in v0.16 —
    /// has to dip below `ocean_level` within `OCEAN_PROBE_RADIUS` of the centre.
    ///
    /// This is a proxy rather than a measurement, deliberately. The map is unbounded and generated
    /// lazily, so nothing here may flood-fill to find out how large a body is. The survey is what
    /// verifies it: it reports the size of the water body nearest each patch of an ocean-gated
    /// material, and a pond-sized number there means the proxy is wrong.
    #[serde(default)]
    center_ocean: bool,
    /// If set, the centre must stand next to the shore band. Cheaper than asking `terrain_at` —
    /// shore is an elevation cut, so one octave answers it — and the right question for a beach
    /// that is not an ocean: a lake and a sea both grow sandy tiles, and a rule that only asked
    /// the ocean proxy turned every inland beach into clay.
    #[serde(default)]
    center_shore: bool,
}

fn any_gate() -> i32 {
    ANY
}

/// The guaranteed opening, keyed by the lattice cell each promise claimed.
type BootstrapTable = BTreeMap<(i32, i32), Site>;

/// One step of the bootstrap pass's outward spiral: how far a lattice cell's centre stands from the
/// landing site, the cell, and that centre. Sorted, so the distance leads and the cell breaks ties.
type SpiralStep = (i32, (i32, i32), (i32, i32));

/// One deposit, resolved from the lattice cell that owns it. Derived from `(params, seed, cell)`
/// and nothing else, which is what lets the lattice be cached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Site {
    center: (i32, i32),
    /// Index into the parameter set's rule table.
    rule: usize,
    radius: i32,
    /// A guaranteed physical-world outcrop may replace the legacy presentation-band gate with dry
    /// substrate. This is derived bootstrap identity and is never saved or checksummed.
    forced_opening: bool,
}

/// The salt the site lattice hashes under, kept clear of every noise octave so which material a
/// deposit holds never correlates with the elevation under it.
const SITE_SALT: u32 = 0x5175E;
/// The row every derived field of a cell hash is drawn on.
const SITE_FIELD_ROW: i32 = 0x517E;
/// The octave the river channel is sampled on.
const RIVER_OCTAVE: u32 = 0xF10DE;
/// The octave the richness channel is sampled on. It gates a site's *centre* now rather than every
/// hex, which is what leaves the world with rich and poor country without deciding materials.
const RICHNESS_OCTAVE: u32 = 0x0E55;
/// A smooth, site-independent edge mask. It perturbs only the outside ring of a deposit, keeping
/// one material per site while stopping every unconstrained field from reading as a perfect disc.
const SITE_SHAPE_OCTAVE: u32 = 0x5A9E;
const SITE_SHAPE_CELL: i32 = 3;
/// How far from an ocean-gated centre the coarse octave is probed for open sea.
const OCEAN_PROBE_RADIUS: i32 = 2;
/// How far a shore-gated centre may stand from the shore band and still count as a beach site.
/// Sand's disc is radius 3–5, so a probe shorter than that would refuse the inland side of a
/// beach the disc itself can still paint.
const SHORE_PROBE_RADIUS: i32 = 4;
/// The largest radius a rule may claim, and the largest wander a centre may take inside its cell.
/// `field_at` scans every lattice cell within reach of a hex and reach grows with both, so a
/// parameter set is not allowed to make that scan unbounded — the same judgement `MAX_FEATURE_CELL`
/// already makes about a lattice stride.
const MAX_SITE_RADIUS: u32 = 8;
const MAX_SITE_JITTER: i32 = 16;
/// The hexes a base extractor covers, and so the smallest patch worth standing one on: a disc of
/// radius R holds 3R² + 3R + 1 hexes, which is 7 at the reach the hand and the base extractor
/// share. Derived from the reach rather than written down, so raising one moves the other.
const WORKABLE_PATCH_HEXES: u32 =
    (3 * EXTRACT_RADIUS * EXTRACT_RADIUS + 3 * EXTRACT_RADIUS + 1) as u32;

/// The knobs a world is generated from.
///
/// Unlike the shape grammar, this is **simulation truth**: two worlds sharing a seed and differing
/// here are different worlds, so parameters travel in the save envelope and the checksum, and
/// `WORLD_GENERATOR_VERSION` moved to 6 when they entered.
///
/// Feature scale and threshold are separate axes on purpose, because they are the pair a generator
/// most easily conflates. **Raising the sea level makes more water, not bigger water** — it
/// produces more ponds. Lakes, seas, and oceans come from a larger `elevation_coarse_cell` and a
/// larger share of the blend for that octave. The same split holds for every other band: where the
/// cuts sit is "how much", and the cell sizes are "ponds or oceans, hillocks or ranges".
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct WorldParams {
    /// The low-frequency elevation octave: the landform scale.
    elevation_coarse_cell: i32,
    /// The high-frequency octave that breaks up a coastline.
    elevation_fine_cell: i32,
    /// The coarse octave's share of the blend, in percent; the fine octave takes the rest. 50
    /// reproduces the `noise(8) / 2 + noise(3) / 2` that generator version 5 was frozen at.
    elevation_coarse_weight: i32,
    moisture_cell: i32,
    richness_cell: i32,
    /// Band cuts on the noise scale, in ascending order.
    water_level: i32,
    shore_level: i32,
    hills_level: i32,
    highland_level: i32,
    /// A hex whose steepest neighbour step exceeds this reads as cliff.
    cliff_step: i32,
    /// Water wetter than this is deep.
    deep_water_moisture: i32,
    /// The lattice a deposit is drawn on. One site cell holds at most one site, so this is how far
    /// apart deposits stand; `site_jitter` is how far a centre may wander inside its own cell, so
    /// that a world of deposits is not a world on a visible grid.
    site_cell: i32,
    site_jitter: i32,
    /// Rivers. `river_cell` is how far apart they run, `river_width` is the half-width of the band
    /// the channel is read against and so how wide a river is — `0` is a world without rivers —
    /// and `river_max_elevation` is where they stop, so no river runs over a summit.
    river_cell: i32,
    river_width: i32,
    river_max_elevation: i32,
    /// The cut the *coarse* elevation octave alone is read against when a rule asks for ocean.
    /// A pond exists only in the fine octave and fails it; an ocean coast passes.
    ocean_level: i32,
    site_rules: Vec<SiteRule>,
}

impl WorldParams {
    /// Whether the four elevation cuts ascend. Ascending cuts are what makes each band reachable:
    /// out of order, a band is not rare — it is unreachable, and the world silently loses whatever
    /// the table put in it. Its own predicate because a repair has to ask it before it offers a set
    /// `validate` would then refuse.
    fn band_levels_ascend(&self) -> bool {
        self.water_level < self.shore_level
            && self.shore_level < self.hills_level
            && self.hills_level < self.highland_level
    }

    /// Every way a parameter set can be nonsense, asked once, before a world is built from it.
    /// A set that generates an unplayable world is a real failure mode and this is not what
    /// catches it — the survey tool is. This catches the sets that are not worlds at all.
    fn validate(&self, definitions: &DefinitionsInput) -> Result<(), String> {
        let cells = [
            ("elevation_coarse_cell", self.elevation_coarse_cell),
            ("elevation_fine_cell", self.elevation_fine_cell),
            ("moisture_cell", self.moisture_cell),
            ("richness_cell", self.richness_cell),
            ("site_cell", self.site_cell),
            ("river_cell", self.river_cell),
        ];
        for (name, cell) in cells {
            if !(1..=MAX_FEATURE_CELL).contains(&cell) {
                return Err(format!(
                    "world parameter {name} must be between 1 and {MAX_FEATURE_CELL}"
                ));
            }
        }
        if !(0..=100).contains(&self.elevation_coarse_weight) {
            return Err("world parameter elevation_coarse_weight must be a percentage".into());
        }
        let levels = [
            ("water_level", self.water_level),
            ("shore_level", self.shore_level),
            ("hills_level", self.hills_level),
            ("highland_level", self.highland_level),
        ];
        for (name, level) in levels {
            if !(0..=NOISE_MAX).contains(&level) {
                return Err(format!("world parameter {name} is outside the noise range"));
            }
        }
        if !self.band_levels_ascend() {
            return Err("world band levels must ascend: water < shore < hills < highland".into());
        }
        if !(0..=NOISE_MAX).contains(&self.cliff_step) || self.cliff_step == 0 {
            return Err("world parameter cliff_step is outside the noise range".into());
        }
        if !(ANY..=NOISE_MAX).contains(&self.deep_water_moisture) {
            return Err("world parameter deep_water_moisture is outside the noise range".into());
        }
        if !(0..=MAX_SITE_JITTER).contains(&self.site_jitter) {
            return Err(format!(
                "world parameter site_jitter must be between 0 and {MAX_SITE_JITTER}"
            ));
        }
        for (name, level) in [
            ("river_width", self.river_width),
            ("river_max_elevation", self.river_max_elevation),
            ("ocean_level", self.ocean_level),
        ] {
            if !(0..=NOISE_MAX).contains(&level) {
                return Err(format!("world parameter {name} is outside the noise range"));
            }
        }
        if self.site_rules.is_empty() {
            return Err("world parameters need at least one site rule".into());
        }
        // A site rule that could name a water band would make the cheap water test `field_at`
        // opens with unsound, and a deposit in a basin is not a thing a pump or an extractor can
        // reach anyway. Refusing it here is what lets the fast path skip the band decision.
        let dry = |terrain: Terrain| !terrain.is_water();
        let mut placeable = false;
        for rule in &self.site_rules {
            let named = definitions
                .items
                .iter()
                .find(|item| item.id == rule.item_id);
            let Some(named) = named else {
                return Err(format!("site rule names unknown item {}", rule.item_id));
            };
            // An extractor can be stood on anything a rule can place, so anything a rule can place
            // has to price extraction. Custom parameters come through here too, which is why the
            // check lives beside the rule rather than only beside the built-in presets.
            if named.extract_steps.is_none() {
                return Err(format!(
                    "site rule names item {} ({}), which has no extract_steps",
                    named.id, named.key
                ));
            }
            if !dry(rule.terrain) || !rule.member.iter().copied().all(dry) {
                return Err("a site rule may not name a water band".into());
            }
            if rule.radius_min == 0 || rule.radius_min > rule.radius_max {
                return Err("site rule radii must ascend from at least 1".into());
            }
            if rule.radius_max > MAX_SITE_RADIUS {
                return Err(format!(
                    "site rule radius_max may not exceed {MAX_SITE_RADIUS}"
                ));
            }
            // Yield is `interpolated + hash % yield_jitter`, so a zero jitter is a division by zero.
            if rule.yield_jitter == 0 {
                return Err("site rule yield_jitter must be at least 1".into());
            }
            if rule.yield_core == 0 || rule.yield_rim == 0 {
                return Err("site rule yields must be at least 1".into());
            }
            if !(ANY..=NOISE_MAX).contains(&rule.site_min) {
                return Err("site rule gate is outside the noise range".into());
            }
            placeable |= rule.weight > 0;
        }
        if !placeable {
            return Err("every site rule is weighted zero, so the world holds nothing".into());
        }
        Ok(())
    }
}

/// The largest feature cell a parameter set may ask for. A cell is a lattice stride, so this is a
/// bound on how far apart two sampled corners may be — not a taste judgement. It keeps a
/// pathological value from making an entire surveyed world one interpolated slope. 1024 hexes is
/// a six-minute walk at 15 m/s, which is the scale oceans and ranges are allowed to ask for.
const MAX_FEATURE_CELL: i32 = 1024;
/// Landforms smaller than this are opening-sized: the bootstrap windows were tuned against a
/// coarse cell of 8, and a synthetic scale sweep (cell 4 vs 24) has to measure feature size, not a
/// landing pad. Shipped presets all sit well above it.
const LANDING_SCALE_CELL: i32 = 32;
/// The opening's own landform scale — v0.21 continental's cell 8 / 3 / 50, which is what the
/// bootstrap windows were measured against. A frozen regional coarse sample cannot produce
/// highland, lowland, and water inside 14 hexes of each other; this one can.
const OPENING_COARSE_CELL: i32 = 8;
const OPENING_FINE_CELL: i32 = 3;
const OPENING_COARSE_WEIGHT: i32 = 50;
/// Hexes around the hub that stay free of rivers, so the first minute is not a moat. Clay's
/// bootstrap window starts at 15, so a river can still be the first pump site.
const RIVER_CLEAR_RADIUS: i32 = LANDING_CLEAR_RADIUS + 6;

/// How far the opening scale fades into the regional one. Half a landform, clamped so a
/// 1024-cell custom world does not force a kilometre of fake continent.
fn landing_radius(params: &WorldParams) -> i32 {
    if params.elevation_coarse_cell < LANDING_SCALE_CELL {
        return 0;
    }
    (params.elevation_coarse_cell / 2).clamp(64, 200)
}

/// Ridge-noise half-width that reads as `hex_width` hexes of river at this `river_cell`.
/// The channel is interpolated over `river_cell`, so a wider cell at the same threshold is a
/// wider river — this inverts that, so a preset can name a width in hexes.
fn river_width_for(river_cell: i32, hex_width: i32) -> i32 {
    (hex_width * NOISE_MAX) / (2 * river_cell.max(1))
}

fn blend_elevation(coarse: i32, fine: i32, weight: i32) -> i32 {
    (coarse * weight + fine * (100 - weight)) / 100
}

/// Neighbour steps scale with the cell, so a `cliff_step` tuned for a 512-hex landform reads as
/// "everything is sheer" at the opening's cell 8. The inner disc uses the step that cell 8 / 3
/// actually needs; the regional value takes over with the landform.
fn cliff_step_at(params: &WorldParams, dist: i32) -> i32 {
    let radius = landing_radius(params);
    if radius == 0 || dist >= radius {
        return params.cliff_step;
    }
    let opening = 14_000;
    let inner = radius * 2 / 5;
    if dist <= inner {
        return opening;
    }
    let t = (dist - inner) * 100 / (radius - inner).max(1);
    (opening * (100 - t) + params.cliff_step * t) / 100
}

/// Same split for rivers: an 8-hex river on a 320-hex channel is a lake across the whole
/// opening, because the channel does not move. The inner disc keeps the one-hex creeks the
/// bootstrap was measured against; the wide river starts with the regional landform.
fn river_params_at(params: &WorldParams, dist: i32) -> (i32, i32) {
    let radius = landing_radius(params);
    let inner = radius * 2 / 5;
    if radius == 0 || dist >= inner || params.river_width == 0 {
        return (params.river_cell, params.river_width);
    }
    let cell = params.river_cell.min(32);
    (cell, river_width_for(cell, 1).min(params.river_width))
}

fn elevation_at(params: &WorldParams, seed: u32, q: i32, r: i32) -> i32 {
    let regional = blend_elevation(
        value_noise(seed, q, r, params.elevation_coarse_cell, 0xA11CE),
        value_noise(seed, q, r, params.elevation_fine_cell, 0xB0A7),
        params.elevation_coarse_weight,
    );
    let radius = landing_radius(params);
    let dist = axial_distance((0, 0), (q, r));
    if radius == 0 || dist >= radius {
        return regional;
    }
    // The inner two-fifths is the opening the bootstrap was tuned against. Past that the
    // regional landform takes over, so a three-minute plains is still a three-minute plains
    // once you leave the first minute.
    let local = blend_elevation(
        value_noise(
            seed,
            q,
            r,
            params.elevation_coarse_cell.min(OPENING_COARSE_CELL),
            0xA11CE,
        ),
        value_noise(
            seed,
            q,
            r,
            params.elevation_fine_cell.min(OPENING_FINE_CELL),
            0xB0A7,
        ),
        OPENING_COARSE_WEIGHT.min(params.elevation_coarse_weight),
    );
    let inner = radius * 2 / 5;
    if dist <= inner {
        return local;
    }
    let t = (dist - inner) * 100 / (radius - inner).max(1);
    (local * (100 - t) + regional * t) / 100
}

fn moisture_at(params: &WorldParams, seed: u32, q: i32, r: i32) -> i32 {
    value_noise(seed, q, r, params.moisture_cell, 0xC0A5)
}

fn terrain_at(
    params: &WorldParams,
    seed: u32,
    q: i32,
    r: i32,
    generated_environment: bool,
) -> Terrain {
    if !generated_environment {
        return Terrain::Lowland;
    }
    if axial_distance((0, 0), (q, r)) <= LANDING_CLEAR_RADIUS {
        return match (q, r) {
            (2, 1) | (2, 2) | (1, 2) => Terrain::ShallowWater,
            (1, -1) | (2, -1) => Terrain::Cliff,
            _ => Terrain::Lowland,
        };
    }
    let elevation = elevation_at(params, seed, q, r);
    let moisture = moisture_at(params, seed, q, r);
    if elevation < params.water_level {
        return if moisture > params.deep_water_moisture {
            Terrain::DeepWater
        } else {
            Terrain::ShallowWater
        };
    }
    if elevation < params.shore_level {
        return Terrain::Shore;
    }
    let dist = axial_distance((0, 0), (q, r));
    if is_river(params, seed, q, r, elevation) {
        return Terrain::ShallowWater;
    }
    let mut max_step = 0;
    for &(dq, dr) in &DIRECTIONS {
        max_step = max_step.max((elevation - elevation_at(params, seed, q + dq, r + dr)).abs());
    }
    if max_step > cliff_step_at(params, dist) {
        return Terrain::Cliff;
    }
    if elevation > params.highland_level {
        Terrain::Highland
    } else if elevation > params.hills_level {
        Terrain::Hills
    } else {
        Terrain::Lowland
    }
}

/// A river hex, which is inland `ShallowWater` rather than an accident of sea level.
///
/// A flow simulation is refused outright: the map is unbounded and generated lazily, so nothing
/// here may depend on knowing where the water upstream went. A river is instead where a dedicated
/// channel runs near its own midpoint, which is O(1) per hex, purely local, and fits the pure
/// `(params, seed, q, r)` contract exactly. `elevation` is passed in because every caller has just
/// computed it, and it is the gate that stops a river at the highland cut.
fn is_river(params: &WorldParams, seed: u32, q: i32, r: i32, elevation: i32) -> bool {
    let dist = axial_distance((0, 0), (q, r));
    if params.river_width == 0
        || elevation >= params.river_max_elevation
        || dist <= RIVER_CLEAR_RADIUS
    {
        return false;
    }
    let (cell, width) = river_params_at(params, dist);
    if width == 0 {
        return false;
    }
    let channel = value_noise(seed, q, r, cell, RIVER_OCTAVE);
    (channel - NOISE_MAX / 2).abs() < width
}

/// Water, asked the cheap way.
///
/// `terrain_at` samples seven elevations to answer the cliff question and a water test needs none
/// of them, so the hot paths that only want "is this wet" — the clay clipping and the barren
/// early-out in `field_at` — ask here instead. It mirrors `terrain_at` exactly, clearing included,
/// and a test asserts the two never disagree.
#[allow(dead_code)]
fn is_water_at(params: &WorldParams, seed: u32, q: i32, r: i32) -> bool {
    if axial_distance((0, 0), (q, r)) <= LANDING_CLEAR_RADIUS {
        return matches!((q, r), (2, 1) | (2, 2) | (1, 2));
    }
    let elevation = elevation_at(params, seed, q, r);
    elevation < params.water_level
        || (elevation >= params.shore_level && is_river(params, seed, q, r, elevation))
}
