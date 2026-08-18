import {
  axialNeighbor,
  axialToPixel,
  type PixelPoint,
} from "@hexlife/embed/hex";

import type {
  BuildingDefinition,
  BuildingKind,
  EntitySnapshot,
  PowerSource,
} from "../core/types";
import { hexPath } from "./hexDraw";

export type SilhouetteKey =
  | BuildingKind
  | "assembly"
  | "smelting"
  | "firing"
  | "cutting"
  | "crushing"
  | PowerSource;

export interface BuildingTrim {
  stroke: string;
  width: number;
}

/**
 * Look from the definition, not from a drawing per id. Composer-kind machines split on
 * `recipe_category`; generators split on `power_source`. A new tier must not need a new function.
 */
export function silhouetteOf(
  kind: BuildingKind,
  recipeCategory?: string | null,
  powerSource?: PowerSource,
): SilhouetteKey {
  if (kind === "composer") {
    if (recipeCategory === "smelting") return "smelting";
    if (recipeCategory === "firing") return "firing";
    if (recipeCategory === "cutting") return "cutting";
    if (recipeCategory === "crushing") return "crushing";
    return "assembly";
  }
  if (kind === "generator") return powerSource ?? "burner";
  return kind;
}

/**
 * Trim reserved for `tier`. v0.14 will pass the real value; until then every building is tier 0
 * so a later upgrade is a data row rather than a new silhouette.
 */
export function trimOf(tier = 0): BuildingTrim {
  if (tier <= 0) return { stroke: "#dce7ef", width: 1.4 };
  if (tier === 1) return { stroke: "#e2c15a", width: 1.85 };
  return { stroke: "#8fd4ff", width: 2.2 };
}

export interface BuildingLookInput {
  building: EntitySnapshot;
  definition?: BuildingDefinition;
  center: PixelPoint;
  size: number;
  color: string;
  now: number;
  reducedMotion: boolean;
  /** Reserved for v0.14. Leave 0 so today's draw matches tomorrow's tier-0 row. */
  tier?: number;
}

export function drawBuildingLook(
  ctx: CanvasRenderingContext2D,
  input: BuildingLookInput,
): void {
  const { building, definition, center, size, color, now, reducedMotion } =
    input;
  const trim = trimOf(input.tier ?? 0);
  const key = silhouetteOf(
    building.kind,
    definition?.recipe_category,
    definition?.power_source,
  );
  const cycle = workCycle(building, now, reducedMotion);
  ctx.save();
  if (building.status === "no power" || building.status === "brownout")
    ctx.globalAlpha = 0.72;
  hexPath(ctx, center, size * 0.8);
  ctx.fillStyle = color;
  ctx.fill();
  ctx.strokeStyle = trim.stroke;
  ctx.lineWidth = trim.width;
  ctx.stroke();
  drawSilhouette(ctx, key, center, size, trim, cycle);
  ctx.restore();
}

export function cargoTravel(
  now: number,
  reducedMotion: boolean,
  buildingId: number,
): number {
  if (reducedMotion) return 0.72;
  return 0.16 + ((now / 820 + buildingId * 0.17) % 0.68);
}

export function workCycle(
  building: EntitySnapshot,
  now: number,
  reducedMotion: boolean,
): number {
  if (building.progress_total > 0 && building.progress > 0)
    return building.progress / building.progress_total;
  if (reducedMotion) return 0;
  if (
    building.status === "extracting" ||
    building.status === "generating" ||
    building.status === "pumping"
  )
    return (now / 700 + building.id * 0.13) % 1;
  return 0;
}

function drawSilhouette(
  ctx: CanvasRenderingContext2D,
  key: SilhouetteKey,
  center: PixelPoint,
  size: number,
  trim: BuildingTrim,
  cycle: number,
): void {
  const x = center.x;
  const y = center.y;
  ctx.strokeStyle = trim.stroke;
  ctx.fillStyle = trim.stroke;
  ctx.lineWidth = Math.max(1.4, size * 0.06);
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  switch (key) {
    case "smelting":
      ctx.fillStyle = `rgba(255, 148, 64, ${0.18 + cycle * 0.45})`;
      ctx.beginPath();
      ctx.arc(x, y + size * 0.06, size * 0.22, 0, Math.PI * 2);
      ctx.fill();
      ctx.strokeStyle = trim.stroke;
      ctx.strokeRect(
        x - size * 0.08,
        y - size * 0.42,
        size * 0.16,
        size * 0.28,
      );
      break;
    case "firing":
      ctx.beginPath();
      ctx.arc(x, y + size * 0.08, size * 0.28, Math.PI, 0);
      ctx.stroke();
      ctx.fillStyle = `rgba(255, 96, 48, ${0.12 + cycle * 0.4})`;
      ctx.beginPath();
      ctx.arc(x, y + size * 0.1, size * 0.14, 0, Math.PI * 2);
      ctx.fill();
      break;
    case "cutting": {
      const angle = -Math.PI / 2 + cycle * Math.PI * 2;
      ctx.beginPath();
      ctx.arc(x, y, size * 0.22, 0, Math.PI * 2);
      ctx.stroke();
      ctx.beginPath();
      ctx.moveTo(x, y);
      ctx.lineTo(
        x + Math.cos(angle) * size * 0.26,
        y + Math.sin(angle) * size * 0.26,
      );
      ctx.stroke();
      break;
    }
    case "crushing": {
      const gap = 0.08 + Math.sin(cycle * Math.PI) * 0.07;
      ctx.beginPath();
      ctx.moveTo(x - size * 0.28, y - size * 0.18);
      ctx.lineTo(x - size * gap, y + size * 0.22);
      ctx.moveTo(x + size * 0.28, y - size * 0.18);
      ctx.lineTo(x + size * gap, y + size * 0.22);
      ctx.stroke();
      break;
    }
    case "assembly":
      ctx.strokeRect(x - size * 0.2, y - size * 0.2, size * 0.4, size * 0.4);
      if (cycle > 0) {
        const spin = cycle * Math.PI * 2;
        ctx.beginPath();
        ctx.arc(
          x + Math.cos(spin) * size * 0.12,
          y + Math.sin(spin) * size * 0.12,
          size * 0.05,
          0,
          Math.PI * 2,
        );
        ctx.fill();
      }
      break;
    case "extractor": {
      const pulse = 1 + cycle * 0.12;
      ctx.beginPath();
      ctx.arc(x, y, size * 0.16 * pulse, 0, Math.PI * 2);
      ctx.stroke();
      ctx.beginPath();
      ctx.moveTo(x, y - size * 0.08);
      ctx.lineTo(x, y + size * 0.28 * pulse);
      ctx.stroke();
      break;
    }
    case "belt":
      break;
    case "container":
      ctx.strokeRect(
        x - size * 0.22,
        y - size * 0.18,
        size * 0.44,
        size * 0.36,
      );
      ctx.beginPath();
      ctx.moveTo(x - size * 0.22, y - size * 0.04);
      ctx.lineTo(x + size * 0.22, y - size * 0.04);
      ctx.stroke();
      break;
    case "consumer":
      ctx.beginPath();
      ctx.moveTo(x - size * 0.22, y - size * 0.16);
      ctx.lineTo(x + size * 0.22, y);
      ctx.lineTo(x - size * 0.22, y + size * 0.16);
      ctx.stroke();
      break;
    case "hub":
      hexPath(ctx, center, size * 0.42);
      ctx.stroke();
      break;
    case "pump":
      ctx.beginPath();
      ctx.arc(x, y - size * 0.04, size * 0.16, 0, Math.PI * 2);
      ctx.stroke();
      ctx.beginPath();
      ctx.moveTo(x, y + size * 0.12);
      ctx.lineTo(x, y + size * 0.3);
      ctx.stroke();
      if (cycle > 0) {
        ctx.globalAlpha = 0.35 + cycle * 0.4;
        ctx.beginPath();
        ctx.arc(
          x,
          y + size * (0.18 + cycle * 0.08),
          size * 0.06,
          0,
          Math.PI * 2,
        );
        ctx.fill();
        ctx.globalAlpha = 1;
      }
      break;
    case "pole":
      ctx.beginPath();
      ctx.moveTo(x, y + size * 0.28);
      ctx.lineTo(x, y - size * 0.32);
      ctx.moveTo(x - size * 0.16, y - size * 0.2);
      ctx.lineTo(x + size * 0.16, y - size * 0.2);
      ctx.stroke();
      break;
    case "burner":
      ctx.strokeRect(
        x - size * 0.18,
        y - size * 0.08,
        size * 0.36,
        size * 0.28,
      );
      ctx.fillStyle = `rgba(255, 160, 48, ${0.15 + cycle * 0.4})`;
      ctx.fillRect(x - size * 0.1, y - size * 0.32, size * 0.2, size * 0.22);
      break;
    case "wind": {
      const spin = cycle * Math.PI * 2;
      ctx.beginPath();
      ctx.moveTo(x, y + size * 0.28);
      ctx.lineTo(x, y - size * 0.08);
      ctx.stroke();
      for (let blade = 0; blade < 3; blade += 1) {
        const angle = spin + (blade * Math.PI * 2) / 3;
        ctx.beginPath();
        ctx.moveTo(x, y - size * 0.08);
        ctx.lineTo(
          x + Math.cos(angle) * size * 0.28,
          y - size * 0.08 + Math.sin(angle) * size * 0.28,
        );
        ctx.stroke();
      }
      break;
    }
    case "hydro": {
      const spin = cycle * Math.PI * 2;
      ctx.beginPath();
      ctx.arc(x, y, size * 0.22, 0, Math.PI * 2);
      ctx.stroke();
      for (let vane = 0; vane < 4; vane += 1) {
        const angle = spin + (vane * Math.PI) / 2;
        ctx.beginPath();
        ctx.moveTo(x, y);
        ctx.lineTo(
          x + Math.cos(angle) * size * 0.22,
          y + Math.sin(angle) * size * 0.22,
        );
        ctx.stroke();
      }
      break;
    }
    case "turbine":
      ctx.beginPath();
      ctx.arc(x, y, size * 0.2, 0, Math.PI * 2);
      ctx.stroke();
      ctx.strokeRect(
        x - size * 0.08,
        y - size * 0.36,
        size * 0.16,
        size * 0.16,
      );
      if (cycle > 0) {
        ctx.fillStyle = `rgba(180, 220, 255, ${0.2 + cycle * 0.35})`;
        ctx.fill();
      }
      break;
    case "boiler":
      ctx.strokeRect(x - size * 0.24, y - size * 0.14, size * 0.48, size * 0.3);
      ctx.beginPath();
      ctx.arc(
        x - size * 0.08,
        y - size * 0.28 - cycle * size * 0.06,
        size * 0.06,
        0,
        Math.PI * 2,
      );
      ctx.arc(
        x + size * 0.1,
        y - size * 0.34 - cycle * size * 0.04,
        size * 0.045,
        0,
        Math.PI * 2,
      );
      ctx.stroke();
      break;
    case "generator":
      ctx.strokeRect(
        x - size * 0.18,
        y - size * 0.18,
        size * 0.36,
        size * 0.36,
      );
      break;
    default:
      break;
  }
}

export function facingTip(
  center: PixelPoint,
  size: number,
  orientation: number,
): PixelPoint {
  const direction = axialNeighbor({ q: 0, r: 0 }, orientation);
  return axialToPixel(direction, size * 0.39, center);
}
