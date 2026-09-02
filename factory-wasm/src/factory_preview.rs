//! factory_preview — extracted from lib.rs by scripts/rust-split.mjs.
//! Methods moved verbatim; add the imports the compiler asks for.

use super::*;

impl Factory {
    /// The parameters, the clamped preview size, and the world units one preview pixel covers.
    ///
    /// Shared by both preview exports so the terrain raster and the site overlay are pictures of
    /// one window: two windows a pixel apart would be an overlay that does not line up.
    pub(crate) fn preview_window(
        &self,
        world_params_json: &str,
        width: u32,
        height: u32,
        hexes_across: u32,
    ) -> Result<(WorldParams, i32, i32, i64), String> {
        let params = world_params_from_json(world_params_json)?;
        // The same gate `Core::new` puts a new world through, so the panel cannot draw a set the
        // start button would refuse — and so a slider mid-drag cannot hand the generator a cell
        // size of zero to divide by.
        params.validate(&self.definitions)?;
        let width = width.clamp(1, MAX_PREVIEW_SIDE) as i32;
        let height = height.clamp(1, MAX_PREVIEW_SIDE) as i32;
        let across = i64::from(hexes_across.clamp(1, MAX_PREVIEW_SPAN));
        let step = (across * i64::from(HEX_X) / i64::from(width)).max(1);
        Ok((params, width, height, step))
    }

    /// The terrain raster behind {@link Factory::world_preview_bytes}, failing in `String` so a
    /// native test can drive the refusal as well as the picture.
    pub(crate) fn preview_cells(
        &self,
        world_params_json: &str,
        seed: u32,
        width: u32,
        height: u32,
        hexes_across: u32,
    ) -> Result<Vec<u8>, String> {
        let (params, width, height, step) =
            self.preview_window(world_params_json, width, height, hexes_across)?;
        let mut cells = Vec::with_capacity((width * height) as usize);
        for py in 0..height {
            let y = (i64::from(py) - i64::from(height) / 2) * step;
            for px in 0..width {
                let x = (i64::from(px) - i64::from(width) / 2) * step;
                let (q, r) = hex_at_world(x, y);
                cells.push(terrain_at(&params, seed, q, r, true) as u8);
            }
        }
        Ok(cells)
    }

    /// The deposit overlay behind {@link Factory::world_preview_sites_json}, on the same terms.
    pub(crate) fn preview_sites(
        &self,
        world_params_json: &str,
        seed: u32,
        width: u32,
        height: u32,
        hexes_across: u32,
    ) -> Result<PreviewSites, String> {
        let (params, width, height, step) =
            self.preview_window(world_params_json, width, height, hexes_across)?;
        let spine = GroundSpine::physical(&params, seed, true);
        let fields = WorldFields::new(&params, seed, &spine);
        // The lattice cells the window can see, from the axial extent of its four corners. A site
        // wanders inside its own cell by `site_jitter` and reaches out by `radius_max`, so the
        // range is widened by `reach` — the same derivation `field_at` scans with, for the same
        // reason: a range one cell short drops deposits off the edge of the picture in silence.
        let corners = [
            (0, 0),
            (width - 1, 0),
            (0, height - 1),
            (width - 1, height - 1),
        ];
        let cells: Vec<(i32, i32)> = corners
            .iter()
            .map(|&(px, py)| {
                let x = (i64::from(px) - i64::from(width) / 2) * step;
                let y = (i64::from(py) - i64::from(height) / 2) * step;
                let (q, r) = hex_at_world(x, y);
                (
                    floor_div(q, params.site_cell),
                    floor_div(r, params.site_cell),
                )
            })
            .collect();
        let min_q = cells.iter().map(|cell| cell.0).min().unwrap_or(0) - fields.reach - 1;
        let max_q = cells.iter().map(|cell| cell.0).max().unwrap_or(0) + fields.reach + 1;
        let min_r = cells.iter().map(|cell| cell.1).min().unwrap_or(0) - fields.reach - 1;
        let max_r = cells.iter().map(|cell| cell.1).max().unwrap_or(0) + fields.reach + 1;
        let unmet: Vec<ItemId> = fields.unmet.iter().map(|&(item_id, _)| item_id).collect();
        let (needs, repair) = self.preview_diagnosis(&params, seed, &unmet);
        // The bootstrap verdict does not depend on the scan, so a window too wide to walk still
        // reports whether the world can be started at all — and how to fix it.
        if i64::from(max_q - min_q + 1) * i64::from(max_r - min_r + 1) > MAX_PREVIEW_SITE_CELLS {
            return Ok(PreviewSites {
                sites: Vec::new(),
                total: 0,
                dense: true,
                unmet,
                needs,
                repair,
            });
        }
        let mut sites = Vec::new();
        for cell_q in min_q..=max_q {
            for cell_r in min_r..=max_r {
                let Some(site) = fields.site_at((cell_q, cell_r), &spine) else {
                    continue;
                };
                let (x, y) = axial_world(site.center.0, site.center.1);
                sites.push(PreviewSite {
                    item_id: params.site_rules[site.rule].item_id,
                    x: (i64::from(x) / step + i64::from(width) / 2) as i32,
                    y: (i64::from(y) / step + i64::from(height) / 2) as i32,
                    // Hexes to pixels through the same step, so a patch covering a tenth of the
                    // window is drawn covering a tenth of the window.
                    radius: (i64::from(site.radius) * i64::from(HEX_X) / step).max(1) as i32,
                });
            }
        }
        let total = sites.len() as u32;
        let dense = sites.len() > MAX_PREVIEW_SITES;
        if dense {
            sites.clear();
        }
        Ok(PreviewSites {
            sites,
            total,
            dense,
            unmet,
            needs,
            repair,
        })
    }

    /// Why a world was refused, and a way out of it, or nothing at all when it was not refused.
    ///
    /// Both halves are searched here rather than by the host because both are answers about the
    /// generator: the bands come from this world's own rules, and every repair offered has been put
    /// through a real bootstrap pass. Nothing is proposed on the strength of the reasoning that
    /// produced it.
    ///
    /// The cost is paid only by a world that already cannot be started, so a parameter set that
    /// opens previews at the price it always did.
    pub(crate) fn preview_diagnosis(
        &self,
        params: &WorldParams,
        seed: u32,
        unmet: &[ItemId],
    ) -> (Vec<PreviewNeed>, Option<PreviewRepair>) {
        if unmet.is_empty() {
            return (Vec::new(), None);
        }
        let spine = GroundSpine::physical(params, seed, true);
        let census = bootstrap_band_census(params, seed, &spine);
        let needs = unmet
            .iter()
            .map(|&item_id| {
                let bands = bootstrap_bands(params, item_id);
                PreviewNeed {
                    ground: bands.iter().any(|band| census.contains(band)),
                    item_id,
                    bands,
                }
            })
            .collect();
        let repair = PreviewRepair {
            seed: repair_seed(params, seed),
            changes: repair_params(params, seed)
                .map(|fixed| world_changes(params, &fixed))
                .unwrap_or_default(),
        };
        // A repair with neither half is not a repair; saying so lets the panel fall back to the
        // hint rather than offering a button that does nothing.
        let repair = (repair.seed.is_some() || !repair.changes.is_empty()).then_some(repair);
        (needs, repair)
    }
}
