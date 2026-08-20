import directionFixture from "../../fixtures/hex-directions.json";

/** The six adjacency headings followed by the six corner transport headings. */
export const TRANSPORT_DIRECTIONS = directionFixture;

/** First transport heading that is not a neighbouring hex edge. */
export const CORNER_START = 6;

/** Player-facing labels in native orientation-index order. */
export const DIRECTION_NAMES = directionFixture.map(({ name }) => name);
