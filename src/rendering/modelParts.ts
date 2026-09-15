/** Explicit 3D placement for elementary forms. The legacy stamp grammar remains a 2D consumer. */
export type ModelGeometry =
  | "box"
  | "cylinder"
  | "crate"
  | "crucible"
  | "dome"
  | "cone"
  | "wheel"
  | "arch"
  | "claw";
export type ModelMotion =
  | "wheel"
  | "heat"
  | "tool-left"
  | "tool-right"
  | "stock"
  | "upper-arm"
  | "forearm"
  | "claw"
  | "load"
  | "outlet"
  | "stored-output";
export interface ModelPart {
  geometry: ModelGeometry;
  at: readonly [number, number, number];
  size: readonly [number, number, number];
  color: string;
  turn?: readonly [number, number, number];
  motion?: ModelMotion;
}
