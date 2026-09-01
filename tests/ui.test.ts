import { describe, expect, it } from "vitest";

import { itemTooltip } from "../src/rendering/itemChip";
import { PanelController } from "../src/ui/panels";
import { machineStockSlots } from "../src/ui/stockSlots";
import { ConfirmDialog } from "../src/ui/confirm";

describe("demolition confirmation", () => {
  it("does not let a queued close event cancel a newer question or accept twice", async () => {
    const nodes = new Map(
      ["title", "contents", "note", "accept", "cancel"].map((name) => [
        `#confirm-${name}`,
        Object.assign(new EventTarget(), {
          textContent: "",
          hidden: false,
          focus() {},
          replaceChildren() {},
        }),
      ]),
    );
    const dialog = Object.assign(new EventTarget(), {
      open: false,
      querySelector: (selector: string) => nodes.get(selector),
      showModal() {
        this.open = true;
      },
      close() {
        this.open = false;
        queueMicrotask(() => dialog.dispatchEvent(new Event("close")));
      },
    });
    const view = new ConfirmDialog(dialog as unknown as HTMLDialogElement);
    const request = {
      title: "Demolish?",
      note: "Ground items expire in about a minute.",
      accept: "Demolish",
      cancel: "Keep it",
    };
    const answers: string[] = [];
    view.ask(request, () => answers.push("old"));
    view.ask(request, () => answers.push("new"));
    await Promise.resolve();
    nodes.get("#confirm-accept")!.dispatchEvent(new Event("click"));
    nodes.get("#confirm-accept")!.dispatchEvent(new Event("click"));
    await Promise.resolve();
    expect(answers).toEqual(["new"]);
    view.ask(request, () => answers.push("cancelled"));
    dialog.dispatchEvent(new Event("cancel", { cancelable: true }));
    await Promise.resolve();
    expect(view.open).toBe(false);
    expect(answers).toEqual(["new"]);
  });
});

class FakeClassList {
  private readonly values = new Set<string>();

  constructor(...values: string[]) {
    values.forEach((value) => this.values.add(value));
  }

  contains(value: string): boolean {
    return this.values.has(value);
  }

  add(value: string): void {
    this.values.add(value);
  }

  remove(value: string): void {
    this.values.delete(value);
  }

  toggle(value: string, force?: boolean): boolean {
    const next = force ?? !this.values.has(value);
    if (next) this.values.add(value);
    else this.values.delete(value);
    return next;
  }
}

interface FakeElement {
  id: string;
  tagName: string;
  open: boolean;
  focused: boolean;
  visible: boolean;
  getClientRects(): unknown[];
  listeners: Map<string, (event: { preventDefault(): void }) => void>;
  addEventListener(
    name: string,
    listener: (event: { preventDefault(): void }) => void,
  ): void;
  showModal(): void;
  close(): void;
  focus(): void;
  classList: FakeClassList;
  dataset: Record<string, string>;
  attributes: Map<string, string>;
  setAttribute(name: string, value: string): void;
}

function element(
  id: string,
  classes: string[],
  dataset: Record<string, string> = {},
): FakeElement {
  return {
    id,
    tagName: "ASIDE",
    open: false,
    focused: false,
    visible: true,
    getClientRects() {
      return this.visible ? [{}] : [];
    },
    listeners: new Map(),
    addEventListener(name, listener) {
      this.listeners.set(name, listener);
    },
    showModal() {
      this.open = true;
    },
    close() {
      this.open = false;
    },
    focus() {
      this.focused = true;
    },
    classList: new FakeClassList(...classes),
    dataset,
    attributes: new Map(),
    setAttribute(name, value) {
      this.attributes.set(name, value);
    },
  };
}

function harness(stored = "[]") {
  const left = element("left", ["glass-panel", "open"]);
  const right = element("right", ["glass-panel"]);
  const modal = element("research-panel", ["glass-panel"]);
  modal.tagName = "DIALOG";
  const modalToggle = element("research-toggle", ["panel-toggle"], {
    panelTarget: modal.id,
  });
  const leftToggle = element("left-toggle", ["panel-toggle"], {
    panelTarget: "left",
  });
  const rightToggle = element("right-toggle", ["panel-toggle"], {
    panelTarget: "right",
  });
  const elements = new Map(
    [left, right, modal, modalToggle, leftToggle, rightToggle].map((value) => [
      value.id,
      value,
    ]),
  );
  const root = {
    getElementById(id: string) {
      return elements.get(id) ?? null;
    },
    querySelectorAll(selector: string) {
      if (selector === ".panel-toggle")
        return [leftToggle, rightToggle, modalToggle];
      if (selector === "dialog.glass-panel") return [modal];
      if (selector === ".glass-panel.open")
        return [left, right, modal].filter(({ classList }) =>
          classList.contains("open"),
        );
      return [];
    },
    querySelector(selector: string) {
      if (selector === ".glass-panel.open")
        return (
          [left, right, modal].find(({ classList }) =>
            classList.contains("open"),
          ) ?? null
        );
      return modalToggle;
    },
  };
  const values = new Map([["hexfactory:panels:v1", stored]]);
  const storage = {
    getItem(key: string) {
      return values.get(key) ?? null;
    },
    setItem(key: string, value: string) {
      values.set(key, value);
    },
  };
  return {
    controller: new PanelController(
      root as unknown as Document,
      storage as unknown as Storage,
    ),
    left,
    right,
    leftToggle,
    rightToggle,
    modal,
    modalToggle,
    values,
  };
}

describe("panel controller", () => {
  it("reveals a pack without displacing a workspace the player opened", () => {
    const view = harness();
    view.controller.reveal("right");
    expect(view.controller.isOpen("right")).toBe(false);
    view.controller.close();
    view.controller.reveal("right");
    expect(view.controller.isOpen("right")).toBe(true);
    view.controller.reveal("left");
    expect(view.controller.isOpen("left")).toBe(false);
  });
  it("opens research modally, closes on Escape, and returns focus to a visible opener", () => {
    // A cross-link opener that is no longer on screen hands focus back to the main toggle rather
    // than to an element the player cannot see.
    const crossLinked = harness();
    crossLinked.controller.toggle(
      "research-panel",
      crossLinked.rightToggle as unknown as HTMLElement,
    );
    crossLinked.rightToggle.visible = false;
    crossLinked.controller.close();
    expect(crossLinked.rightToggle.focused).toBe(false);
    expect(crossLinked.modalToggle.focused).toBe(true);

    const view = harness();
    view.controller.bind();
    view.controller.toggle("research-panel");
    expect(view.modal.open).toBe(true);
    expect(view.left.classList.contains("open")).toBe(false);
    expect(view.modalToggle.attributes.get("aria-expanded")).toBe("true");
    let prevented = false;
    view.modal.listeners.get("cancel")!({
      preventDefault: () => {
        prevented = true;
      },
    });
    expect(prevented).toBe(true);
    expect(view.modal.open).toBe(false);
    expect(view.modalToggle.focused).toBe(true);
    expect(view.values.get("hexfactory:panels:v1")).toBe("[]");

    // A stored modal is not reopened over the title screen, and opening a workspace dismisses it.
    const stored = harness('["research-panel"]');
    stored.controller.restore();
    expect(stored.modal.open).toBe(false);
    expect(stored.modal.classList.contains("open")).toBe(false);
    stored.controller.toggle("research-panel");
    stored.controller.toggle("right");
    expect(stored.modal.open).toBe(false);
    expect(stored.right.classList.contains("open")).toBe(true);
  });

  it("restores the last valid workspace exclusively, and persists what it opened", () => {
    const restored = harness('["missing","left","right"]');
    restored.left.classList.remove("open");
    restored.controller.restore();

    expect(restored.left.classList.contains("open")).toBe(false);
    expect(restored.right.classList.contains("open")).toBe(true);
    expect(restored.leftToggle.attributes.get("aria-expanded")).toBe("false");
    expect(restored.rightToggle.attributes.get("aria-expanded")).toBe("true");

    const view = harness();
    view.controller.toggle("right");
    expect(view.left.classList.contains("open")).toBe(false);
    expect(view.right.classList.contains("open")).toBe(true);
    expect(view.values.get("hexfactory:panels:v1")).toBe('["right"]');

    view.controller.close();
    expect(view.right.classList.contains("open")).toBe(false);
    expect(view.values.get("hexfactory:panels:v1")).toBe("[]");
  });

  it("formats detailed hover tooltips with name, count, description, and properties", () => {
    const coal = {
      id: 5,
      key: "coal",
      name: "Coal",
      color: "#000000",
      icon: "lump",
      description: "Dense highland fuel, and the carbon steel is made with.",
      stack_size: 20,
      fuel_value: 160,
    };
    const tooltip = itemTooltip(coal, coal.name, { count: 5 });
    expect(tooltip).toBe(
      "Coal (5 / 20)\nDense highland fuel, and the carbon steel is made with.\nFuel: 160",
    );

    const plate = {
      id: 11,
      key: "iron-plate",
      name: "Iron plate",
      color: "#c3ced6",
      icon: "plate",
      description: "Smelted iron. The structural backbone of every later tier.",
      stack_size: 10,
    };
    const plateTooltip = itemTooltip(plate, plate.name, {
      progress: { have: 2, need: 5 },
      shortfall: 3,
    });
    expect(plateTooltip).toBe(
      "Iron plate (2 / 5) · Need 3 more\nSmelted iron. The structural backbone of every later tier.",
    );

    expect(itemTooltip(undefined, "Empty")).toBe("Empty");
  });

  /**
   * A compartment slot must survive its own contents changing.
   *
   * `syncChildren` reuses an element for a key and destroys it for any key it no longer sees, so a
   * key that carried the quantity would replace every slot in a running machine on every frame. A
   * stack drag would then press one element and release against its replacement, and the drop target
   * would vanish from under the pointer. Keys keyed to the item alone are what hold the grid still.
   */
  it("keeps compartment slot keys stable while quantities change", () => {
    const expected = [5, 11];
    const empty = machineStockSlots([], expected, true, 60);
    const filling = machineStockSlots(
      [
        { item_id: 5, quantity: 3 },
        { item_id: 11, quantity: 1 },
      ],
      expected,
      true,
      60,
    );
    const fuller = machineStockSlots(
      [
        { item_id: 5, quantity: 19 },
        { item_id: 11, quantity: 7 },
      ],
      expected,
      true,
      60,
    );
    const keys = (slots: { key: string }[]): string[] =>
      slots.map(({ key }) => key);
    expect(keys(empty)).toEqual(["expected-5", "expected-11"]);
    expect(keys(filling)).toEqual(keys(empty));
    expect(keys(fuller)).toEqual(keys(empty));
    // Only the presentation moves: the ghost clears once something is in the slot.
    expect(empty[0]?.ghost).toBe(true);
    expect(filling[0]?.ghost).toBe(false);

    // The same for a compartment with no recipe to expect anything, which is how a container draws.
    const held = machineStockSlots([{ item_id: 5, quantity: 4 }], [], true, 60);
    const heldMore = machineStockSlots(
      [{ item_id: 5, quantity: 40 }],
      [],
      true,
      60,
    );
    expect(keys(held)).toEqual(["stored-5", "drop"]);
    expect(keys(heldMore)).toEqual(keys(held));
    // A full container loses the spare slot, and only that slot: the stack it is holding keeps its
    // element, so a drag already aimed at it still lands. Its store is one shared pool.
    expect(
      keys(machineStockSlots([{ item_id: 5, quantity: 60 }], [], true, 60)),
    ).toEqual(["stored-5"]);
    // A fuel compartment is bounded per item instead, so a firebox holding a whole capacity of one
    // fuel still offers the spare slot — a second fuel gets that capacity to itself, and native
    // would accept it. The drawing has to agree, or the player sees no way to put it in.
    expect(
      keys(
        machineStockSlots([{ item_id: 5, quantity: 60 }], [], true, 60, true),
      ),
    ).toEqual(["stored-5", "drop"]);

    // And never two slots with the same key. A duplicate makes `syncChildren` drop the second
    // element the moment it reconciles, so a slot the player can see would stop existing on the
    // next frame. An item named by the recipe and also sitting in the compartment is the case that
    // would otherwise produce two.
    const both = machineStockSlots(
      [
        { item_id: 5, quantity: 2 },
        { item_id: 11, quantity: 0 },
      ],
      [5, 5, 11, 0],
      true,
    );
    expect(new Set(keys(both)).size).toBe(both.length);
    expect(keys(both)).toEqual(["expected-5", "expected-11"]);
  });
});
