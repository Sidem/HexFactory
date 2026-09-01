import sceneScale from "../../fixtures/scene-scale.json";

import { GRADE_STEP_HEIGHT } from "./surfaceLook";

/**
 * How the renderer turns native's integer heights into scene heights.
 *
 * The wire carries `height` and `water_depth` as integers in whatever unit the active ground source
 * counts in, and nothing in the payload says which unit that is. `fixtures/scene-scale.json` does,
 * and Rust asserts the file against the source production actually constructs — so the physical
 * activation cannot move native's unit without this file, and this file, moving with it.
 *
 * One scene unit is one hex circumradius, which is what `WORLD_SCALE` divides native world
 * coordinates by, so a millimetre figure converts by the circumradius and nothing else.
 */

/** Scene height of one unit of native's published height field. */
export const HEIGHT_UNIT_HEIGHT =
  sceneScale.height_unit === "quantum"
    ? sceneScale.height_quantum_mm / sceneScale.cell_circumradius_mm
    : // A legacy band step is a presentation step and has never had a metre value. It keeps the one
      // the shipped grade already draws, so a legacy world's relief is exactly what it was.
      GRADE_STEP_HEIGHT;

/** Physical metres represented by one native height unit, derived from the same scale fixture. */
export const HEIGHT_UNIT_METRES =
  (HEIGHT_UNIT_HEIGHT * sceneScale.cell_circumradius_mm) / 1000;

/**
 * The height difference that draws as a vertical face rather than an ordinary blended slope.
 *
 * It is the step the player can climb, because that is the line the drawing is meant to show: a
 * slope you can walk up looks like ground, and anything steeper looks like a wall you have to go
 * around or quarry. Nothing here decides legality — native still answers for that.
 */
export const CLIFF_THRESHOLD = sceneScale.max_walk_step * HEIGHT_UNIT_HEIGHT;

/** Whether native is publishing physical quanta rather than legacy presentation band steps. */
export const HEIGHT_IS_PHYSICAL = sceneScale.height_unit === "quantum";

/**
 * The scene heights the finished ground can reach: the generated bed's own range, opened at both
 * ends by every cut and fill the player is allowed to pay for.
 *
 * It is what the ground source can produce, not what this world happens to contain, so it is a
 * constant rather than something measured off a snapshot. The camera brackets its clip planes and
 * its pick ray with it once, and neither has to be recomputed as the survey grows.
 */
export const RELIEF_FLOOR = sceneScale.relief_min * HEIGHT_UNIT_HEIGHT;
export const RELIEF_CEILING = sceneScale.relief_max * HEIGHT_UNIT_HEIGHT;
/** The tallest difference in height two points in one world can be apart. */
export const RELIEF_SPAN = RELIEF_CEILING - RELIEF_FLOOR;
