import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { evaluateStartup, groupOf } from "./startup-budget.mjs";

/** A build directory holding exactly the named assets, each of the given size. */
function dist(entries) {
  const directory = mkdtempSync(join(tmpdir(), "hexfactory-startup-"));
  const files = [];
  for (const [name, bytes] of Object.entries(entries)) {
    const path = join(directory, name);
    // Incompressible content, so a gzipped size stays close to the size the test asked for.
    writeFileSync(path, randomBytes(bytes));
    const group = groupOf(name);
    if (group) files.push({ name, path, group });
  }
  return { directory, files };
}

test("the admin page is not part of a player's first load", () => {
  assert.equal(groupOf("admin-abc.js"), null);
  assert.equal(groupOf("admin.html"), null);
  assert.equal(groupOf("main-abc.js"), "javascript");
  assert.equal(groupOf("factory.worker-abc.js"), "javascript");
  assert.equal(groupOf("factory_wasm_bg-abc.wasm"), "wasm");
  assert.equal(groupOf("index.html"), "interface");
  assert.equal(groupOf("main-abc.css"), "interface");
});

test("the worker counts as startup JavaScript and the wasm as its own group", () => {
  const { directory, files } = dist({
    "main-abc.js": 4_000,
    "factory.worker-abc.js": 1_000,
    "factory_wasm_bg-abc.wasm": 9_000,
    "admin-abc.js": 8_000,
    "index.html": 500,
  });
  try {
    const result = evaluateStartup({
      files,
      ceilings: { javascript: 100_000, wasm: 100_000, interface: 100_000 },
      totalCeiling: 100_000,
    });
    const by = Object.fromEntries(
      result.groups.map((entry) => [entry.group, entry.raw]),
    );
    assert.equal(by.javascript, 5_000);
    assert.equal(by.wasm, 9_000);
    assert.equal(by.interface, 500);
    assert.equal(result.errors.length, 0);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a group over its ceiling is named, and so is the total", () => {
  const { directory, files } = dist({
    "main-abc.js": 20_000,
    "factory_wasm_bg-abc.wasm": 1_000,
  });
  try {
    const result = evaluateStartup({
      files,
      ceilings: { javascript: 1_000, wasm: 100_000, interface: 100_000 },
      totalCeiling: 1_000,
    });
    assert.match(result.errors.join("\n"), /javascript is .* over its/);
    assert.match(result.errors.join("\n"), /startup total is .* over its/);
    assert.equal(
      result.groups.find((entry) => entry.group === "wasm").over,
      false,
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
