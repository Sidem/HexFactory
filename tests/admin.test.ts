import { describe, expect, it } from "vitest";

import { runDiagnostics } from "../src/admin/diagnostics";
import {
  formatDefinitionsJson,
  formatScenariosJson,
  formatTechnologiesJson,
  parseImportedJson,
} from "../src/admin/exporter";
import { FootprintEditor } from "../src/admin/footprintEditor";
import { AdminStore } from "../src/admin/state";
import { validateDefinitions } from "../src/core/definitions";
import type { Definitions } from "../src/core/types";

describe("Admin Dashboard Studio", () => {
  it("opens on the shipped catalogue clean, and diagnoses what a bad edit would break", () => {
    const store = new AdminStore();
    expect(store.definitions.items.length).toBeGreaterThan(10);
    expect(store.definitions.recipes.length).toBeGreaterThan(10);
    expect(store.definitions.buildings.length).toBeGreaterThan(10);
    expect(store.technologies.technologies.length).toBeGreaterThan(5);
    expect(
      runDiagnostics(store.definitions, store.technologies).filter(
        (i) => i.severity === "error",
      ),
    ).toHaveLength(0);

    const brokenDefs = structuredClone(store.definitions);

    // Break 1: Add recipe referencing non-existent item #9999
    brokenDefs.recipes.push({
      id: 999,
      key: "broken-recipe",
      name: "Broken Recipe",
      description: "Broken",
      category: "smelting",
      inputs: [{ item_id: 9999, quantity: 1 }],
      output: { item_id: 1, quantity: 1 },
      duration: 60,
    });

    // Break 2: Duplicate item ID
    brokenDefs.items.push({
      id: 1,
      key: "fake-ore",
      name: "Fake Ore",
      color: "#ffffff",
      icon: "ore",
      description: "Dup",
      stack_size: 10,
    });

    // Break 3: Orphan category with no machine
    brokenDefs.recipes.push({
      id: 998,
      key: "alchemy-recipe",
      name: "Alchemy Recipe",
      description: "No machine",
      category: "alchemy",
      inputs: [{ item_id: 1, quantity: 1 }],
      output: { item_id: 11, quantity: 1 },
      duration: 60,
    });

    const issues = runDiagnostics(brokenDefs, store.technologies);
    expect(issues.some((i) => i.id.includes("bad-input-9999"))).toBe(true);
    expect(issues.some((i) => i.id.includes("duplicate-item-id-1"))).toBe(true);
    expect(issues.some((i) => i.id.includes("orphan-recipe-998"))).toBe(true);
  });

  it("handles CRUD and duplication, undoes it, and reports what is dirty", () => {
    const store = new AdminStore();
    const initialCount = store.definitions.items.length;
    const nextId = store.getNextItemId();

    // Create
    store.saveItem({
      id: nextId,
      key: "super-alloy",
      name: "Super Alloy",
      color: "#ffaa00",
      icon: "plate",
      description: "Advanced structural alloy.",
      stack_size: 20,
    });

    expect(store.definitions.items.length).toBe(initialCount + 1);
    expect(store.definitions.items.find((i) => i.id === nextId)?.name).toBe(
      "Super Alloy",
    );

    // Update
    store.saveItem({
      id: nextId,
      key: "super-alloy",
      name: "Ultra Alloy",
      color: "#ffaa00",
      icon: "plate",
      description: "Upgraded alloy.",
      stack_size: 50,
      fuel_value: 200,
    });

    const updated = store.definitions.items.find((i) => i.id === nextId);
    expect(updated?.name).toBe("Ultra Alloy");
    expect(updated?.stack_size).toBe(50);
    expect(updated?.fuel_value).toBe(200);

    // Duplicate
    const dup = store.duplicateItem(nextId);
    expect(dup).not.toBeNull();
    expect(dup?.id).toBe(nextId + 1);
    expect(dup?.name).toBe("Ultra Alloy (Copy)");

    // Delete
    if (dup) store.deleteItem(dup.id);
    store.deleteItem(nextId);
    expect(store.definitions.items.length).toBe(initialCount);

    // Undo and redo carry a mutation both ways.
    const origLength = store.definitions.recipes.length;
    const recipeId = store.getNextRecipeId();

    store.saveRecipe({
      id: recipeId,
      key: "test-alloy-craft",
      name: "Test Alloy Craft",
      description: "Crafting test alloy.",
      category: "smelting",
      inputs: [{ item_id: 1, quantity: 2 }],
      output: { item_id: 11, quantity: 1 },
      duration: 120,
    });

    expect(store.definitions.recipes.length).toBe(origLength + 1);
    expect(store.canUndo()).toBe(true);

    // Undo
    const undone = store.undo();
    expect(undone).toBe(true);
    expect(store.definitions.recipes.length).toBe(origLength);
    expect(store.canRedo()).toBe(true);

    // Redo
    const redone = store.redo();
    expect(redone).toBe(true);
    expect(store.definitions.recipes.length).toBe(origLength + 1);

    // And a diff against the shipped baseline names the field that moved, not merely the row.
    const clean = new AdminStore();
    expect(clean.getDirtyCount()).toBe(0);
    const ironOre = clean.definitions.items.find((i) => i.key === "ore");
    if (ironOre) clean.saveItem({ ...ironOre, stack_size: 99 });

    const diffs = clean.getDiffSummary();
    expect(diffs.length).toBeGreaterThan(0);
    const itemDiff = diffs.find((d) => d.id === ironOre?.id);
    expect(itemDiff).toBeDefined();
    expect(itemDiff?.changeType).toBe("modified");
    expect(itemDiff?.details[0]).toContain("stack_size");
  });

  it("validates FootprintEditor coordinate management and (0,0) anchor invariant", () => {
    const editor = new FootprintEditor({
      initialFootprint: [{ q: 0, r: 0 }],
      gridRadius: 2,
    });

    expect(editor.getFootprint()).toEqual([{ q: 0, r: 0 }]);

    editor.setFootprint([
      { q: 0, r: 0 },
      { q: 1, r: 0 },
      { q: 0, r: 1 },
    ]);
    const fp = editor.getFootprint();
    expect(fp).toHaveLength(3);
    expect(fp.some((c) => c.q === 0 && c.r === 0)).toBe(true);
    expect(fp.some((c) => c.q === 1 && c.r === 0)).toBe(true);

    // Toggle coordinate
    editor.toggle(1, 0); // remove (1,0)
    expect(editor.getFootprint()).toHaveLength(2);
    editor.toggle(0, 0); // cannot remove anchor (0,0)
    expect(editor.getFootprint()).toHaveLength(2);
  });

  it("exports valid formatted definitions JSON that passes engine validation", () => {
    const store = new AdminStore();
    const jsonStr = formatDefinitionsJson(store.definitions);
    expect(typeof jsonStr).toBe("string");

    const parsed = parseImportedJson<Definitions>(jsonStr);
    expect(parsed.success).toBe(true);
    if (parsed.success) {
      expect(() => validateDefinitions(parsed.data)).not.toThrow();
    }

    const techJson = formatTechnologiesJson(store.technologies);
    expect(parseImportedJson(techJson).success).toBe(true);

    const scenJson = formatScenariosJson(store.scenarios);
    expect(parseImportedJson(scenJson).success).toBe(true);
  });
});
