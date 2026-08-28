import type { ItemDefinition } from "../core/types";
import { itemIconSvg } from "./icons";

/**
 * The one way an item is ever drawn.
 *
 * Before this, eight places rendered the same conceptual object — an item and a quantity — and no
 * two agreed: three orderings of glyph, name and count, three spellings of the same number, and
 * two places (the contract bill and the request board) where an item had a bare colour swatch and
 * no glyph at all. Those two are precisely where the player is being asked to fetch something
 * specific, and colour alone is not an identity in this catalogue: Iron plate `#c3ced6`, Gravel
 * `#9a9188`, and Concrete `#9aa0a4` are three greys.
 *
 * Three rules this component exists to enforce, and the reason it is a component rather than a
 * convention:
 *
 * - **An item always shows its glyph.** There is no variant that omits it.
 * - **One spelling per meaning.** {@link ItemChipView.count} is an amount and reads `3`;
 *   {@link ItemChipView.progress} is progress toward a known target and reads `3 / 10` with a
 *   meter. `×3` was a third spelling of the first and is gone.
 * - **A shortfall is a state of the chip**, not a class on the card around it — which is what makes
 *   "say what is held against every cost line" one call at four sites instead of four treatments.
 *
 * Variants are modifier classes on one markup (`.named`, `.metered`, `.short`), never a second
 * shape. Chips are created once and patched in place, so a chip inside a list carrying a control
 * satisfies the "patched in place, never rebuilt" invariant by construction.
 */
export interface ItemChipView {
  /** A plain amount. Rendered as `3`. Mutually exclusive with {@link ItemChipView.progress}. */
  count?: number;
  /** Progress toward a known target. Rendered as `3 / 10`. */
  progress?: { have: number; need: number };
  /** Draw the bar under a {@link ItemChipView.progress} number. Ignored without one. */
  meter?: boolean;
  /** Show the item's name. Off for dense rows where the glyph and a tooltip carry it. */
  named?: boolean;
  /** The compact form: smaller glyph and tighter spacing, for a run of chips on one line. */
  short?: boolean;
  /**
   * How many units the player is missing, when this chip is a thing they are being asked to
   * supply. Anything above zero marks the chip short and appends the amount to its label.
   */
  shortfall?: number;
}

export function createItemChip(): HTMLElement {
  const chip = document.createElement("span");
  chip.className = "item-chip";
  chip.innerHTML =
    '<span class="item-chip-glyph"></span><strong class="item-chip-name"></strong><span class="item-chip-count"></span><small class="item-chip-short"></small><i class="item-chip-meter"><b></b></i>';
  return chip;
}

function within(chip: HTMLElement, selector: string): HTMLElement {
  const element = chip.querySelector<HTMLElement>(selector);
  if (!element) throw new Error(`Item chip is missing ${selector}`);
  return element;
}

/**
 * Patch one chip to describe `item`. Passing `undefined` draws the fallback identity rather than
 * an empty box, because a definition the host cannot find is a bug worth seeing on screen.
 */
export function fillItemChip(
  chip: HTMLElement,
  item: ItemDefinition | undefined,
  itemId: number | undefined,
  view: ItemChipView = {},
): void {
  const named = view.named ?? true;
  const shortfall = view.shortfall ?? 0;
  const name =
    item?.name ?? (itemId === undefined ? "Empty" : `Item ${itemId}`);
  chip.classList.toggle("named", named);
  chip.classList.toggle("short", view.short === true);
  const metered = view.progress !== undefined && view.meter === true;
  chip.classList.toggle("metered", metered);
  chip.classList.toggle("empty", item === undefined && itemId === undefined);
  // A metered chip draws the gap as a bar and states both numbers, so marking it short as well
  // would be the same fact three times. The shortfall still reaches the label below.
  chip.classList.toggle("shortfall", shortfall > 0 && !metered);
  // A supply line that is covered is deliberately quiet rather than green: the player only needs
  // to be told about the lines standing between them and the build.
  chip.classList.toggle(
    "covered",
    shortfall === 0 && view.progress !== undefined && !metered,
  );
  if (itemId === undefined) delete chip.dataset.itemId;
  else chip.dataset.itemId = String(itemId);
  chip.style.setProperty("--item-color", item?.color ?? "transparent");

  const glyph = within(chip, ".item-chip-glyph");
  const markup = item ? itemIconSvg(item.icon, item.color) : "";
  // Rewriting identical markup every frame would restart the SVG's own layout for nothing.
  if (glyph.dataset.icon !== (item?.key ?? "")) {
    glyph.innerHTML = markup;
    glyph.dataset.icon = item?.key ?? "";
  }

  const label = within(chip, ".item-chip-name");
  label.hidden = !named;
  label.textContent = named ? name : "";

  const count = within(chip, ".item-chip-count");
  const meter = within(chip, ".item-chip-meter");
  if (view.progress) {
    const { have, need } = view.progress;
    count.hidden = false;
    count.textContent = `${have} / ${need}`;
    meter.hidden = !metered;
    const ratio = need > 0 ? Math.min(1, Math.max(0, have / need)) : 0;
    within(meter, "b").style.width = `${ratio * 100}%`;
  } else {
    meter.hidden = true;
    count.hidden = view.count === undefined;
    count.textContent = view.count === undefined ? "" : String(view.count);
  }

  // The deficit is written out rather than left to be subtracted: "need 2 more" is the fact the
  // player acts on, and it is the whole reason the boolean was not enough.
  const short = within(chip, ".item-chip-short");
  short.hidden = shortfall === 0 || metered;
  short.textContent = short.hidden ? "" : `need ${shortfall}`;

  chip.title = itemTooltip(item, name, view);
  chip.setAttribute("aria-label", chipLabel(name, view));
}

/** Detailed description for hovering over items, chips, and slots. */
export function itemTooltip(
  item: ItemDefinition | undefined,
  name: string,
  view: ItemChipView = {},
): string {
  const shortfall = view.shortfall ?? 0;
  const amount = view.progress
    ? `${view.progress.have} / ${view.progress.need}`
    : view.count === undefined
      ? ""
      : item?.stack_size
        ? `${view.count} / ${item.stack_size}`
        : String(view.count);

  const header = amount ? `${name} (${amount})` : name;
  const lines: string[] = [
    shortfall > 0 ? `${header} · Need ${shortfall} more` : header,
  ];

  if (item?.description) {
    lines.push(item.description);
  }

  const attributes: string[] = [];
  if (item?.fuel_value) {
    attributes.push(`Fuel: ${item.fuel_value}`);
  }
  if (attributes.length > 0) {
    lines.push(attributes.join(" · "));
  }

  return lines.join("\n");
}

/** The same chip in words, for a screen reader and for the tooltip a dense row needs. */
export function chipLabel(name: string, view: ItemChipView): string {
  const shortfall = view.shortfall ?? 0;
  const amount = view.progress
    ? `${view.progress.have} of ${view.progress.need}`
    : view.count === undefined
      ? ""
      : String(view.count);
  const head = amount ? `${name}: ${amount}` : name;
  return shortfall > 0 ? `${head}. Need ${shortfall} more` : head;
}
