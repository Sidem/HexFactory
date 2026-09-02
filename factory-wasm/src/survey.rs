use super::*;

/// The shipped catalogue, for material names. The survey reports what a player would read.
const DEFINITIONS: &str = include_str!("../../src/data/definitions.json");

/// How far out a survey samples by default. This is the *opening*: bootstrap windows, purity,
/// and patch statistics all live inside a few dozen hexes of the hub. Landscape claims —
/// oceans, ranges, how long a biome takes to walk — need a radius of a couple of landform
/// cells, which is what `landscape_radius` returns; this number stays small so the gate does
/// not walk a million hexes.
pub const DEFAULT_RADIUS: i32 = 96;

/// A radius that can actually see a landform of this cell size, capped so a 960-cell ocean
/// preset does not walk eleven million hexes on every `npm run survey`.
pub fn landscape_radius(coarse_cell: i32) -> i32 {
    DEFAULT_RADIUS.max((coarse_cell * 3) / 2).min(768)
}

#[derive(Clone, Debug, Serialize)]
pub struct BandCount {
    /// The band's name as a label. A survey is a report, not a wire contract, so this is the
    /// readable spelling rather than the enum the snapshot travels as.
    pub band: String,
    pub hexes: u32,
    /// Share of the sampled disc, in parts per thousand.
    pub per_mille: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct MaterialCount {
    pub item_id: ItemId,
    pub name: String,
    pub cells: u32,
    /// Cells per thousand land hexes. Land, not total, because water holds nothing and a
    /// wetter preset would otherwise look poorer than it plays.
    pub per_mille_land: u32,
    /// Axial distance from the landing site to the nearest generated cell of this material,
    /// and the mean over every cell in the sample. `None` means the sample found none, which
    /// is the failure this tool exists to make visible.
    pub nearest: Option<u32>,
    pub mean_distance: Option<u32>,
}

/// Connected runs of one material, which is what an extractor is actually offered and what the
/// survey has never reported. Totals, densities, and distances all look healthy for a world of
/// scattered single cells, so a generator that mixes two materials under one extractor disc can
/// pass every figure this tool printed before. `purity` is the number Landforms and Fields
/// v0.21 is for; the rest say whether a patch is worth walking to.
#[derive(Clone, Debug, Serialize)]
pub struct PatchCount {
    pub item_id: ItemId,
    pub name: String,
    /// Connected runs of this material inside the sample.
    pub patches: u32,
    /// Hexes the fill visited. This must equal the material's `cells`, and it is carried
    /// rather than inferred so a test can say so — a flood fill that loses or double-counts a
    /// hex would otherwise quietly move every mean below it.
    pub hexes: u32,
    /// Hexes per patch, and the largest single patch, both in hexes.
    pub mean_patch: u32,
    pub largest_patch: u32,
    /// Total units in a patch, averaged over patches. Size alone understates a rich small
    /// deposit and overstates a wide thin one, and yield is what the extractor draws down.
    pub mean_patch_yield: u32,
    /// Axial distance from the landing site to the nearest patch of at least
    /// `WORKABLE_PATCH_HEXES`, which is a different and more useful number than `nearest`: a
    /// lone cell two hexes away is not a deposit an extractor can be stood on.
    pub nearest_workable_patch: Option<u32>,
    /// Share of this material's hexes whose radius-1 disc holds exactly one material, in parts
    /// per thousand. An extractor on a mixed hex covers both and cleanly works neither.
    pub purity_per_mille: u32,
    /// Patches touching the edge of the sample, on the same reasoning as `truncated_bodies`: a
    /// patch the sample cuts off is a floor, not a measurement.
    pub truncated_patches: u32,
    /// The size of the water body nearest each patch, averaged over patches. This is what
    /// verifies the beach proxy: a sand rule asks the coarse elevation octave alone whether a
    /// centre stands against ocean, and the generator may not flood-fill to check. The survey
    /// can, so a small number here means the proxy is wrong. `None` means the sample holds no
    /// water at all.
    pub mean_nearest_body: Option<u32>,
}

/// The running totals a patch flood fill accumulates, before names and means are attached.
#[derive(Clone, Copy, Debug, Default)]
struct PatchTotals {
    patches: u32,
    hexes: u32,
    yield_total: u64,
    largest_patch: u32,
    nearest_workable_patch: Option<u32>,
    pure_hexes: u32,
    truncated_patches: u32,
    nearest_body_total: u64,
    nearest_body_patches: u32,
}

/// Ponds or oceans, counted. This is the measurement the milestone's central claim rests on:
/// sea level decides how *much* water there is, and feature scale decides how *big* it is, so
/// the two are told apart by body size at a fixed `water_level`.
///
/// Rivers are **not** counted here. They read as `ShallowWater` like everything else and are
/// common and linear, so folding them in would quietly stop `largest_body` from meaning ocean.
#[derive(Clone, Debug, Serialize)]
pub struct WaterShape {
    pub water_hexes: u32,
    pub bodies: u32,
    pub largest_body: u32,
    pub mean_body: u32,
    /// Bodies reaching the edge of the sample, whose true size the sample cannot see. A
    /// largest-body figure carrying these is a floor, not a measurement.
    pub truncated_bodies: u32,
}

/// Inland water that is a line rather than a basin, reported on its own for the reason above.
/// Shallow water stops being an accident of sea level once rivers exist and becomes common and
/// linear, which is what makes a bridge a necessity rather than an ornament.
#[derive(Clone, Debug, Serialize)]
pub struct RiverShape {
    pub river_hexes: u32,
    /// Connected runs of river, and the mean length of one in hexes.
    pub runs: u32,
    pub mean_run: u32,
    pub longest_run: u32,
}

/// One guaranteed material of the opening, as the generator actually placed it. The bootstrap
/// pass is a promise rather than geography, so it is reported here instead of being folded
/// into the counts — the same split the clearing already lives under.
#[derive(Clone, Debug, Serialize)]
pub struct BootstrapRow {
    pub item_id: ItemId,
    pub name: String,
    /// Distance from the landing site to the nearest hex of the guaranteed patch, and how many
    /// hexes that patch holds once its member test has clipped it. `None` means the pass gave
    /// up, which is the failure the survey exists to make visible.
    pub edge: Option<u32>,
    pub hexes: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct WorldSurvey {
    pub preset: String,
    pub seed: u32,
    pub radius: i32,
    pub hexes: u32,
    pub land_hexes: u32,
    pub bands: Vec<BandCount>,
    pub materials: Vec<MaterialCount>,
    pub patches: Vec<PatchCount>,
    /// Share of every generated resource hex, of any material, whose radius-1 disc holds
    /// exactly one material. This is the single figure v0.21 is measured against, and it is
    /// reported over the whole sample rather than per material because an extractor does not
    /// care which two materials it straddles.
    pub purity_per_mille: u32,
    pub water: WaterShape,
    pub rivers: RiverShape,
    pub bootstrap: Vec<BootstrapRow>,
}

/// Survey a shipped preset by key.
pub fn survey_preset(key: &str, seed: u32, radius: i32) -> Result<WorldSurvey, String> {
    survey_overridden(key, &[], seed, radius)
}

/// Survey a preset with named scalar parameters replaced. The milestone's whole point is that
/// a world is a parameter set rather than a preset, so the tool that measures one has to be
/// able to measure a set nobody shipped — which is how a preset's numbers get chosen.
pub fn survey_overridden(
    key: &str,
    overrides: &[(String, i32)],
    seed: u32,
    radius: i32,
) -> Result<WorldSurvey, String> {
    let mut params = preset_params(key).ok_or_else(|| format!("unknown world preset {key}"))?;
    let mut label = key.to_string();
    for (name, value) in overrides {
        let slot: &mut i32 = match name.as_str() {
            "elevation_coarse_cell" => &mut params.elevation_coarse_cell,
            "elevation_fine_cell" => &mut params.elevation_fine_cell,
            "elevation_coarse_weight" => &mut params.elevation_coarse_weight,
            "moisture_cell" => &mut params.moisture_cell,
            "richness_cell" => &mut params.richness_cell,
            "water_level" => &mut params.water_level,
            "shore_level" => &mut params.shore_level,
            "hills_level" => &mut params.hills_level,
            "highland_level" => &mut params.highland_level,
            "cliff_step" => &mut params.cliff_step,
            "deep_water_moisture" => &mut params.deep_water_moisture,
            "site_cell" => &mut params.site_cell,
            "site_jitter" => &mut params.site_jitter,
            "river_cell" => &mut params.river_cell,
            "river_width" => &mut params.river_width,
            "river_max_elevation" => &mut params.river_max_elevation,
            "ocean_level" => &mut params.ocean_level,
            other => return Err(format!("unknown world parameter {other}")),
        };
        *slot = *value;
        label.push_str(&format!(" {name}={value}"));
    }
    Ok(run(&label, &params, seed, radius))
}

pub fn preset_keys() -> Vec<String> {
    world_presets()
        .into_iter()
        .map(|preset| preset.key.to_string())
        .collect()
}

/// The shipped landform cell of a preset, so the survey binary can size its disc without
/// generating anything.
pub fn preset_coarse_cell(key: &str) -> Option<i32> {
    preset_params(key).map(|params| params.elevation_coarse_cell)
}

/// The default seed of the shipped `new-game` scenario, so a survey and a played world are
/// talking about the same landscape unless the caller says otherwise.
pub fn default_seed() -> u32 {
    1_213_486_160
}

pub(crate) fn run(label: &str, params: &WorldParams, seed: u32, radius: i32) -> WorldSurvey {
    let definitions: DefinitionsInput =
        serde_json::from_str(DEFINITIONS).expect("shipped definitions parse");
    // The survey and a played world share one evaluator, so a surveyed world and a played one
    // cannot disagree about either the lattice or the opening.
    let spine = GroundSpine::physical(params, seed, true);
    let fields = WorldFields::new(params, seed, &spine);
    let cells: Vec<(i32, i32)> = disc(radius);
    let mut bands: BTreeMap<Terrain, u32> = BTreeMap::new();
    let mut terrain_of: BTreeMap<(i32, i32), Terrain> = BTreeMap::new();
    let mut river_cells: BTreeSet<(i32, i32)> = BTreeSet::new();
    let mut land_hexes = 0u32;
    let mut found: BTreeMap<ItemId, (u32, u32, u32)> = BTreeMap::new();
    let mut field_of: BTreeMap<(i32, i32), (ItemId, u32)> = BTreeMap::new();
    for &(q, r) in &cells {
        let terrain = spine.presentation_at(q, r);
        terrain_of.insert((q, r), terrain);
        *bands.entry(terrain).or_default() += 1;
        if !terrain.is_water() {
            land_hexes += 1;
        } else if is_survey_river(params, seed, q, r) {
            river_cells.insert((q, r));
        }
        if let Some((item_id, quantity)) = surveyed_field(&fields, &spine, q, r) {
            field_of.insert((q, r), (item_id, quantity));
            let distance = axial_distance((0, 0), (q, r)) as u32;
            let entry = found.entry(item_id).or_insert((0, u32::MAX, 0));
            entry.0 += 1;
            entry.1 = entry.1.min(distance);
            entry.2 += distance;
        }
    }
    let hexes = cells.len() as u32;
    let bands = bands
        .into_iter()
        .map(|(terrain, count)| BandCount {
            band: format!("{terrain:?}"),
            hexes: count,
            per_mille: per_mille(count, hexes),
        })
        .collect();
    let (water, body_of) = water_shape(&terrain_of, &river_cells, radius);
    let (totals, pure_hexes) = patch_shape(&fields, &spine, &field_of, &body_of, radius);
    let name_of = |item_id: ItemId| {
        definitions
            .items
            .iter()
            .find(|item| item.id == item_id)
            .map(|item| item.name.clone())
            .unwrap_or_else(|| format!("item {item_id}"))
    };
    // Every generated item, whether or not this parameter set produced any — a material the
    // table names and the world does not hold is the row a reader most needs to see.
    let mut materials = Vec::new();
    let mut patches = Vec::new();
    for &item_id in &[
        IRON_ORE, CRYSTAL, COPPER_ORE, COAL, STONE, SAND, CLAY, WOOD, LIMESTONE, CRUDE_OIL,
    ] {
        let name = name_of(item_id);
        let totals = totals.get(&item_id).copied().unwrap_or_default();
        patches.push(PatchCount {
            item_id,
            name: name.clone(),
            patches: totals.patches,
            hexes: totals.hexes,
            mean_patch: if totals.patches == 0 {
                0
            } else {
                totals.hexes / totals.patches
            },
            largest_patch: totals.largest_patch,
            mean_patch_yield: if totals.patches == 0 {
                0
            } else {
                (totals.yield_total / u64::from(totals.patches)) as u32
            },
            nearest_workable_patch: totals.nearest_workable_patch,
            purity_per_mille: per_mille(totals.pure_hexes, totals.hexes),
            truncated_patches: totals.truncated_patches,
            mean_nearest_body: (totals.nearest_body_patches > 0).then(|| {
                (totals.nearest_body_total / u64::from(totals.nearest_body_patches)) as u32
            }),
        });
        let stats = found.get(&item_id).copied();
        materials.push(MaterialCount {
            item_id,
            name,
            cells: stats.map(|(count, _, _)| count).unwrap_or(0),
            per_mille_land: per_mille(stats.map(|(count, _, _)| count).unwrap_or(0), land_hexes),
            nearest: stats.map(|(_, nearest, _)| nearest),
            mean_distance: stats.map(|(count, _, total)| total / count.max(1)),
        });
    }
    WorldSurvey {
        preset: label.to_string(),
        seed,
        radius,
        hexes,
        land_hexes,
        bands,
        materials,
        patches,
        purity_per_mille: per_mille(pure_hexes, field_of.len() as u32),
        water,
        rivers: river_shape(&river_cells),
        bootstrap: bootstrap_rows(&fields, &spine, &name_of),
    }
}

/// What the survey counts as a generated cell. The clearing is a promise, not geography, so it
/// is no evidence about what a parameter set generates — `field_at` already suppresses it, and
/// the guaranteed opening is reported on its own in `bootstrap`.
fn surveyed_field(
    fields: &WorldFields,
    spine: &GroundSpine,
    q: i32,
    r: i32,
) -> Option<(ItemId, u32)> {
    fields
        .field_at(q, r, true, spine)
        .map(|field| (field.item_id, field.quantity))
}

/// A river hex, told apart from sea and lake by the test that made it one. Both read as
/// `ShallowWater`, and the whole point of reporting them apart is that a linear inland water
/// and an ocean are different facts about a world.
fn is_survey_river(params: &WorldParams, seed: u32, q: i32, r: i32) -> bool {
    if axial_distance((0, 0), (q, r)) <= LANDING_CLEAR_RADIUS {
        return false;
    }
    let elevation = elevation_at(params, seed, q, r);
    elevation >= params.shore_level && is_river(params, seed, q, r, elevation)
}

/// Connected runs of river, filled over the same six directions everything else here uses.
fn river_shape(river_cells: &BTreeSet<(i32, i32)>) -> RiverShape {
    let mut unvisited = river_cells.clone();
    let river_hexes = unvisited.len() as u32;
    let mut runs = Vec::new();
    while let Some(&start) = unvisited.iter().next() {
        unvisited.remove(&start);
        let mut stack = vec![start];
        let mut length = 0u32;
        while let Some((q, r)) = stack.pop() {
            length += 1;
            for (dq, dr) in DIRECTIONS {
                if unvisited.remove(&(q + dq, r + dr)) {
                    stack.push((q + dq, r + dr));
                }
            }
        }
        runs.push(length);
    }
    RiverShape {
        river_hexes,
        runs: runs.len() as u32,
        mean_run: if runs.is_empty() {
            0
        } else {
            river_hexes / runs.len() as u32
        },
        longest_run: runs.into_iter().max().unwrap_or(0),
    }
}

/// The opening the generator promised, measured rather than assumed: how far the player walks
/// to each guaranteed patch, and how much of it survived the member clipping.
fn bootstrap_rows(
    fields: &WorldFields,
    spine: &GroundSpine,
    name_of: &dyn Fn(ItemId) -> String,
) -> Vec<BootstrapRow> {
    let placed: BTreeMap<ItemId, (u32, u32)> = fields
        .guarantees(spine)
        .into_iter()
        .map(|(item_id, walk, hexes)| (item_id, (walk, hexes)))
        .collect();
    BOOTSTRAP_GUARANTEES
        .iter()
        .map(|&(item_id, _, _)| BootstrapRow {
            item_id,
            name: name_of(item_id),
            edge: placed.get(&item_id).map(|&(walk, _)| walk),
            hexes: placed.get(&item_id).map_or(0, |&(_, hexes)| hexes),
        })
        .collect()
}

/// Patches, flood filled over the six adjacency directions exactly as `water_shape` fills
/// bodies, plus the purity count. Returns the per-material totals and the number of resource
/// hexes of any material that stand in a single-material disc.
///
/// The fill stays inside the sample — a patch reaching the edge is counted as truncated rather
/// than followed out of the disc — but purity reads `surveyed_field` directly, so a hex on the
/// rim is judged against its real neighbours rather than against a sample boundary.
fn patch_shape(
    fields: &WorldFields,
    spine: &GroundSpine,
    field_of: &BTreeMap<(i32, i32), (ItemId, u32)>,
    body_of: &BTreeMap<(i32, i32), u32>,
    radius: i32,
) -> (BTreeMap<ItemId, PatchTotals>, u32) {
    let nearest_body = nearest_body_size(body_of, radius);
    let mut totals: BTreeMap<ItemId, PatchTotals> = BTreeMap::new();
    let mut unvisited: BTreeSet<(i32, i32)> = field_of.keys().copied().collect();
    while let Some(&start) = unvisited.iter().next() {
        let item_id = field_of[&start].0;
        unvisited.remove(&start);
        let mut stack = vec![start];
        let mut hexes = 0u32;
        let mut yield_total = 0u64;
        let mut nearest = u32::MAX;
        let mut touches_edge = false;
        // The body nearest the patch is the body nearest whichever of its hexes is closest to
        // one, which is what the multi-source walk below already answers per hex.
        let mut body: Option<(u32, u32)> = None;
        while let Some((q, r)) = stack.pop() {
            hexes += 1;
            yield_total += u64::from(field_of[&(q, r)].1);
            let distance = axial_distance((0, 0), (q, r));
            nearest = nearest.min(distance as u32);
            if distance >= radius {
                touches_edge = true;
            }
            if let Some(&(reach, size)) = nearest_body.get(&(q, r)) {
                if body.is_none_or(|(best, _)| reach < best) {
                    body = Some((reach, size));
                }
            }
            for (dq, dr) in DIRECTIONS {
                let next = (q + dq, r + dr);
                // The material test comes first: a neighbour of another material must stay
                // unvisited so its own patch is still found.
                if field_of
                    .get(&next)
                    .is_some_and(|&(other, _)| other == item_id)
                    && unvisited.remove(&next)
                {
                    stack.push(next);
                }
            }
        }
        let entry = totals.entry(item_id).or_default();
        entry.patches += 1;
        entry.hexes += hexes;
        entry.yield_total += yield_total;
        entry.largest_patch = entry.largest_patch.max(hexes);
        if touches_edge {
            entry.truncated_patches += 1;
        }
        if let Some((_, size)) = body {
            entry.nearest_body_total += u64::from(size);
            entry.nearest_body_patches += 1;
        }
        if hexes >= WORKABLE_PATCH_HEXES {
            entry.nearest_workable_patch = Some(
                entry
                    .nearest_workable_patch
                    .map_or(nearest, |best| best.min(nearest)),
            );
        }
    }

    let mut pure_hexes = 0u32;
    for (&(q, r), &(item_id, _)) in field_of {
        let mixed = DIRECTIONS.iter().any(|&(dq, dr)| {
            surveyed_field(fields, spine, q + dq, r + dr).is_some_and(|(other, _)| other != item_id)
        });
        if !mixed {
            pure_hexes += 1;
            totals.entry(item_id).or_default().pure_hexes += 1;
        }
    }
    (totals, pure_hexes)
}

/// For every hex in the sample, how far the nearest water body is and how big that body is.
///
/// One multi-source walk out from every body hex at once, rather than a scan per patch: the
/// per-patch form is patches × water hexes and this is hexes × six.
fn nearest_body_size(
    body_of: &BTreeMap<(i32, i32), u32>,
    radius: i32,
) -> BTreeMap<(i32, i32), (u32, u32)> {
    let mut reached: BTreeMap<(i32, i32), (u32, u32)> = body_of
        .iter()
        .map(|(&cell, &size)| (cell, (0, size)))
        .collect();
    let mut frontier: Vec<(i32, i32)> = reached.keys().copied().collect();
    let mut distance = 0u32;
    while !frontier.is_empty() {
        distance += 1;
        let mut next = Vec::new();
        for (q, r) in frontier {
            let size = reached[&(q, r)].1;
            for (dq, dr) in DIRECTIONS {
                let cell = (q + dq, r + dr);
                if axial_distance((0, 0), cell) > radius || reached.contains_key(&cell) {
                    continue;
                }
                reached.insert(cell, (distance, size));
                next.push(cell);
            }
        }
        frontier = next;
    }
    reached
}

/// Connected water bodies inside the sample, by flood fill over the six adjacency directions.
/// Returns the shape and, per body hex, the size of the body it belongs to — which is what
/// verifies the beach proxy the generator is not allowed to measure for itself.
///
/// River hexes are excluded. They read as `ShallowWater` like a lake does, and folding a
/// continent-spanning line into the body fill would join every basin it touches into one and
/// call the result an ocean.
fn water_shape(
    terrain_of: &BTreeMap<(i32, i32), Terrain>,
    river_cells: &BTreeSet<(i32, i32)>,
    radius: i32,
) -> (WaterShape, BTreeMap<(i32, i32), u32>) {
    let mut unvisited: BTreeSet<(i32, i32)> = terrain_of
        .iter()
        .filter(|(cell, terrain)| terrain.is_water() && !river_cells.contains(cell))
        .map(|(&cell, _)| cell)
        .collect();
    let water_hexes = unvisited.len() as u32;
    let mut sizes = Vec::new();
    let mut truncated = 0u32;
    let mut body_of: BTreeMap<(i32, i32), u32> = BTreeMap::new();
    while let Some(&start) = unvisited.iter().next() {
        unvisited.remove(&start);
        let mut stack = vec![start];
        let mut members = Vec::new();
        let mut touches_edge = false;
        while let Some((q, r)) = stack.pop() {
            members.push((q, r));
            if axial_distance((0, 0), (q, r)) >= radius {
                touches_edge = true;
            }
            for (dq, dr) in DIRECTIONS {
                let next = (q + dq, r + dr);
                if unvisited.remove(&next) {
                    stack.push(next);
                }
            }
        }
        if touches_edge {
            truncated += 1;
        }
        let size = members.len() as u32;
        for cell in members {
            body_of.insert(cell, size);
        }
        sizes.push(size);
    }
    let bodies = sizes.len() as u32;
    (
        WaterShape {
            water_hexes,
            bodies,
            largest_body: sizes.iter().copied().max().unwrap_or(0),
            mean_body: if bodies == 0 { 0 } else { water_hexes / bodies },
            truncated_bodies: truncated,
        },
        body_of,
    )
}

fn disc(radius: i32) -> Vec<(i32, i32)> {
    let mut cells = Vec::new();
    for q in -radius..=radius {
        for r in -radius..=radius {
            if axial_distance((0, 0), (q, r)) <= radius {
                cells.push((q, r));
            }
        }
    }
    cells
}

fn per_mille(count: u32, total: u32) -> u32 {
    if total == 0 {
        0
    } else {
        (u64::from(count) * 1000 / u64::from(total)) as u32
    }
}

pub fn format_json(survey: &WorldSurvey) -> String {
    serde_json::to_string_pretty(survey).expect("survey serializes")
}

/// The human-readable form the notes are written from.
pub fn format_report(survey: &WorldSurvey) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "preset {} | seed {} | radius {} | {} hexes ({} land)\n",
        survey.preset, survey.seed, survey.radius, survey.hexes, survey.land_hexes
    ));
    out.push_str("  band            hexes    per mille\n");
    for band in &survey.bands {
        out.push_str(&format!(
            "  {:<14} {:>7}   {:>6}\n",
            band.band, band.hexes, band.per_mille
        ));
    }
    out.push_str("  material        cells  per mille land   nearest    mean\n");
    for material in &survey.materials {
        let show = |value: Option<u32>| {
            value.map_or_else(|| "  none".to_string(), |value| format!("{value:>6}"))
        };
        out.push_str(&format!(
            "  {:<14} {:>6}   {:>13}   {}  {}\n",
            material.name,
            material.cells,
            material.per_mille_land,
            show(material.nearest),
            show(material.mean_distance)
        ));
    }
    out.push_str(
        "  material       patches    mean   largest   mean yield   workable   purity   cut   \
             near body\n",
    );
    for patch in &survey.patches {
        let show = |value: Option<u32>| {
            value.map_or_else(|| "  none".to_string(), |value| format!("{value:>6}"))
        };
        out.push_str(&format!(
            "  {:<14} {:>7}  {:>6}   {:>7}   {:>10}     {}   {:>6}  {:>4}   {}\n",
            patch.name,
            patch.patches,
            patch.mean_patch,
            patch.largest_patch,
            patch.mean_patch_yield,
            show(patch.nearest_workable_patch),
            patch.purity_per_mille,
            patch.truncated_patches,
            show(patch.mean_nearest_body)
        ));
    }
    out.push_str(&format!(
        "  purity: {} per mille of resource hexes stand in a single-material disc\n",
        survey.purity_per_mille
    ));
    out.push_str("  guaranteed     walk   hexes\n");
    for row in &survey.bootstrap {
        out.push_str(&format!(
            "  {:<14} {}  {:>6}\n",
            row.name,
            row.edge
                .map_or_else(|| "  none".to_string(), |value| format!("{value:>6}")),
            row.hexes
        ));
    }
    out.push_str(&format!(
        "  water: {} hexes in {} bodies | largest {} | mean {} | {} reach the sample edge\n",
        survey.water.water_hexes,
        survey.water.bodies,
        survey.water.largest_body,
        survey.water.mean_body,
        survey.water.truncated_bodies
    ));
    out.push_str(&format!(
        "  rivers: {} hexes in {} runs | mean {} | longest {}\n",
        survey.rivers.river_hexes,
        survey.rivers.runs,
        survey.rivers.mean_run,
        survey.rivers.longest_run
    ));
    out
}
