/**
 * Stage A item icons. Geometric, original, and sized to a hex cell: the glyph sits in the
 * inner 60% so a neighbouring hex never clips it. Keys match `ItemDefinition.icon`.
 *
 * The set names material *forms*, not individual items — iron and copper ore share `ore` and
 * differ by their identity colour, as do every plate and every kind of grit. That keeps twenty-odd
 * items legible on a twelve-glyph vocabulary, and it is the rule Stage B's generator inherits.
 */
export const ITEM_ICON_KEYS = [
  "ore",
  "crystal",
  "component",
  "lump",
  "grains",
  "log",
  "droplet",
  "plate",
  "wire",
  "gear",
  "frame",
  "circuit",
  "kit",
] as const;
export type ItemIconKey = (typeof ITEM_ICON_KEYS)[number];

const ICONS: Record<ItemIconKey, string> = {
  ore: `<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M12 3.2 19.2 8v8L12 20.8 4.8 16V8L12 3.2Zm0 2.4L7.2 8.7v6.6L12 18.4l4.8-3.1V8.7L12 5.6Z"/><path fill="currentColor" d="M12 8.2 16 10.5v4.6L12 17.4 8 15.1v-4.6L12 8.2Z"/></svg>`,
  crystal: `<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M12 2.4 18.6 9.2 12 21.6 5.4 9.2 12 2.4Zm0 3.3L8.2 9.5h7.6L12 5.7Zm-5.2 5.4 5.2 9.1 5.2-9.1H6.8Z"/></svg>`,
  component: `<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M8.2 4.2h7.6l4 4v7.6l-4 4H8.2l-4-4V8.2l4-4Zm.8 1.8-2.2 2.2v7.6L9 18h6l2.2-2.2V8.2L15 6H9Zm1.5 3.2h6.6v1.6h-6.6V9.2Zm0 3.2h6.6v1.6h-6.6v-1.6Z"/></svg>`,
  lump: `<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M9.4 3.6 15 5.2l2.8 4.4-1.6 5.2-5.4 2.2-4.6-3.2L5 7.8l4.4-4.2Zm.5 2.5L7.1 8.7l.9 4.4 3.2 2.2 3.7-1.5 1.1-3.6-1.9-3-4.2-1.1Z"/><path fill="currentColor" d="M13.6 16.4 19 18l-1.6 2.8-4.6-1.2.8-3.2Z"/></svg>`,
  grains: `<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="6.6" r="2.5" fill="currentColor"/><circle cx="7" cy="15.4" r="2.5" fill="currentColor"/><circle cx="17" cy="15.4" r="2.5" fill="currentColor"/><circle cx="12" cy="13" r="1.6" fill="currentColor"/></svg>`,
  log: `<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M4 5.6h16v4.2H4V5.6Zm1.8 1.8v.6h12.4v-.6H5.8Z"/><path fill="currentColor" d="M4 11.2h16v4.2H4v-4.2Zm1.8 1.8v.6h12.4V13H5.8Z"/><path fill="currentColor" d="M6.6 16.8h10.8v3H6.6v-3Z"/></svg>`,
  droplet: `<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M12 2.8c3.6 4.4 6 7.8 6 10.6a6 6 0 0 1-12 0c0-2.8 2.4-6.2 6-10.6Zm0 3.4c-2.6 3.4-4.2 5.8-4.2 7.2a4.2 4.2 0 0 0 8.4 0c0-1.4-1.6-3.8-4.2-7.2Z"/></svg>`,
  plate: `<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M3.6 8.4 12 4.2l8.4 4.2L12 12.6 3.6 8.4Zm4.8 0L12 10.2l3.6-1.8L12 6.6 8.4 8.4Z"/><path fill="currentColor" d="M3.6 12.4 12 16.6l8.4-4.2v3L12 19.6l-8.4-4.2v-3Z"/></svg>`,
  wire: `<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" d="M3.4 16.6 8 7.4l4 9.2 4-9.2 2.6 5.2"/></svg>`,
  gear: `<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M10.6 2.4h2.8l.5 2.5 1.7.7 2.1-1.4 2 2-1.4 2.1.7 1.7 2.5.5v2.8l-2.5.5-.7 1.7 1.4 2.1-2 2-2.1-1.4-1.7.7-.5 2.5h-2.8l-.5-2.5-1.7-.7-2.1 1.4-2-2 1.4-2.1-.7-1.7-2.5-.5v-2.8l2.5-.5.7-1.7L4.3 6.2l2-2 2.1 1.4 1.7-.7.5-2.5ZM12 8.6a3.4 3.4 0 1 0 0 6.8 3.4 3.4 0 0 0 0-6.8Z"/></svg>`,
  frame: `<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M3.4 4.2h17.2v15.6H3.4V4.2Zm2 2v11.6h13.2V6.2H5.4Z"/><path fill="currentColor" d="M6.6 7.4h1.6v9.2H6.6V7.4Zm9.2 0h1.6v9.2h-1.6V7.4Zm-8 3.8h8v1.6h-8v-1.6Z"/></svg>`,
  circuit: `<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M5.4 5.4h13.2v13.2H5.4V5.4Zm2 2v9.2h9.2V7.4H7.4Z"/><path fill="currentColor" d="M9.4 9.4h5.2v1.6h-3.6v3.6H9.4V9.4Zm3.6 3.6h1.6v1.6H13v-1.6Z"/><path fill="currentColor" d="M2.6 8.2h2.8v1.4H2.6V8.2Zm0 5.2h2.8v1.4H2.6v-1.4Zm16 -5.2h2.8v1.4h-2.8V8.2Zm0 5.2h2.8v1.4h-2.8v-1.4Z"/></svg>`,
  kit: `<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M2.4 8.4h19.2v7.2H2.4V8.4Zm1.8 1.8v3.6h15.6v-3.6H4.2Z"/><circle cx="7" cy="12" r="1.5" fill="currentColor"/><circle cx="12" cy="12" r="1.5" fill="currentColor"/><circle cx="17" cy="12" r="1.5" fill="currentColor"/></svg>`,
};

export function isItemIconKey(value: string): value is ItemIconKey {
  return (ITEM_ICON_KEYS as readonly string[]).includes(value);
}

export function itemIconSvg(icon: string, color: string): string {
  const markup = isItemIconKey(icon) ? ICONS[icon] : ICONS.ore;
  return markup.replace(
    'viewBox="0 0 24 24"',
    `viewBox="0 0 24 24" style="color:${color}"`,
  );
}

/** Draw the same glyph onto a canvas hex, scaled so it fits the inner 60% of `size`. */
export function drawItemIcon(
  ctx: CanvasRenderingContext2D,
  icon: string,
  color: string,
  x: number,
  y: number,
  size: number,
): void {
  const glyph = size * 0.6;
  ctx.save();
  ctx.translate(x, y);
  ctx.fillStyle = color;
  ctx.strokeStyle = color;
  ctx.lineWidth = Math.max(1.5, glyph * 0.12);
  switch (icon) {
    case "crystal":
      polygon(ctx, [
        [0, -glyph * 0.5],
        [glyph * 0.32, -glyph * 0.08],
        [0, glyph * 0.5],
        [-glyph * 0.32, -glyph * 0.08],
      ]);
      ctx.fill();
      break;
    case "component":
      ctx.strokeRect(-glyph * 0.32, -glyph * 0.32, glyph * 0.64, glyph * 0.64);
      ctx.fillRect(-glyph * 0.18, -glyph * 0.08, glyph * 0.36, glyph * 0.16);
      break;
    case "lump":
      polygon(ctx, [
        [-glyph * 0.34, -glyph * 0.1],
        [-glyph * 0.1, -glyph * 0.38],
        [glyph * 0.26, -glyph * 0.3],
        [glyph * 0.38, glyph * 0.06],
        [glyph * 0.12, glyph * 0.36],
        [-glyph * 0.24, glyph * 0.28],
      ]);
      ctx.fill();
      break;
    case "grains":
      for (const [cx, cy] of [
        [0, -glyph * 0.26],
        [-glyph * 0.28, glyph * 0.22],
        [glyph * 0.28, glyph * 0.22],
      ] as Array<[number, number]>) {
        ctx.beginPath();
        ctx.arc(cx, cy, glyph * 0.17, 0, Math.PI * 2);
        ctx.fill();
      }
      break;
    case "log":
      for (const row of [-0.32, -0.02, 0.28] as const) {
        ctx.fillRect(-glyph * 0.4, glyph * row, glyph * 0.8, glyph * 0.2);
      }
      break;
    case "droplet":
      ctx.beginPath();
      ctx.moveTo(0, -glyph * 0.44);
      ctx.bezierCurveTo(
        glyph * 0.42,
        0,
        glyph * 0.34,
        glyph * 0.44,
        0,
        glyph * 0.44,
      );
      ctx.bezierCurveTo(
        -glyph * 0.34,
        glyph * 0.44,
        -glyph * 0.42,
        0,
        0,
        -glyph * 0.44,
      );
      ctx.fill();
      break;
    case "plate":
      polygon(ctx, [
        [0, -glyph * 0.32],
        [glyph * 0.42, -glyph * 0.08],
        [0, glyph * 0.16],
        [-glyph * 0.42, -glyph * 0.08],
      ]);
      ctx.fill();
      ctx.beginPath();
      ctx.moveTo(-glyph * 0.42, glyph * 0.12);
      ctx.lineTo(0, glyph * 0.36);
      ctx.lineTo(glyph * 0.42, glyph * 0.12);
      ctx.stroke();
      break;
    case "wire":
      ctx.beginPath();
      ctx.moveTo(-glyph * 0.42, glyph * 0.24);
      ctx.lineTo(-glyph * 0.14, -glyph * 0.28);
      ctx.lineTo(glyph * 0.14, glyph * 0.24);
      ctx.lineTo(glyph * 0.42, -glyph * 0.28);
      ctx.stroke();
      break;
    case "gear":
      for (let tooth = 0; tooth < 6; tooth += 1) {
        const angle = (tooth * Math.PI) / 3;
        ctx.save();
        ctx.rotate(angle);
        ctx.fillRect(-glyph * 0.09, -glyph * 0.46, glyph * 0.18, glyph * 0.2);
        ctx.restore();
      }
      ctx.beginPath();
      ctx.arc(0, 0, glyph * 0.3, 0, Math.PI * 2);
      ctx.stroke();
      break;
    case "frame":
      ctx.strokeRect(-glyph * 0.38, -glyph * 0.38, glyph * 0.76, glyph * 0.76);
      ctx.beginPath();
      ctx.moveTo(-glyph * 0.38, glyph * 0.38);
      ctx.lineTo(glyph * 0.38, -glyph * 0.38);
      ctx.stroke();
      break;
    case "circuit":
      ctx.strokeRect(-glyph * 0.34, -glyph * 0.34, glyph * 0.68, glyph * 0.68);
      ctx.beginPath();
      ctx.moveTo(-glyph * 0.16, -glyph * 0.16);
      ctx.lineTo(glyph * 0.16, -glyph * 0.16);
      ctx.lineTo(glyph * 0.16, glyph * 0.16);
      ctx.stroke();
      break;
    case "kit":
      ctx.strokeRect(-glyph * 0.46, -glyph * 0.18, glyph * 0.92, glyph * 0.36);
      for (const cx of [-0.26, 0, 0.26] as const) {
        ctx.beginPath();
        ctx.arc(glyph * cx, 0, glyph * 0.08, 0, Math.PI * 2);
        ctx.fill();
      }
      break;
    default:
      polygon(
        ctx,
        Array.from({ length: 6 }, (_, corner) => {
          const angle = ((60 * corner - 30) * Math.PI) / 180;
          return [
            Math.cos(angle) * glyph * 0.42,
            Math.sin(angle) * glyph * 0.42,
          ] as [number, number];
        }),
      );
      ctx.fill();
  }
  ctx.restore();
}

/** White glyph on a square, for the WebGL icon atlas to tint per item. */
export function bakeItemIcon(icon: string, size = 64): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext("2d");
  if (ctx) drawItemIcon(ctx, icon, "#ffffff", size / 2, size / 2, size);
  return canvas;
}

function polygon(
  ctx: CanvasRenderingContext2D,
  points: Array<[number, number]>,
): void {
  ctx.beginPath();
  points.forEach(([x, y], index) => {
    if (index === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  });
  ctx.closePath();
}
