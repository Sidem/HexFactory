import { describe, expect, it } from "vitest";

import { PanelController } from "../src/ui/panels";

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
    querySelector() {
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
  it("returns to the main toggle when a cross-link opener is now hidden", () => {
    const view = harness();
    view.controller.toggle(
      "research-panel",
      view.rightToggle as unknown as HTMLElement,
    );
    view.rightToggle.visible = false;
    view.controller.close();
    expect(view.rightToggle.focused).toBe(false);
    expect(view.modalToggle.focused).toBe(true);
  });
  it("opens research modally, closes on Escape, and returns focus to its opener", () => {
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
  });

  it("does not restore a modal over the title screen and dismisses it when opening another workspace", () => {
    const view = harness('["research-panel"]');
    view.controller.restore();
    expect(view.modal.open).toBe(false);
    expect(view.modal.classList.contains("open")).toBe(false);
    view.controller.toggle("research-panel");
    view.controller.toggle("right");
    expect(view.modal.open).toBe(false);
    expect(view.right.classList.contains("open")).toBe(true);
  });
  it("restores only the last valid workspace and synchronizes its toggles", () => {
    const view = harness('["missing","left","right"]');
    view.left.classList.remove("open");
    view.controller.restore();

    expect(view.left.classList.contains("open")).toBe(false);
    expect(view.right.classList.contains("open")).toBe(true);
    expect(view.leftToggle.attributes.get("aria-expanded")).toBe("false");
    expect(view.rightToggle.attributes.get("aria-expanded")).toBe("true");
  });

  it("makes opening a workspace exclusive and persists the resulting ids", () => {
    const view = harness();
    view.controller.toggle("right");

    expect(view.left.classList.contains("open")).toBe(false);
    expect(view.right.classList.contains("open")).toBe(true);
    expect(view.values.get("hexfactory:panels:v1")).toBe('["right"]');

    view.controller.close();
    expect(view.right.classList.contains("open")).toBe(false);
    expect(view.values.get("hexfactory:panels:v1")).toBe("[]");
  });
});
