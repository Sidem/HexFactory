import { itemIconSvg } from "../rendering/icons";

export function paintHexFace(
  hex: HTMLElement,
  fill: string,
  stroke: string,
  impassable: boolean,
): void {
  hex.style.setProperty("--band-fill", fill);
  hex.style.setProperty("--band-stroke", stroke);
  hex.classList.toggle("impassable", impassable);
}

export function setMeter(
  row: HTMLElement,
  fill: HTMLElement,
  amount: HTMLElement,
  current: number,
  total: number,
  visible: boolean,
): void {
  row.hidden = !visible;
  if (!visible) return;
  const ratio = total > 0 ? Math.min(1, Math.max(0, current / total)) : 0;
  fill.style.width = `${ratio * 100}%`;
  amount.textContent = `${current} / ${total}`;
}

export function setItemGlyph(
  element: HTMLElement,
  icon: string | undefined,
  color: string | undefined,
): void {
  element.style.setProperty("--item-color", color ?? "transparent");
  element.innerHTML = icon && color ? itemIconSvg(icon, color) : "";
}
