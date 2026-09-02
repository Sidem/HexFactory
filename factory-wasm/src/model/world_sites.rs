/// Item ids the generator writes into the world. Generation is content, so these name the shipped
/// catalog the same way the guaranteed opening below does.
const IRON_ORE: ItemId = 1;
const CRYSTAL: ItemId = 3;
const COPPER_ORE: ItemId = 4;
const COAL: ItemId = 5;
const STONE: ItemId = 6;
const SAND: ItemId = 7;
const CLAY: ItemId = 8;
const WOOD: ItemId = 9;
const LIMESTONE: ItemId = 26;
const CRUDE_OIL: ItemId = 28;

/// The shipped resource table. Order is no longer a generation input — the lattice weights one
/// rule against the others eligible for a band rather than taking the first that matches — so this
/// reads top to bottom as the bands do, from the tops to the coast.
///
/// Every number here was chosen against `npm run survey`, the way `cliff_step` was chosen in v0.16.
fn default_site_rules() -> Vec<SiteRule> {
    let rule =
        |terrain, item_id, weight, radius_min, radius_max, site_min, core, rim, jitter| SiteRule {
            terrain,
            item_id,
            weight,
            radius_min,
            radius_max,
            site_min,
            yield_core: core,
            yield_rim: rim,
            yield_jitter: jitter,
            member: Vec::new(),
            member_water_within: 0,
            center_ocean: false,
            center_shore: false,
        };
    // Iron and coal both belong to the tops and the rolling ground under them, so both name the
    // pair as members and neither is clipped to the band its centre happened to land in. That is
    // the whole mixed-material fix seen from the data side: they are separate *sites* now, so two
    // neighbouring fields is what a smelting site looks like, never one alternating hex.
    let ore_bands = vec![Terrain::Hills, Terrain::Highland];
    vec![
        SiteRule {
            member: ore_bands.clone(),
            ..rule(Terrain::Highland, IRON_ORE, 34, 3, 4, 28_000, 20, 8, 3)
        },
        SiteRule {
            member: ore_bands.clone(),
            ..rule(Terrain::Hills, IRON_ORE, 24, 3, 4, 28_000, 20, 8, 3)
        },
        SiteRule {
            member: ore_bands.clone(),
            ..rule(Terrain::Highland, COAL, 26, 2, 4, ANY, 18, 8, 3)
        },
        // Scree around mountains. Cliff hexes are members and are unworkable, so the buildable rim
        // is where you quarry — v0.11's extraction-radius lesson intact, at fifty times the supply
        // the eighteen cliff cells of version 6 could offer.
        SiteRule {
            member: vec![Terrain::Highland, Terrain::Cliff],
            ..rule(Terrain::Highland, STONE, 26, 3, 5, ANY, 12, 12, 2)
        },
        SiteRule {
            member: vec![Terrain::Hills, Terrain::Highland, Terrain::Cliff],
            ..rule(Terrain::Hills, STONE, 20, 3, 5, ANY, 12, 12, 2)
        },
        // Rare, finite, remote, and never guaranteed near the landing site. It is the reason to
        // leave. The rarity is the *radius*: one disc of seven hexes, usually clipped to less. A
        // richness gate on top of that read as scarcity on `continental` and as absence on
        // `basin`, whose highland is a tenth of the world — and a material that a preset can
        // simply not hold is not rare, it is missing.
        rule(Terrain::Highland, CRYSTAL, 18, 1, 2, ANY, 10, 10, 2),
        // Copper belongs to rolling ground and iron and coal to the tops, which is what the `Hills`
        // doc comment already promises. The pair above may spill down into hills; copper never
        // climbs.
        rule(Terrain::Hills, COPPER_ORE, 34, 2, 4, 30_000, 18, 8, 3),
        // Limestone is a hill quarry, not cliff scree. It is the binder feedstock, so it has to be
        // a readable site with buildable ground around it rather than a first-belt gift.
        rule(Terrain::Hills, LIMESTONE, 22, 2, 4, 28_000, 16, 8, 3),
        SiteRule {
            member: ore_bands,
            ..rule(Terrain::Hills, COAL, 16, 2, 3, 40_000, 18, 8, 3)
        },
        // A forest: 150–250 units across a large area, renewable through the `regrowth_ticks` the
        // item already carries, with a soft edge. Three per cell is a rate change as well as a
        // shape change — a base extractor drains its seven hexes and then runs at whatever regrowth
        // supplies — which is why forestry is a question of area rather than of throughput.
        rule(Terrain::Lowland, WOOD, 30, 5, 6, ANY, 3, 1, 2),
        rule(Terrain::Hills, WOOD, 18, 4, 6, ANY, 3, 1, 2),
        rule(Terrain::Lowland, CRUDE_OIL, 8, 2, 3, ANY, 40, 20, 4),
        rule(Terrain::Hills, CRUDE_OIL, 10, 2, 3, ANY, 40, 20, 4),
        // Riverbanks and lake shores. Rivers are what make this common rather than decorative,
        // which is why the two ship together. Shore-centred clay is the lighter of the two: the
        // sandy-looking tiles are the shore band, and sand has to be what you find on them first.
        SiteRule {
            member: vec![Terrain::Lowland, Terrain::Shore],
            member_water_within: 2,
            ..rule(Terrain::Lowland, CLAY, 24, 2, 3, ANY, 14, 14, 3)
        },
        rule(Terrain::Hills, CLAY, 12, 2, 3, ANY, 14, 14, 3),
        SiteRule {
            member: vec![Terrain::Lowland, Terrain::Shore],
            member_water_within: 2,
            ..rule(Terrain::Shore, CLAY, 16, 2, 3, ANY, 14, 14, 3)
        },
        // Sand sits on the shore band, clipped to it so a beach is a strip rather than a blob.
        // Any shore: a lake, a sea, and a pond all look sandy, and a player walking those tiles
        // should find sand. The ocean proxy used to refuse every inland beach.
        SiteRule {
            ..rule(Terrain::Shore, SAND, 40, 3, 5, ANY, 16, 16, 3)
        },
        // The same beach, reached from the land side. A shore band is a thin ribbon — 26 per mille
        // of `highlands` — so a rule that can only start *on* it is a coin flip on how many of a
        // handful of lattice cells happen to land in the ribbon. A centre just inland clips to
        // exactly the same strip; the shore gate keeps a forest cell from spending itself on an
        // empty disc that never reaches a beach.
        SiteRule {
            member: vec![Terrain::Shore],
            center_shore: true,
            ..rule(Terrain::Lowland, SAND, 26, 3, 5, ANY, 16, 16, 3)
        },
    ]
}

/// The opening a new world guarantees: a material, and the window its patch must fall in.
///
/// This replaced `LANDING_FIELD`, a hardcoded list of eight single cells — one of every material —
/// sitting inside the clearing. That constant, and not the generator, is why every material used
/// to be visible in the first minute; it was the sample platter the roadmap decision named.
///
/// A window is a distance from the landing site to the **nearest hex of the patch**, so it is what
/// the player actually walks, and its floor is what keeps a guaranteed disc from reaching inside
/// the clearing whose field suppression stays exactly as it was. Sand is not guaranteed by
/// distance — the ocean gate decides where a coast is — and crystal is never guaranteed at all.
const BOOTSTRAP_GUARANTEES: [(ItemId, i32, i32); 7] = [
    // The first extractor and the first thing a player walks into, both in sight of the hub.
    // Distances are hexes on the 25 m² lattice (~5.37 m), so 9 hexes is a short walk, not a
    // neighbouring tile.
    (IRON_ORE, 9, 24),
    (WOOD, 9, 24),
    // A short walk, chosen rather than stumbled on.
    (COAL, 15, 40),
    (STONE, 15, 40),
    // Carries a river or a shore with it, which is also the first pump site.
    (CLAY, 15, 40),
    // Binder feedstock: past the opening, before the copper expedition.
    (LIMESTONE, 18, 48),
    // The second metal is an expedition, not an errand.
    (COPPER_ORE, 25, 64),
];

/// How far a window is widened, per step and in total, when a seed puts nothing inside it. Past
/// the cap the world is refused rather than papered over: a preset that cannot bootstrap is the
/// failure the survey exists to make visible.
const BOOTSTRAP_WIDEN_STEP: i32 = 12;
const BOOTSTRAP_WIDEN_CAP: i32 = 96;

/// Make one band's deposits commoner and wider.
///
/// A preset that makes a band scarce is not allowed to make the materials in it unfindable as
/// well. `relaxed()` used to buy that by lowering the per-hex gates on the band's rows, and there
/// are no per-hex gates left to lower — a site is gated at its centre and nowhere else. Weight and
/// radius are the direct form of the same compensation, and they are the honest one: `npm run
/// survey` can see a patch that got wider or commoner, and could never see a gate that moved.
fn favoured(
    rules: Vec<SiteRule>,
    terrain: Terrain,
    weight_gain: u32,
    radius_gain: u32,
) -> Vec<SiteRule> {
    rules
        .into_iter()
        .map(|rule| {
            if rule.terrain != terrain || rule.weight == 0 {
                return rule;
            }
            SiteRule {
                weight: rule.weight + weight_gain,
                radius_max: (rule.radius_max + radius_gain).min(MAX_SITE_RADIUS),
                ..rule
            }
        })
        .collect()
}

/// A named parameter set. A preset is what a player picks; the parameter set is what makes a
/// preset a data row — the same relationship the shape grammar has to a building definition. The
/// raw parameters stay exposed behind the preset in the new-world flow, so the usable surface and
/// the maintainable one are the same table read at two depths.
#[derive(Clone, Debug, Serialize)]
struct WorldPreset {
    key: &'static str,
    name: &'static str,
    description: &'static str,
    params: WorldParams,
}

/// The preset a scenario generates under when nothing names another.
const DEFAULT_PRESET_KEY: &str = "continental";

fn world_presets() -> Vec<WorldPreset> {
    vec![
        WorldPreset {
            key: "continental",
            name: "Continental",
            description: "Mixed coasts and inland ranges. The shipped default.",
            params: WorldParams {
                // A hex is 25 m² and the walk is 15 m/s, so a landform of 512 hexes is a three-minute
                // crossing — plains and ranges you travel, not tiles you glance over. Weight 68
                // lets the coarse octave hold a coastline together; the fine octave is local
                // relief, not a second landform scale.
                elevation_coarse_cell: 512,
                elevation_fine_cell: 10,
                elevation_coarse_weight: 68,
                moisture_cell: 96,
                richness_cell: 64,
                water_level: 18_000,
                shore_level: 24_000,
                hills_level: 33_000,
                highland_level: 42_000,
                // Neighbour steps shrink as the cell grows. 2_400 is "sheer" at this fine scale;
                // the shipped 14_000 at cell 8 would never fire.
                cliff_step: 2_400,
                deep_water_moisture: 40_000,
                site_cell: 18,
                site_jitter: 5,
                // Eight hexes thick, about 320 hexes apart: a real river, and still a sparse wall
                // until v0.22 builds a bridge. Density is ~2.5% of walked hexes against the ~3%
                // the one-hex network ran at.
                river_cell: 320,
                river_width: river_width_for(320, 8),
                river_max_elevation: 42_000,
                ocean_level: 16_000,
                site_rules: default_site_rules(),
            },
        },
        WorldPreset {
            key: "archipelago",
            name: "Archipelago",
            description: "Small islands in scattered water. Short coasts, long walks.",
            params: WorldParams {
                // Islands you walk across, not tiles you step over: ~690 m / 45 s at 15 m/s, still
                // the small end of the four. Weight 60 holds a shore together at this cell without
                // turning the preset into one continent.
                elevation_coarse_cell: 128,
                elevation_fine_cell: 6,
                elevation_coarse_weight: 60,
                moisture_cell: 48,
                richness_cell: 40,
                water_level: 26_000,
                shore_level: 31_000,
                hills_level: 38_000,
                // 46_000 left almost no highland in the opening: cell 8 / 3 with a 26_000 sea
                // cut spends its top on a thin cap, and iron and stone both start on it. 42_000
                // is the same cap continental uses, so an island still has a top and a default
                // extractor can still be stood on it.
                highland_level: 42_000,
                // Broken ground is steep ground: the step that means "sheer" has to scale with
                // the gradient the feature scale produces.
                cliff_step: 4_200,
                deep_water_moisture: 44_000,
                site_cell: 24,
                site_jitter: 4,
                // Scattered water everywhere already; a river network on top of it would leave the
                // walkable ground in shreds.
                river_cell: 80,
                river_width: 0,
                river_max_elevation: 42_000,
                ocean_level: 26_000,
                // Every band here is scarce or shredded, so every band compensates in its own rows.
                // The tops survive least, the rolling ground carries the copper nothing else can,
                // and a forest on an island only reaches a workable size if its disc starts wider.
                site_rules: favoured(
                    favoured(
                        favoured(default_site_rules(), Terrain::Highland, 12, 2),
                        Terrain::Hills,
                        8,
                        2,
                    ),
                    Terrain::Lowland,
                    0,
                    2,
                ),
            },
        },
        WorldPreset {
            key: "highlands",
            name: "Highlands",
            description: "High ground and hard rock. Little water, much cliff.",
            params: WorldParams {
                // Ranges you walk: ~690 m / four minutes. The finest cliffs of the four, because
                // this is the hard-rock preset.
                elevation_coarse_cell: 640,
                elevation_fine_cell: 12,
                elevation_coarse_weight: 72,
                moisture_cell: 80,
                richness_cell: 64,
                water_level: 12_000,
                shore_level: 16_000,
                hills_level: 26_000,
                highland_level: 36_000,
                cliff_step: 1_600,
                deep_water_moisture: 38_000,
                site_cell: 20,
                site_jitter: 5,
                // The preset with the least standing water is the one rivers do the most for: they
                // are where its clay, its pumps, and its hydro come from. Ten hexes thick so a
                // highland river reads as a river.
                river_cell: 240,
                river_width: river_width_for(240, 10),
                river_max_elevation: 36_000,
                // The one preset with no ocean at all: 41 bodies in a 27,937-hex sample and the
                // largest of them 46 hexes. A gate its own basins cannot clear does not make its
                // beaches rarer, it deletes sand from the world — so the cut sits where those
                // basins pass it. "Sand sits on the largest water this world has" is the honest
                // reading of the same rule, and the survey prints the body size that says so.
                ocean_level: 22_000,
                // Almost no shore band, so the sand and clay it does hold are common inside it.
                // Lowland is the valley floor and is scarce too: a forest has to start wider or
                // the largest patch cannot fill a deep extractor.
                site_rules: favoured(
                    favoured(default_site_rules(), Terrain::Shore, 40, 2),
                    Terrain::Lowland,
                    0,
                    2,
                ),
            },
        },
        WorldPreset {
            key: "basin",
            name: "Basin",
            description: "Great contiguous seas around broad land. Ocean, not ponds.",
            params: WorldParams {
                // The sea end of the same scale: 960 hexes is a six-minute landform, and a body
                // that spans two of those is an ocean you do not walk around. Weight 82 is what
                // holds a coastline together at this cell.
                elevation_coarse_cell: 960,
                elevation_fine_cell: 16,
                elevation_coarse_weight: 82,
                moisture_cell: 120,
                richness_cell: 72,
                water_level: 22_000,
                shore_level: 27_000,
                hills_level: 36_000,
                highland_level: 45_000,
                cliff_step: 1_000,
                deep_water_moisture: 40_000,
                site_cell: 18,
                site_jitter: 5,
                river_cell: 400,
                river_width: river_width_for(400, 10),
                river_max_elevation: 45_000,
                ocean_level: 22_000,
                site_rules: default_site_rules(),
            },
        },
    ]
}

fn preset_params(key: &str) -> Option<WorldParams> {
    world_presets()
        .into_iter()
        .find(|preset| preset.key == key)
        .map(|preset| preset.params)
}

fn default_world_params() -> WorldParams {
    preset_params(DEFAULT_PRESET_KEY).expect("the default preset is in the table")
}

/// One field of a lattice cell's hash. Four are drawn — two for the centre offset, one for the
/// weighted pick, one for the radius — and they are separate hashes rather than bit slices of one
/// value, so a weight sum that happens to sit near a power of two is not quietly biased. A site
/// cell covers `site_cell²` hexes and the lattice is cached, so this is paid once per deposit
/// rather than once per hex.
fn site_field(hash: u32, index: i32) -> u32 {
    coordinate_hash(hash, index, SITE_FIELD_ROW)
}

fn site_hash(seed: u32, cell: (i32, i32)) -> u32 {
    coordinate_hash(seed ^ SITE_SALT, cell.0, cell.1)
}

/// Where in its own cell a site stands. The jitter is what keeps a world of deposits from reading
/// as a world on a grid.
fn site_center(params: &WorldParams, hash: u32, cell: (i32, i32)) -> (i32, i32) {
    let span = (2 * params.site_jitter + 1) as u32;
    let offset = |index: i32| (site_field(hash, index) % span) as i32 - params.site_jitter;
    (
        cell.0 * params.site_cell + offset(0),
        cell.1 * params.site_cell + offset(1),
    )
}

/// Whether the coarse elevation octave alone dips below `ocean_level` near a centre — the proxy
/// `SiteRule::center_ocean` documents. Coarse-octave water is what makes a body big, so a pond
/// edge, which exists only in the fine octave, fails this and an ocean coast passes.
fn center_on_ocean(
    params: &WorldParams,
    seed: u32,
    center: (i32, i32),
    spine: &GroundSpine,
) -> bool {
    hexes_in_radius(center, OCEAN_PROBE_RADIUS)
        .into_iter()
        .any(|(q, r)| {
            if spine.is_physical() {
                let ground = spine.generated_at(q, r);
                ground.bed.get() <= scale::SEA_LEVEL_QUANTA
                    || ground.hydrology.depth_quanta >= scale::WADE_LIMIT_QUANTA
            } else {
                value_noise(seed, q, r, params.elevation_coarse_cell, 0xA11CE) < params.ocean_level
            }
        })
}

/// Whether the shore band sits next to a centre — the cheap elevation-cut form of "this is a
/// beach site". `terrain_at` would also sample cliffs; a water test would also fire on rivers,
/// which are clay country. Shore is the sandy-looking tiles, and that is the only band asked.
fn center_on_shore(
    params: &WorldParams,
    seed: u32,
    center: (i32, i32),
    spine: &GroundSpine,
) -> bool {
    hexes_in_radius(center, SHORE_PROBE_RADIUS)
        .into_iter()
        .any(|(q, r)| {
            if spine.is_physical() {
                spine.presentation_at(q, r) == Terrain::Shore
            } else {
                let elevation = elevation_at(params, seed, q, r);
                elevation >= params.water_level && elevation < params.shore_level
            }
        })
}

/// The rules a centre is eligible for, and the pick among them. Returns an index into the rule
/// table. `None` means this cell holds no site at all, which is how barren ground stays the common
/// case.
fn eligible_rule(
    params: &WorldParams,
    seed: u32,
    hash: u32,
    center: (i32, i32),
    spine: &GroundSpine,
) -> Option<usize> {
    let band = spine.presentation_at(center.0, center.1);
    let richness = value_noise(
        seed,
        center.0,
        center.1,
        params.richness_cell,
        RICHNESS_OCTAVE,
    );
    let mut ocean: Option<bool> = None;
    let mut shore: Option<bool> = None;
    let mut admits = |rule: &SiteRule| {
        if rule.weight == 0 || rule.terrain != band || richness <= rule.site_min {
            return false;
        }
        if rule.center_ocean {
            // Asked at most once per cell, and only for a rule that got this far.
            return *ocean.get_or_insert_with(|| center_on_ocean(params, seed, center, spine));
        }
        if rule.center_shore {
            return *shore.get_or_insert_with(|| center_on_shore(params, seed, center, spine));
        }
        true
    };
    let mut total = 0u32;
    for rule in &params.site_rules {
        if admits(rule) {
            total += rule.weight;
        }
    }
    if total == 0 {
        return None;
    }
    let mut pick = site_field(hash, 2) % total;
    for (index, rule) in params.site_rules.iter().enumerate() {
        if !admits(rule) {
            continue;
        }
        if pick < rule.weight {
            return Some(index);
        }
        pick -= rule.weight;
    }
    None
}

/// The site a lattice cell holds before the bootstrap pass has its say. A pure function of
/// `(params, seed, cell)`, which is exactly what lets the lattice be cached.
fn natural_site(
    params: &WorldParams,
    seed: u32,
    cell: (i32, i32),
    spine: &GroundSpine,
) -> Option<Site> {
    let hash = site_hash(seed, cell);
    let center = site_center(params, hash, cell);
    let index = eligible_rule(params, seed, hash, center, spine)?;
    let rule = &params.site_rules[index];
    let span = rule.radius_max - rule.radius_min + 1;
    Some(Site {
        center,
        rule: index,
        radius: (rule.radius_min + site_field(hash, 3) % span) as i32,
        forced_opening: false,
    })
}

/// Whether a site admits one hex, and how far that hex is from its centre.
///
/// `band` is passed in because every caller has just computed it and a band decision costs seven
/// elevation samples. The member test is the clipping that makes a beach a strip rather than a
/// blob and keeps a scree field against its cliffs.
fn site_covers(
    params: &WorldParams,
    seed: u32,
    site: &Site,
    q: i32,
    r: i32,
    band: Terrain,
    spine: &GroundSpine,
) -> Option<i32> {
    let distance = axial_distance(site.center, (q, r));
    // Smooth noise moves the rim by one hex in either direction. Only the edge can change, so the
    // patch stays connected around its centre instead of becoming per-cell confetti or returning
    // to the mixed-material model the site lattice replaced.
    let shape = value_noise(seed, q, r, SITE_SHAPE_CELL, SITE_SHAPE_OCTAVE);
    let edge_jitter = (shape * 3 / (NOISE_MAX + 1)) - 1;
    if distance > (site.radius + edge_jitter).max(1) {
        return None;
    }
    let rule = &params.site_rules[site.rule];
    let admitted = if spine.is_physical() && site.forced_opening {
        !spine.wet_at(q, r)
    } else if rule.member.is_empty() {
        band == rule.terrain
    } else {
        rule.member.contains(&band)
    };
    if !admitted {
        return None;
    }
    if rule.member_water_within > 0
        && !hexes_in_radius((q, r), rule.member_water_within as i32)
            .into_iter()
            .any(|(cell_q, cell_r)| spine.wet_at(cell_q, cell_r))
    {
        return None;
    }
    Some(distance)
}

/// The guaranteed opening, resolved once from `(params, seed)`.
///
/// Spirals outward over lattice cells in a fixed order and, for each guarantee, claims the first
/// unclaimed cell whose centre band admits that material, whose forced disc lands inside the
/// window, and which actually holds a workable patch once the member test has clipped it. A
/// claimed cell is forced to that rule at `radius_max`.
///
/// Two things make this correct rather than merely deterministic. The window is a floor as well as
/// a ceiling, so a guaranteed disc can never reach inside the clearing. And a window that finds
/// nothing widens in fixed steps to a hard cap and then reports the guarantee as unmet, which
/// `Core::new` refuses the world over — `highlands` has almost no Shore band and is the preset that
/// will find this.
///
/// Derived state on the same terms as the site cache: recomputed from `(params, seed)`, never
/// saved, never hashed. The free function is shared by `Core`, the survey, and the balance report,
/// so a surveyed world and a played world cannot disagree about the opening.
fn bootstrap_sites(
    params: &WorldParams,
    seed: u32,
    spine: &GroundSpine,
) -> (BootstrapTable, Vec<(ItemId, i32)>) {
    let mut claimed: BootstrapTable = BTreeMap::new();
    let mut unmet = Vec::new();
    let cells = bootstrap_cells(params, seed);
    for &(item_id, floor, ceiling) in &BOOTSTRAP_GUARANTEES {
        let mut reach = ceiling;
        let placed = loop {
            let found = cells.iter().find_map(|&(distance, cell, center)| {
                if claimed.contains_key(&cell) {
                    return None;
                }
                let (index, forced_opening) = bootstrap_rule(params, seed, center, item_id, spine)?;
                let site = Site {
                    center,
                    rule: index,
                    radius: params.site_rules[index].radius_max as i32,
                    forced_opening,
                };
                let edge = distance - site.radius;
                if edge < floor || edge > reach {
                    return None;
                }
                (member_hexes(params, seed, &site, spine) >= WORKABLE_PATCH_HEXES)
                    .then_some((cell, site))
            });
            if let Some(found) = found {
                break Some(found);
            }
            if reach >= ceiling + BOOTSTRAP_WIDEN_CAP {
                break None;
            }
            reach += BOOTSTRAP_WIDEN_STEP;
        };
        match placed {
            Some((cell, site)) => {
                claimed.insert(cell, site);
            }
            None => unmet.push((item_id, ceiling + BOOTSTRAP_WIDEN_CAP)),
        }
    }
    (claimed, unmet)
}

/// Every lattice cell the bootstrap pass may claim, nearest centre first.
///
/// The spiral, written as a sort rather than as a ring walk. The order has to be fixed and a
/// hand-rolled ring walk is exactly where that goes wrong; the centre distance is what makes it a
/// spiral, and the cell breaks every tie so nothing is decided by iteration order.
///
/// Shared with the diagnosis below, which is the point of it being a function: what a repair
/// measures has to be the ground the pass actually looked at, not a disc that resembles it.
fn bootstrap_cells(params: &WorldParams, seed: u32) -> Vec<SpiralStep> {
    let furthest = BOOTSTRAP_GUARANTEES
        .iter()
        .map(|&(_, _, ceiling)| ceiling)
        .max()
        .unwrap_or(0)
        + BOOTSTRAP_WIDEN_CAP;
    let span = (furthest + MAX_SITE_RADIUS as i32) / params.site_cell + 2;
    let mut cells: Vec<SpiralStep> = Vec::new();
    for cell_q in -span..=span {
        for cell_r in -span..=span {
            let cell = (cell_q, cell_r);
            let center = site_center(params, site_hash(seed, cell), cell);
            cells.push((axial_distance((0, 0), center), cell, center));
        }
    }
    cells.sort_unstable();
    cells
}

/// The rule a guaranteed cell is forced to: the first row for this material whose band the centre
/// stands in and whose ocean gate it clears. The richness gate is deliberately *not* asked — a
/// guarantee that poor country could veto is not a guarantee.
fn bootstrap_rule(
    params: &WorldParams,
    seed: u32,
    center: (i32, i32),
    item_id: ItemId,
    spine: &GroundSpine,
) -> Option<(usize, bool)> {
    let band = spine.presentation_at(center.0, center.1);
    let exact = params.site_rules.iter().position(|rule| {
        rule.weight > 0
            && rule.item_id == item_id
            && rule.terrain == band
            && (!rule.center_ocean || center_on_ocean(params, seed, center, spine))
            && (!rule.center_shore || center_on_shore(params, seed, center, spine))
    });
    if let Some(index) = exact {
        return Some((index, false));
    }
    if !spine.is_physical() || spine.wet_at(center.0, center.1) {
        return None;
    }
    // The translated physical opening is a valley shelf rather than a miniature sample of every
    // old presentation band. When a shelf does not expose the band a material used to name, force
    // its first authored rule as a dry local outcrop; yield, radius and water-proximity policy
    // still come from that rule.
    params
        .site_rules
        .iter()
        .position(|rule| rule.weight > 0 && rule.item_id == item_id)
        .map(|index| (index, true))
}

/// How many hexes a site actually admits once its member test has clipped the disc. A guarantee
/// that lands a highland rule on a peak with nothing around it is not a guarantee, so the
/// bootstrap pass asks this before it claims a cell.
fn member_hexes(params: &WorldParams, seed: u32, site: &Site, spine: &GroundSpine) -> u32 {
    hexes_in_radius(site.center, site.radius)
        .into_iter()
        .filter(|&(q, r)| {
            !spine.wet_at(q, r)
                && axial_distance((0, 0), (q, r)) > LANDING_CLEAR_RADIUS
                && site_covers(params, seed, site, q, r, spine.presentation_at(q, r), spine)
                    .is_some()
        })
        .count() as u32
}

/// The bands a rule could seat this material's guaranteed centre in.
///
/// The centre's band is what `bootstrap_rule` gates on, so this is the ground a guarantee is
/// actually looking for — not the ground its disc ends up covering, which the member test decides
/// afterwards.
fn bootstrap_bands(params: &WorldParams, item_id: ItemId) -> Vec<Terrain> {
    let mut bands: Vec<Terrain> = params
        .site_rules
        .iter()
        .filter(|rule| rule.weight > 0 && rule.item_id == item_id)
        .map(|rule| rule.terrain)
        .collect();
    bands.sort_unstable();
    bands.dedup();
    bands
}

/// The bands the bootstrap pass could actually stand on, as the set of every lattice centre's band.
///
/// This is what separates the two ways an opening fails. A band that is not in here at all means
/// the world holds no such ground near the landing site and no seed will find any; a band that is
/// in here means the ground exists and the guarantee failed on room, distance, or a patch too
/// small — which is a different sentence and a different fix.
fn bootstrap_band_census(
    params: &WorldParams,
    seed: u32,
    spine: &GroundSpine,
) -> BTreeSet<Terrain> {
    bootstrap_cells(params, seed)
        .iter()
        .map(|&(_, _, center)| spine.presentation_at(center.0, center.1))
        .collect()
}

/// Whether a parameter set opens at this seed, which is the only question a repair candidate is
/// judged on. Every suggestion below is put through it, so nothing is offered on the strength of
/// the reasoning that produced it.
fn bootstraps(params: &WorldParams, seed: u32) -> bool {
    let spine = GroundSpine::physical(params, seed, true);
    bootstrap_sites(params, seed, &spine).1.is_empty()
}
