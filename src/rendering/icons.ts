/**
 * Stage A item icons. Geometric, original, and sized to a hex cell: the glyph sits in the
 * inner 60% so a neighbouring hex never clips it. Keys match `ItemDefinition.icon`.
 */
export const ITEM_ICON_KEYS = ["ore", "crystal", "component"] as const;
export type ItemIconKey = (typeof ITEM_ICON_KEYS)[number];

const ICONS: Record<ItemIconKey, string> = {
  ore: `<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M12 3.2 19.2 8v8L12 20.8 4.8 16V8L12 3.2Zm0 2.4L7.2 8.7v6.6L12 18.4l4.8-3.1V8.7L12 5.6Z"/><path fill="currentColor" d="M12 8.2 16 10.5v4.6L12 17.4 8 15.1v-4.6L12 8.2Z"/></svg>`,
  crystal: `<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M12 2.4 18.6 9.2 12 21.6 5.4 9.2 12 2.4Zm0 3.3L8.2 9.5h7.6L12 5.7Zm-5.2 5.4 5.2 9.1 5.2-9.1H6.8Z"/></svg>`,
  component: `<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M8.2 4.2h7.6l4 4v7.6l-4 4H8.2l-4-4V8.2l4-4Zm.8 1.8-2.2 2.2v7.6L9 18h6l2.2-2.2V8.2L15 6H9Zm1.5 3.2h6.6v1.6h-6.6V9.2Zm0 3.2h6.6v1.6h-6.6v-1.6Z"/></svg>`,
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
  switch (icon) {
    case "crystal":
      ctx.beginPath();
      ctx.moveTo(0, -glyph * 0.5);
      ctx.lineTo(glyph * 0.32, -glyph * 0.08);
      ctx.lineTo(0, glyph * 0.5);
      ctx.lineTo(-glyph * 0.32, -glyph * 0.08);
      ctx.closePath();
      ctx.fill();
      break;
    case "component":
      ctx.lineWidth = Math.max(1.5, glyph * 0.12);
      ctx.strokeRect(-glyph * 0.32, -glyph * 0.32, glyph * 0.64, glyph * 0.64);
      ctx.fillRect(-glyph * 0.18, -glyph * 0.08, glyph * 0.36, glyph * 0.16);
      break;
    default:
      ctx.beginPath();
      for (let corner = 0; corner < 6; corner += 1) {
        const angle = ((60 * corner - 30) * Math.PI) / 180;
        const px = Math.cos(angle) * glyph * 0.42;
        const py = Math.sin(angle) * glyph * 0.42;
        if (corner === 0) ctx.moveTo(px, py);
        else ctx.lineTo(px, py);
      }
      ctx.closePath();
      ctx.fill();
  }
  ctx.restore();
}
