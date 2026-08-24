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
  const leftToggle = element("left-toggle", ["panel-toggle"], {
    panelTarget: "left",
  });
  const rightToggle = element("right-toggle", ["panel-toggle"], {
    panelTarget: "right",
  });
  const elements = new Map(
    [left, right, leftToggle, rightToggle].map((value) => [value.id, value]),
  );
  const root = {
    getElementById(id: string) {
      return elements.get(id) ?? null;
    },
    querySelectorAll(selector: string) {
      if (selector === ".panel-toggle") return [leftToggle, rightToggle];
      if (selector === ".glass-panel.open")
        return [left, right].filter(({ classList }) =>
          classList.contains("open"),
        );
      return [];
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
    values,
  };
}

describe("panel controller", () => {
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
