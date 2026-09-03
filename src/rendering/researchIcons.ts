/** The three emblems a whole skill ladder shares, named so each rank can wear its family's. */
const GLYPH_PACK =
  '<rect x="7" y="8" width="18" height="21" rx="4"/><path d="M12 8V4h8v4M7 15h18M12 20h8v6h-8zM4 12v10m24-10v10"/>';
const GLYPH_REACH =
  '<path d="m5 27 9-22 4 22M9 18h11M10 8h10M16 5l11 6-11 6M3 28h20"/><circle cx="14" cy="5" r="2"/><path d="M25 21v8m-4-4h8"/>';
const GLYPH_PACE =
  '<circle cx="20" cy="6" r="3"/><path d="m20 9-4 6 5 5 1 9M16 15l-5 3m5 2-4 8M20 12l6 3M3 11h6M2 17h5M4 23h4"/>';

/** Original SVG research emblems. Presentation keys never enter saves or native definitions. */
const GLYPHS: Record<string, string> = {
  "subsurface-piping":
    '<path d="M3 17h7l3 7h6l3-7h7M10 17l2-5h8l2 5M13 24v4m6-4v4"/>',
  "petroleum-processing":
    '<path d="M5 28V10a4 4 0 0 1 8 0v18M5 14h8M5 21h8M9 6V2m10 26V17a4 4 0 0 1 8 0v11M13 24h6M3 28h26"/><path d="M23 3c-4 5-4 8 0 8s4-3 0-8Z"/>',
  "asphalt-roads": '<path d="M4 29 10 3h12l6 26M16 5v4m0 4v4m0 4v5M3 29h26"/>',
  "field-logistics":
    '<rect x="3" y="8" width="26" height="16" rx="7"/><circle cx="10" cy="16" r="3"/><circle cx="22" cy="16" r="3"/><path d="M11 4h10m-3-3 3 3-3 3"/>',
  "automated-extraction":
    '<path d="M7 27V5h18v22M7 10h18M16 10v7m-5 0h10l-5 8-5-8ZM3 28h26"/>',
  composition:
    '<path d="m16 3 11 6v13l-11 6-11-6V9L16 3Zm-11 6 11 6 11-6M16 15v13M11 6l11 6"/><path d="M11 20h-3m16-3v5"/>',
  "storage-planning":
    '<path d="M4 10h24v18H4V10Zm-1 0 4-6h18l4 6M12 14h8v5h-8zM4 23h24"/>',
  "material-processing":
    '<path d="M6 28V10l5-6h10l5 6v18M4 28h24M10 11h12M10 28v-9h12v9"/><path d="M14 24c-3-3 1-5 1-7 4 3 6 7 2 9"/>',
  "mechanical-shaping":
    '<circle cx="16" cy="16" r="8"/><circle cx="16" cy="16" r="2"/><path d="m16 3 3 5 6-1-1 6 5 3-5 3 1 6-6-1-3 5-3-5-6 1 1-6-5-3 5-3-1-6 6 1 3-5Z"/>',
  hydrology:
    '<path d="M13 3C9 9 5 13 5 18a8 8 0 0 0 15 4M13 3c3 4 6 8 7 11M10 19a3 3 0 0 0 3 3M19 16h9v11h-9zM23 12v4m-5-4h10"/>',
  "on-site-power": '<path d="m19 2-12 17h8l-2 11 12-18h-8l2-10Z"/>',
  "sited-generation":
    '<path d="m16 15-1 14h4l-2-14M16 13 9 3l-4 3 10 9M17 14l12-2-1-5-11 6M16 16l-7 9 4 3 4-11"/><circle cx="16" cy="14" r="2"/>',
  "steam-works":
    '<path d="M6 27V15h20v12H6Zm-3 0h26M10 15v-4m6 4V9m6 6v-4M10 7c-4-4 4-3 0-7m6 5c-4-4 4-3 0-7m6 9c-4-4 4-3 0-7"/><circle cx="16" cy="21" r="3"/>',
  "corner-transport":
    '<path d="M5 26V11a5 5 0 0 1 5-5h17M12 26V14a1 1 0 0 1 1-1h14M24 3l5 6-5 6M3 22h11M3 17h11M16 4v11M21 4v11"/>',
  "machine-tiers":
    '<path d="M5 28V15h10v13M18 28V6h9v22M3 28h26M8 19h4m-4 4h4M21 10h3m-3 5h3m-3 5h3M4 8l5-5 5 5M9 3v9"/>',
  transmission:
    '<path d="M11 29 16 3l5 26M9 9h14M6 17h20M12 17l8 8m0-8-8 8M5 9v4m22-4v4M3 17v5m26-5v5"/>',
  "grid-engineering":
    '<path d="M8 8h16v16H8zM16 2v6m0 16v6M2 16h6m16 0h6M4 4l4 4m16 16 4 4M4 28l4-4M24 8l4-4"/><path d="m17 11-5 7h4l-1 4 5-7h-4l1-4Z"/>',
  "shallow-crossings":
    '<path d="M3 15c7-12 19-12 26 0M3 19c7-12 19-12 26 0M5 14v10m22-10v10M10 10v6m12-6v6M3 28c3-4 5 4 8 0s5 4 8 0 5 4 10 0"/>',
  "belt-junctions":
    '<path d="M13 29V17L5 9V3m14 26V17l8-8V3M13 17V3m6 14V3M2 6l3-3 3 3m2 0 6-5 6 5m2 0 3-3 3 3"/>',
  "grade-separation":
    '<path d="M3 13h8m10 0h8M3 20h8m10 0h8M12 3v26m8-26v26M12 7h8m-8 18h8M8 10l4 6-4 7m16-13-4 6 4 7"/><path stroke-dasharray="2 3" d="M12 16h8"/>',
  "expanded-pack": GLYPH_PACK,
  "surveyed-construction": GLYPH_REACH,
  "field-survey":
    '<circle cx="13" cy="10" r="3"/><path d="M13 13v3M7 16h12M9 16 5 29m8-13v13m4-13 4 13M3 29h22"/><path d="M21 5a10 10 0 0 1 5 6M20 10a5 5 0 0 1 2 3"/>',
  "open-water-swimming":
    '<path d="M3 22c3-4 6 4 10 0s7 4 11 0 5 0 5 0M3 27c3-4 6 4 10 0s7 4 11 0 5 0 5 0"/><circle cx="11" cy="10" r="3"/><path d="m14 14 6 3 5-5M7 18l7-4 4-7"/>',
  "fired-masonry":
    '<path d="M4 26V12l4-4h4l4 4v14H4Zm12 0V10l4-5h4l4 5v16H16ZM3 26h26M8 16h4m-4 5h4m12-9h4m-4 5h4"/>',
  "travellers-pace": GLYPH_PACE,
  // A ladder's higher ranks wear their first rank's emblem. They are the same upgrade bought
  // again, and three unrelated glyphs in a row would say they were three different ones; the card
  // beside each carries the numeral. The keys still have to be here, one per authored skill.
  "expanded-pack-ii": GLYPH_PACK,
  "expanded-pack-iii": GLYPH_PACK,
  "surveyed-construction-ii": GLYPH_REACH,
  "surveyed-construction-iii": GLYPH_REACH,
  "travellers-pace-ii": GLYPH_PACE,
  "travellers-pace-iii": GLYPH_PACE,
};

export const RESEARCH_ICON_KEYS = Object.keys(GLYPHS);

export function researchIconSvg(key: string): string {
  const glyph =
    GLYPHS[key] ??
    '<path d="m16 3 12 7v12l-12 7-12-7V10L16 3Z"/><path d="M16 10v12m-6-6h12"/>';
  return `<svg viewBox="0 0 32 32" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${glyph}</svg>`;
}

export function researchBranchColor(branch: string): string {
  return (
    (
      {
        woodwork: "#d9b67b",
        masonry: "#d4896a",
        metallurgy: "#becddd",
        manufacturing: "#e3a184",
        logistics: "#88bfff",
        infrastructure: "#b2beca",
        plumbing: "#7cdacb",
        electricity: "#efd27c",
        chemistry: "#c5a5e8",
        carrying: "#c6adeb",
        construction: "#e0b48a",
        surveying: "#8fd4ff",
        mobility: "#79d7d0",
      } as Record<string, string>
    )[branch] ?? "#a9c4c0"
  );
}
