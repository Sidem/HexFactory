import type { BuildingDefinition } from "../../core/types";
import type { ShapePart } from "../shapeGrammar";
import type { ModelGeometry, ModelMotion, ModelPart } from "../modelParts";

const WOOD = "#aa7442";
const EDGE = "#dfb778";
const METAL = "#597c80";
const DARK = "#29383d";
const FIRE = "#ff9a36";
const CLAY = "#ba704f";
const SIGNAL = "#8adfbb";
type ModelKey =
  | "hub"
  | "container"
  | "burner"
  | "pole"
  | "extractor"
  | "smelting"
  | "manual-workshop"
  | "assembly"
  | "primitive-smelting"
  | "firing";

/** Categories select shared forms. Fluid tanks, husbandry and later tiers keep their own designs. */
export function elementaryModelKey(
  definition?: BuildingDefinition,
): ModelKey | undefined {
  if (!definition || (definition.tier ?? 0) > 0) return undefined;
  const { kind, recipe_category: recipe, source_category: source } = definition;
  if (kind === "hub" || kind === "pole") return kind;
  if (kind === "container" && !definition.accepted_item_ids?.length)
    return "container";
  if (kind === "extractor" && !source) return "extractor";
  if (kind === "generator" && definition.power_source === "burner")
    return "burner";
  if (
    kind === "composer" &&
    (recipe === "smelting" ||
      recipe === "firing" ||
      recipe === "primitive-smelting" ||
      recipe === "manual-workshop" ||
      recipe === "assembly")
  )
    return recipe;
  return undefined;
}

function part(
  geometry: ModelGeometry,
  at: ModelPart["at"],
  size: ModelPart["size"],
  color: string,
  motion?: ModelMotion,
  turn?: ModelPart["turn"],
): ShapePart {
  return {
    part: "chamber",
    x: at[0],
    y: -at[1],
    scale: Math.max(...size) / 2,
    material: "structure",
    phase: motion ? "pulse" : "still",
    ...(motion === "heat" ? { glow: color } : {}),
    model: { geometry, at, size, color, motion, turn },
  };
}

const bench = (): ShapePart[] => [
  part("box", [0, 0.62, 0], [1.3, 0.16, 0.86], WOOD),
  ...[-0.48, 0.48].flatMap((x) =>
    [-0.3, 0.3].map((z) => part("box", [x, 0.27, z], [0.13, 0.54, 0.13], DARK)),
  ),
];

const MODELS: Record<ModelKey, readonly ShapePart[]> = {
  hub: [
    part("box", [0, 0.09, 0], [1.65, 0.18, 1.4], DARK),
    part("arch", [-0.28, 0.64, 0], [0.82, 1.05, 1.24], METAL),
    part("box", [-0.64, 0.55, 0], [0.12, 0.92, 1.24], DARK),
    part("box", [0.4, 0.21, 0], [0.74, 0.06, 0.92], EDGE),
    part("cylinder", [-0.45, 1.38, -0.47], [0.12, 0.6, 0.12], EDGE),
    part("cylinder", [-0.45, 1.75, -0.47], [0.28, 0.24, 0.28], SIGNAL),
  ],
  container: [
    part("crate", [0, 0.42, 0], [1.22, 0.84, 1.1], WOOD),
    part("box", [0, 0.14, 0], [0.96, 0.08, 0.84], DARK),
    part("box", [0, 0.16, 0], [0.9, 0.6, 0.78], EDGE, "stock"),
  ],
  burner: [
    part("box", [-0.12, 0.47, 0], [0.92, 0.94, 0.72], METAL),
    part("box", [0.35, 0.37, 0], [0.03, 0.42, 0.38], DARK),
    part("box", [0.37, 0.36, 0], [0.02, 0.3, 0.26], FIRE, "heat"),
    part("wheel", [0.02, 0.52, 0.46], [0.82, 0.14, 0.82], EDGE, "wheel", [
      Math.PI / 2,
      0,
      0,
    ]),
    part("cylinder", [-0.4, 1.08, -0.18], [0.17, 0.55, 0.17], DARK),
  ],
  pole: [
    part("box", [0, 0.91, 0], [0.15, 1.82, 0.15], WOOD),
    part("box", [0, 1.68, 0], [0.15, 0.14, 1.12], WOOD),
    ...[-0.45, 0.45].map((z) =>
      part("cylinder", [0, 1.82, z], [0.16, 0.2, 0.16], "#d7d8c7"),
    ),
  ],
  extractor: [
    part("crate", [0.12, 0.24, 0], [0.82, 0.48, 0.88], METAL),
    part("cylinder", [-0.32, 0.44, 0], [0.38, 0.88, 0.38], DARK),
    part("box", [0, 0, 0], [0.16, 1, 0.2], EDGE, "upper-arm"),
    part("box", [0, 0, 0], [0.13, 1, 0.16], METAL, "forearm"),
    part("claw", [0, 0, 0], [0.34, 0.32, 0.34], DARK, "claw"),
    part("box", [0, 0, 0], [0.18, 0.18, 0.18], EDGE, "load"),
    part("box", [0, 0, 0], [0.22, 0.22, 0.22], EDGE, "stored-output"),
    part("box", [0, 0, 0], [0.58, 0.08, 0.54], DARK, "outlet"),
  ],
  smelting: [
    part("crucible", [0, 0.63, 0], [1.14, 1.26, 1.14], DARK),
    part("cylinder", [0, 1.06, 0], [0.78, 0.05, 0.78], "#342924"),
    part("cylinder", [0, 1.1, 0], [0.78, 0.05, 0.78], FIRE, "heat"),
    part("box", [0.53, 0.45, 0], [0.27, 0.18, 0.3], METAL),
  ],
  "manual-workshop": [
    ...bench(),
    part("box", [0.31, 0.8, 0.16], [0.18, 0.2, 0.4], METAL),
    part("box", [0.05, 0.8, 0.16], [0.12, 0.2, 0.4], DARK),
    part("cylinder", [0.46, 0.74, 0.16], [0.05, 0.35, 0.05], EDGE, undefined, [
      0,
      0,
      Math.PI / 2,
    ]),
  ],
  assembly: [
    ...bench(),
    part("arch", [-0.25, 1.02, 0], [0.22, 0.78, 1.12], METAL),
    part("box", [0, 0.88, -0.36], [0.24, 0.18, 0.42], EDGE, "tool-left"),
    part("box", [0, 0.88, 0.36], [0.24, 0.18, 0.42], EDGE, "tool-right"),
    part("box", [0, 0.75, 0], [0.3, 0.08, 0.26], METAL),
  ],
  "primitive-smelting": [
    part("cone", [0, 0.47, 0], [1.18, 0.94, 1.18], "#8c8574"),
    part("box", [0.49, 0.29, 0], [0.045, 0.4, 0.4], DARK),
    part("box", [0.515, 0.26, 0], [0.025, 0.25, 0.28], FIRE, "heat"),
  ],
  firing: [
    part("cylinder", [0, 0.1, 0], [1.4, 0.2, 1.4], CLAY),
    part("dome", [0, 0.62, 0], [1.4, 0.9, 1.4], CLAY),
    part("arch", [0.56, 0.38, 0], [0.25, 0.65, 0.58], "#da9569"),
    part("box", [0.68, 0.31, 0], [0.03, 0.42, 0.36], DARK),
    part("box", [0.7, 0.27, 0], [0.02, 0.27, 0.25], FIRE, "heat"),
  ],
};

export function elementaryParts(
  definition: BuildingDefinition | undefined,
  growth = 0,
): readonly ShapePart[] | undefined {
  const key = elementaryModelKey(definition);
  if (!key) return undefined;
  if (key !== "hub" || growth === 0) return MODELS[key];
  return [
    ...MODELS.hub,
    ...Array.from({ length: Math.min(4, growth) }, (_, index) =>
      part(
        "cylinder",
        [-0.45, 1.99 + index * 0.22, -0.47],
        [0.28, 0.16, 0.28],
        SIGNAL,
      ),
    ),
  ];
}

/** Horizontal growth follows the native plot; heights remain comparable to the human worker. */
export function elementaryScale(
  footprintScale: number,
): readonly [number, number] {
  return [1 + (footprintScale - 1) * 0.85, 1 + (footprintScale - 1) * 0.15];
}
