import directionFixture from "../../fixtures/hex-directions.json";

/** The six adjacency headings followed by the six corner transport headings. */
export const TRANSPORT_DIRECTIONS = directionFixture;

/** First transport heading that is not a neighbouring hex edge. */
export const CORNER_START = 6;

/** Player-facing labels in native orientation-index order. */
export const DIRECTION_NAMES = directionFixture.map(({ name }) => name);

/**
 * One press of `R` across both families, in angular order.
 *
 * The table lists the six edges and then the six corners, so stepping its indices would turn a belt
 * through every edge before it reached the first corner — six presses to nudge a heading by 30°.
 * This interleaves them instead, and mirrors `OrientationAxis::next` and `::previous` in the core so
 * the pending building and the placed one turn the same way. `rotationMatchesNativeAngularOrder`
 * pins it against the shared direction fixture's own world vectors rather than against the index
 * arithmetic below, which is the only way the two implementations can be compared at all.
 */
export function rotateAnyOrientation(
  orientation: number,
  step: number,
): number {
  const edge = orientation < CORNER_START;
  const spoke = edge ? orientation : orientation - CORNER_START;
  if (step >= 0)
    return edge ? CORNER_START + ((spoke + 2) % 6) : (spoke + 5) % 6;
  return edge ? CORNER_START + ((spoke + 1) % 6) : (spoke + 4) % 6;
}
