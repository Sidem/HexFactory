import { TERRAIN_INFO, TERRAIN_ORDER, terrainAccess } from "../core/terrain";
import { required } from "./dom";

/** Render the static band legend from the same authored table as the world. */
export function renderTerrainLegend(): void {
  const element = required<HTMLDivElement>("terrain-legend");
  for (const terrain of TERRAIN_ORDER) {
    const band = TERRAIN_INFO[terrain];
    const row = document.createElement("div");
    row.setAttribute("role", "listitem");
    const swatch = document.createElement("i");
    swatch.style.setProperty("--band-fill", band.fill);
    swatch.style.setProperty("--band-stroke", band.stroke);
    if (!band.passable) swatch.className = "impassable";
    const name = document.createElement("span");
    name.textContent = band.name;
    const access = document.createElement("small");
    access.textContent = terrainAccess(band);
    if (!band.passable) access.className = "impassable-label";
    row.append(swatch, name, access);
    element.append(row);
  }
}
