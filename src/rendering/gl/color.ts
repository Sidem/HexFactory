/**
 * CSS hex → unit RGBA. Presentation only: these numbers never enter a checksum or a command.
 *
 * Accepts `#rgb`, `#rrggbb`, and `#rrggbbaa` — the last is what the terrain table uses for a
 * fill that already carries its own alpha.
 */
const CACHE = new Map<string, readonly [number, number, number, number]>();

export function parseRgba(color: string): [number, number, number, number] {
  const hit = CACHE.get(color);
  if (hit) return [hit[0], hit[1], hit[2], hit[3]];
  const hex = color.startsWith("#") ? color.slice(1) : color;
  const expand = hex.length === 3 || hex.length === 4;
  const r = parseInt(expand ? hex[0]! + hex[0] : hex.slice(0, 2), 16) / 255;
  const g = parseInt(expand ? hex[1]! + hex[1] : hex.slice(2, 4), 16) / 255;
  const b = parseInt(expand ? hex[2]! + hex[2] : hex.slice(4, 6), 16) / 255;
  const a =
    hex.length === 4
      ? parseInt(hex[3]! + hex[3], 16) / 255
      : hex.length >= 8
        ? parseInt(hex.slice(6, 8), 16) / 255
        : 1;
  const parsed = [
    Number.isFinite(r) ? r : 0,
    Number.isFinite(g) ? g : 0,
    Number.isFinite(b) ? b : 0,
    Number.isFinite(a) ? a : 1,
  ] as const;
  CACHE.set(color, parsed);
  return [parsed[0], parsed[1], parsed[2], parsed[3]];
}
