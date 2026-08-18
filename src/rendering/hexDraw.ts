import { HEX_DIRECTIONS, type PixelPoint } from "@hexlife/embed/hex";

/** Pointy-top hex outline, matching the lattice the rest of the view is drawn on. */
export function hexPath(
  context: CanvasRenderingContext2D,
  center: PixelPoint,
  size: number,
): void {
  context.beginPath();
  for (let corner = 0; corner < HEX_DIRECTIONS.length; corner += 1) {
    const angle = ((60 * corner - 30) * Math.PI) / 180;
    const x = center.x + size * Math.cos(angle);
    const y = center.y + size * Math.sin(angle);
    if (corner === 0) context.moveTo(x, y);
    else context.lineTo(x, y);
  }
  context.closePath();
}

export function hexCorner(
  center: PixelPoint,
  size: number,
  index: number,
): PixelPoint {
  const angle = ((60 * index - 30) * Math.PI) / 180;
  return {
    x: center.x + size * Math.cos(angle),
    y: center.y + size * Math.sin(angle),
  };
}

export function drawHex(
  context: CanvasRenderingContext2D,
  center: PixelPoint,
  size: number,
  fill: string,
  stroke: string,
  lineWidth = 1,
): void {
  hexPath(context, center, size);
  context.fillStyle = fill;
  context.fill();
  context.strokeStyle = stroke;
  context.lineWidth = lineWidth;
  context.stroke();
}
