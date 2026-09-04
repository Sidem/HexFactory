//! Deterministic grade design for bounded earthwork selections.

use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct GradePoint {
    pub cell: (i32, i32),
    pub current: i32,
    pub natural: i32,
}

pub(super) struct GradeTargets {
    pub level: i32,
    pub smooth: BTreeMap<(i32, i32), i32>,
}

impl Core {
    pub(super) fn ground_grade_targets(
        &self,
        edit: &GroundEdit,
        cells: &[(i32, i32)],
        grade_limit: i32,
    ) -> GradeTargets {
        let points: Vec<_> = cells
            .iter()
            .map(|&cell| GradePoint {
                cell,
                current: self.ground_elevation_at(cell.0, cell.1),
                natural: self.generated_ground_at(cell.0, cell.1).bed.get(),
            })
            .collect();
        let anchor = edit.datum.unwrap_or((edit.q, edit.r));
        let first = self.ground_elevation_at(anchor.0, anchor.1);
        let level = match edit.reference {
            GroundReference::First => first,
            GroundReference::Lowest => points.iter().map(|point| point.current).min().unwrap_or(0),
            GroundReference::Highest => points.iter().map(|point| point.current).max().unwrap_or(0),
        };
        let smooth = matches!(edit.action, GroundAction::Smooth)
            .then(|| {
                smooth_grade(
                    &points,
                    (anchor, first),
                    self.walk_step_limit(),
                    grade_limit,
                )
            })
            .unwrap_or_default();
        GradeTargets { level, smooth }
    }
}

/// Smooth a selection into a walkable surface tied to the picked starting altitude.
///
/// The lower envelope is the highest walkable surface that never rises above the current ground.
/// It therefore cuts a ridge before asking for fill. The datum cone then raises only points that
/// cannot be reached from the anchor at the legal walking step. Both surfaces obey the same slope
/// limit, so their maximum does too; already-walkable ground is unchanged.
fn smooth_grade(
    points: &[GradePoint],
    anchor: ((i32, i32), i32),
    walk_step: i32,
    grade_limit: i32,
) -> BTreeMap<(i32, i32), i32> {
    points
        .iter()
        .map(|point| {
            let envelope = points
                .iter()
                .map(|other| other.current + walk_step * axial_distance(point.cell, other.cell))
                .min()
                .unwrap_or(point.current);
            let from_anchor = walk_step * axial_distance(point.cell, anchor.0);
            let target = envelope
                .max(anchor.1 - from_anchor)
                .min(anchor.1 + from_anchor)
                .clamp(point.natural - grade_limit, point.natural + grade_limit);
            (point.cell, target)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smooth_grade_keeps_good_ground_and_repairs_a_ridge_and_pit_from_the_anchor() {
        let cells = [(0, 0), (1, 0), (2, 0)];
        let grade = |heights: [i32; 3]| {
            let points: Vec<_> = cells
                .iter()
                .zip(heights)
                .map(|(&cell, current)| GradePoint {
                    cell,
                    current,
                    natural: 0,
                })
                .collect();
            smooth_grade(&points, ((0, 0), heights[0]), 4, 32)
        };

        assert_eq!(
            grade([0, 4, 8]).values().copied().collect::<Vec<_>>(),
            [0, 4, 8]
        );
        assert_eq!(
            grade([0, 8, 8]).values().copied().collect::<Vec<_>>(),
            [0, 4, 8]
        );
        assert_eq!(
            grade([0, -8, -8]).values().copied().collect::<Vec<_>>(),
            [0, -4, -8]
        );
    }
}
