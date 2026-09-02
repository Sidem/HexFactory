fn placed_sort_key(placed: &PlacedBuilding) -> (i32, i32, u16, u8, Option<u16>) {
    (
        placed.q,
        placed.r,
        placed.definition_id,
        placed.orientation,
        placed.recipe_id,
    )
}

fn coordinate_hash(seed: u32, q: i32, r: i32) -> u32 {
    let mut value =
        seed ^ (q as u32).wrapping_mul(0x9e3779b1) ^ (r as u32).wrapping_mul(0x85ebca77);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846ca68b);
    value ^ (value >> 16)
}

fn floor_div(value: i32, divisor: i32) -> i32 {
    value.div_euclid(divisor)
}

fn axial_world(q: i32, r: i32) -> (i32, i32) {
    (q * HEX_X + r * (HEX_X / 2), r * HEX_Y)
}

/// How finely `hex_at_world` divides a hex before it rounds. A twelfth-of-a-thousandth of a hex is
/// far below anything a preview pixel can show, and keeping it a power of two keeps the fixed point
/// exact.
const HEX_SUBDIVISION: i64 = 1 << 12;

/// The hex holding a world point: `axial_world` run backwards, then rounded to the nearest centre.
///
/// Fixed point rather than floating point, for the same reason the generator is integer — this maps
/// a preview pixel onto a hex, and a rounding that differed between two builds would be two
/// different pictures of one parameter set. It is not a checksum input, but it is compared: by a
/// player moving one slider and looking at what changed.
fn hex_at_world(x: i64, y: i64) -> (i32, i32) {
    let r = y * HEX_SUBDIVISION / i64::from(HEX_Y);
    let q = x * HEX_SUBDIVISION / i64::from(HEX_X) - r / 2;
    round_axial(q, r)
}

/// Cube rounding: round all three axes, then rebuild whichever moved furthest from the other two,
/// so the result always satisfies `q + r + s == 0` and is the centre actually nearest the point.
fn round_axial(q: i64, r: i64) -> (i32, i32) {
    let s = -q - r;
    let (rounded_q, rounded_r, rounded_s) = (round_hex(q), round_hex(r), round_hex(s));
    let drift_q = (rounded_q * HEX_SUBDIVISION - q).abs();
    let drift_r = (rounded_r * HEX_SUBDIVISION - r).abs();
    let drift_s = (rounded_s * HEX_SUBDIVISION - s).abs();
    if drift_q > drift_r && drift_q > drift_s {
        ((-rounded_r - rounded_s) as i32, rounded_r as i32)
    } else if drift_r > drift_s {
        (rounded_q as i32, (-rounded_q - rounded_s) as i32)
    } else {
        (rounded_q as i32, rounded_r as i32)
    }
}

/// One subdivided axis to the nearest whole hex, halves away from zero. Written out because Rust's
/// integer division truncates toward zero, which would round the negative half of the map the wrong
/// way and shear the picture across the origin.
fn round_hex(value: i64) -> i64 {
    (value * 2 + HEX_SUBDIVISION * value.signum()) / (HEX_SUBDIVISION * 2)
}

fn world_direction(direction: u8) -> (i16, i16) {
    const WORLD_DIRECTIONS: [(i16, i16); 6] = [
        (1000, 0),
        (500, 866),
        (-500, 866),
        (-1000, 0),
        (-500, -866),
        (500, -866),
    ];
    WORLD_DIRECTIONS[usize::from(direction % 6)]
}

/// True when every cell of a definition's footprint is reachable from its anchor through the six
/// edge steps.
///
/// Asked of the authored offsets only. Rotation by whole sixths is a symmetry of this lattice, so
/// a contiguous footprint stays contiguous at every heading a definition may face, and translation
/// to a placement anchor cannot separate it either. Checking the definition once is therefore the
/// same as checking every placement of it.
fn unique_offsets(
    cells: &[Coordinate],
    label: &str,
    building_id: DefinitionId,
) -> Result<BTreeSet<(i32, i32)>, String> {
    let unique: BTreeSet<_> = cells.iter().map(|cell| (cell.q, cell.r)).collect();
    if unique.len() != cells.len() {
        return Err(format!("building {building_id} has an invalid {label}"));
    }
    Ok(unique)
}

fn footprint_is_contiguous(cells: &BTreeSet<(i32, i32)>) -> bool {
    let mut reached = BTreeSet::from([(0, 0)]);
    let mut frontier = vec![(0, 0)];
    while let Some((q, r)) = frontier.pop() {
        for (dq, dr) in DIRECTIONS {
            let step = (q + dq, r + dr);
            if cells.contains(&step) && reached.insert(step) {
                frontier.push(step);
            }
        }
    }
    reached.len() == cells.len()
}

fn rotate_coordinate(mut coordinate: Coordinate, turns: u8) -> Coordinate {
    for _ in 0..turns % 6 {
        coordinate = Coordinate {
            q: -coordinate.r,
            r: coordinate.q + coordinate.r,
        };
    }
    coordinate
}

/// Split `total` across `weights` so the parts sum to exactly `total`.
///
/// Integer floor first, then the leftover units to the largest fractional remainders, ties broken
/// by position — and callers always pass entities in ascending id order, so the tie-break is a
/// save's own order. Exactness is the point: this is how energy is conserved between what plants
/// produced and what machines banked, with no per-entity remainder to store and no drift to audit.
fn apportion(total: u64, weights: &[u64]) -> Vec<u64> {
    let sum: u64 = weights.iter().sum();
    if sum == 0 || total == 0 {
        return vec![0; weights.len()];
    }
    let mut parts: Vec<u64> = weights
        .iter()
        .map(|&weight| (weight as u128 * total as u128 / sum as u128) as u64)
        .collect();
    let mut leftover = total - parts.iter().sum::<u64>();
    if leftover == 0 {
        return parts;
    }
    let mut order: Vec<usize> = (0..weights.len()).collect();
    order.sort_by_key(|&index| {
        let remainder = (weights[index] as u128 * total as u128) % sum as u128;
        (std::cmp::Reverse(remainder), index)
    });
    for index in order {
        if leftover == 0 {
            break;
        }
        parts[index] += 1;
        leftover -= 1;
    }
    parts
}

fn axial_distance(from: (i32, i32), to: (i32, i32)) -> i32 {
    let dq = to.0 - from.0;
    let dr = to.1 - from.1;
    (dq.abs() + dr.abs() + (dq + dr).abs()) / 2
}

/// The routing orientation that steps from one hex to another in a single transport step, or
/// `None` if no direction connects them.
///
/// Searches `TRANSPORT_DIRECTIONS`, so it answers for the two-row period as well as the six edges.
/// The six come first and keep their indices, so every delta that resolved before resolves to the
/// same number now.
fn step_direction(from: (i32, i32), to: (i32, i32)) -> Option<u8> {
    let delta = (to.0 - from.0, to.1 - from.1);
    TRANSPORT_DIRECTIONS
        .iter()
        .position(|direction| *direction == delta)
        .map(|index| index as u8)
}

/// The cells one drag covers, resolved on the axis the dragged definition builds on.
///
/// The two rules are kept apart rather than merged into one greedy loop over twelve directions,
/// because a unit step almost always closes the distance and a two-row step closes it only from
/// inside a narrow cone — so a single greedy loop would never select north or south at all. The
/// consequence of splitting them is the property that matters most: `hex_line` is untouched, so
/// **every drag that resolved before v0.14 resolves to exactly the same cells now.**
fn line_between(from: (i32, i32), to: (i32, i32), axis: OrientationAxis) -> Vec<(i32, i32)> {
    match axis {
        OrientationAxis::Edge => hex_line(from, to),
        OrientationAxis::Corner => hex_line_corner(from, to),
        OrientationAxis::Any => hex_line_any(from, to),
    }
}

/// The cells one drag covers when the definition may take every heading.
///
/// The greedy rule the two axis-specific resolvers keep apart can finally be merged, because with
/// both periods available the objection that sank a twelve-direction loop no longer holds: an edge
/// step always closes one, so the run can never stall in the way a corner-only greedy can, and a
/// corner step closes two only inside the 30°-of-vertical cone. Taking the largest closure and
/// tie-breaking on the lowest heading therefore selects the two-row period exactly where it is
/// worth taking and the unit period everywhere else — one rule, no tuned constant.
///
/// This is geometry alone. It is what `place_line` and the drag's out-of-range fallback walk;
/// `drag_route` prices the same lattice against the player's inventory and what is actually legal
/// to build, and that — not this — is what a live drag follows.
fn hex_line_any(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    let mut cells = vec![from];
    let mut current = from;
    while current != to && cells.len() < MAX_LINE_CELLS {
        let remaining = axial_distance(current, to);
        let Some((_, &(dq, dr))) = TRANSPORT_DIRECTIONS
            .iter()
            .enumerate()
            .filter_map(|(heading, step)| {
                let closed =
                    remaining - axial_distance((current.0 + step.0, current.1 + step.1), to);
                (closed > 0).then_some((closed, heading, step))
            })
            .max_by_key(|&(closed, heading, _)| (closed, std::cmp::Reverse(heading)))
            .map(|(_, heading, step)| (heading, step))
        else {
            break;
        };
        current = (current.0 + dq, current.1 + dr);
        cells.push(current);
    }
    cells
}

/// The cells one corner-heading drag covers — the explicit rule the two-row period needs.
///
/// A step is taken only when it closes the full two rows it spans. That single condition *is* the
/// angle rule, and it needs no tuned constant to say so: in the hex norm, `(1, -2)` is the sum of
/// `NE` and `NW`, and a sum closes the distance by its whole length exactly when the target lies
/// in the closed cone those two span. That cone is 60° wide and centred on due north — `NE` sits
/// 30° east of vertical and `NW` 30° west of it — so the rule reads, precisely, **within 30° of
/// vertical, use the two-row period**.
///
/// A drag that leaves the cone stops rather than wandering: the run builds the risers it can and
/// the player places the corner themselves, which is the same "build what is legal and say where
/// it stopped" contract `place_line` already keeps for cost and for terrain.
fn hex_line_corner(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    let mut cells = vec![from];
    let mut current = from;
    while current != to && cells.len() < MAX_LINE_CELLS {
        let remaining = axial_distance(current, to);
        // The lexicographic minimum is an explicit tie-break. The exhaustive lattice test below
        // also pins that the shipped rosette presents no ties, but determinism does not rely on it.
        let Some(&(dq, dr)) = TRANSPORT_DIRECTIONS[usize::from(NORTH)..]
            .iter()
            .filter(|(dq, dr)| {
                axial_distance((current.0 + dq, current.1 + dr), to) == remaining - 2
            })
            .min_by_key(|(dq, dr)| (*dq, *dr))
        else {
            break;
        };
        current = (current.0 + dq, current.1 + dr);
        cells.push(current);
    }
    cells
}

/// The cells one drag covers, from `from` through `to` inclusive.
///
/// Each step takes the lowest-numbered of the six directions that moves strictly closer to the
/// target. Once a direction stops closing the distance it never starts again, so a run uses at most
/// two directions and turns exactly once — the fewest turns a belt line between those endpoints can
/// have, and the same path every time. Integer-only and independent of iteration order, so it is
/// safe on a state-affecting path. The result is capped at `MAX_LINE_CELLS`; a longer drag builds
/// as far as the cap and stops.
fn hex_line(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    let mut cells = vec![from];
    let mut current = from;
    while current != to && cells.len() < MAX_LINE_CELLS {
        let remaining = axial_distance(current, to);
        let Some(&(dq, dr)) = DIRECTIONS
            .iter()
            .find(|(dq, dr)| axial_distance((current.0 + dq, current.1 + dr), to) < remaining)
        else {
            break;
        };
        current = (current.0 + dq, current.1 + dr);
        cells.push(current);
    }
    cells
}

fn squared_distance(ax: i32, ay: i32, bx: i32, by: i32) -> i64 {
    let dx = i64::from(ax) - i64::from(bx);
    let dy = i64::from(ay) - i64::from(by);
    dx * dx + dy * dy
}

fn circles_overlap(ax: i32, ay: i32, ar: i32, bx: i32, by: i32, br: i32) -> bool {
    squared_distance(ax, ay, bx, by) < i64::from(ar + br).pow(2)
}

/// Newton's method, in integers. `aim` resolves to a checksum input, so the float square root the
/// same job would normally use is not available: the same aim has to produce the same facing on
/// every platform that runs this core, and `f64::sqrt` is only required to be correctly rounded,
/// not to be the same instruction everywhere.
fn integer_sqrt(value: i64) -> i64 {
    if value <= 0 {
        return 0;
    }
    let mut guess = value;
    let mut next = (guess + 1) / 2;
    while next < guess {
        guess = next;
        next = (guess + value / guess) / 2;
    }
    guess
}

fn resource_snapshot_of(
    key: (i32, i32),
    item_id: ItemId,
    quantity: u32,
    initial_quantity: u32,
) -> ResourceSnapshot {
    let (x, y) = axial_world(key.0, key.1);
    ResourceSnapshot {
        q: key.0,
        r: key.1,
        x,
        y,
        radius: HEX_RADIUS as u32,
        item_id,
        quantity,
        initial_quantity,
    }
}

fn hexes_in_radius(origin: (i32, i32), radius: i32) -> Vec<(i32, i32)> {
    let mut cells = Vec::new();
    for dq in -radius..=radius {
        for dr in -radius..=radius {
            let cell = (origin.0 + dq, origin.1 + dr);
            if axial_distance(origin, cell) <= radius {
                cells.push(cell);
            }
        }
    }
    cells
}

fn hexes_in_chunk(chunk_q: i32, chunk_r: i32, size: i32) -> impl Iterator<Item = (i32, i32)> {
    (0..size).flat_map(move |local_r| {
        (0..size).map(move |local_q| (chunk_q * size + local_q, chunk_r * size + local_r))
    })
}

fn chunk_world_bounds(chunk_q: i32, chunk_r: i32, size: i32) -> (i32, i32, i32) {
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    for (q, r) in [
        (chunk_q * size, chunk_r * size),
        (chunk_q * size + size - 1, chunk_r * size),
        (chunk_q * size, chunk_r * size + size - 1),
        (chunk_q * size + size - 1, chunk_r * size + size - 1),
    ] {
        let (x, y) = axial_world(q, r);
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    let origin_x = min_x - HEX_RADIUS;
    let origin_y = min_y - HEX_RADIUS;
    let width = (max_x + HEX_RADIUS) - origin_x;
    let height = (max_y + HEX_RADIUS) - origin_y;
    (origin_x, origin_y, width.max(height))
}

/// Inverse of `axial_world` with cube rounding, so a world point maps to the hex whose centre is
/// nearest. Integer-only: numerators stay in `HEX_X * HEX_Y` space and rounding picks the cube
/// axis with the largest residual.
fn world_to_axial(x: i32, y: i32) -> (i32, i32) {
    let den = i64::from(HEX_X) * i64::from(HEX_Y);
    let q_num = i64::from(x) * i64::from(HEX_Y) - i64::from(y) * i64::from(HEX_X / 2);
    let r_num = i64::from(y) * i64::from(HEX_X);
    cube_round_num(q_num, r_num, -q_num - r_num, den)
}

fn cube_round_num(q: i64, r: i64, s: i64, den: i64) -> (i32, i32) {
    let rq = div_round(q, den);
    let rr = div_round(r, den);
    let rs = div_round(s, den);
    let dq = (rq * den - q).abs();
    let dr = (rr * den - r).abs();
    let ds = (rs * den - s).abs();
    if dq >= dr && dq >= ds {
        ((-rr - rs) as i32, rr as i32)
    } else if dr >= ds {
        (rq as i32, (-rq - rs) as i32)
    } else {
        (rq as i32, rr as i32)
    }
}

fn div_round(num: i64, den: i64) -> i64 {
    if den == 0 {
        return 0;
    }
    if num >= 0 {
        (num + den / 2) / den
    } else {
        -((-num + den / 2) / den)
    }
}

/// Integer value noise on the axial lattice. Samples a `cell`-sized grid and bilinearly
/// interpolates, so a hex still needs no stored neighbors.
fn value_noise(seed: u32, q: i32, r: i32, cell: i32, octave: u32) -> i32 {
    let cell = cell.max(1);
    let cq = floor_div(q, cell);
    let cr = floor_div(r, cell);
    let fq = q - cq * cell;
    let fr = r - cr * cell;
    let n00 = i32::from((coordinate_hash(seed ^ octave, cq, cr) >> 16) as u16);
    let n10 = i32::from((coordinate_hash(seed ^ octave, cq + 1, cr) >> 16) as u16);
    let n01 = i32::from((coordinate_hash(seed ^ octave, cq, cr + 1) >> 16) as u16);
    let n11 = i32::from((coordinate_hash(seed ^ octave, cq + 1, cr + 1) >> 16) as u16);
    let nx0 = lerp_i32(n00, n10, fq, cell);
    let nx1 = lerp_i32(n01, n11, fq, cell);
    lerp_i32(nx0, nx1, fr, cell)
}

fn lerp_i32(a: i32, b: i32, t: i32, span: i32) -> i32 {
    a + (b - a) * t / span.max(1)
}
