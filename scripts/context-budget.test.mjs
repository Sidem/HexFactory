import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";

import { evaluateBudget } from "./context-budget.mjs";

const script = fileURLToPath(new URL("./context-budget.mjs", import.meta.url));

test("legacy debt is a no-growth ratchet", () => {
  const path = "src/large.ts";
  const debt = {
    baselineBytes: 52_000,
    targetBytes: 30_000,
    owner: "split large source",
    expires: "2099-01-01",
  };
  const result = evaluateBudget({
    files: [path],
    config: { version: 1, allow: { [path]: debt } },
    today: "2026-09-02",
    sizeOf: () => 52_001,
  });
  assert.equal(result.failures.length, 0);
  assert.match(result.errors.join("\n"), /grew 1 bytes/);
});

test("paid and stale debt cannot linger", () => {
  const debt = {
    baselineBytes: 52_000,
    targetBytes: 30_000,
    owner: "split large source",
    expires: "2099-01-01",
  };
  const result = evaluateBudget({
    files: ["src/small.ts"],
    config: {
      version: 1,
      allow: { "src/small.ts": debt, "src/gone.ts": debt },
    },
    today: "2026-09-02",
    sizeOf: () => 100,
  });
  assert.match(result.errors.join("\n"), /debt is paid/);
  assert.match(result.errors.join("\n"), /stale debt entry/);
});

test("context accounting separates root, route, and scoped instructions", () => {
  const sizes = new Map([
    ["AGENTS.md", 1_000],
    ["docs/AGENT-MAP.md", 2_000],
    ["src/AGENTS.md", 3_000],
  ]);
  const result = evaluateBudget({
    files: [...sizes.keys()],
    config: { version: 1, allow: {} },
    sizeOf: (path) => sizes.get(path),
  });
  assert.equal(result.rootInstructions, 1_000);
  assert.equal(result.routeIndex, 2_000);
  assert.equal(result.scopedInstructions, 3_000);
});

test("the CLI includes untracked source files", () => {
  const directory = mkdtempSync(join(tmpdir(), "hexfactory-context-"));
  try {
    spawnSync("git", ["init", "-q"], { cwd: directory });
    writeFileSync(join(directory, "untracked.ts"), "x".repeat(52 * 1024));
    const run = spawnSync(process.execPath, [script, "--json"], {
      cwd: directory,
      encoding: "utf8",
    });
    assert.equal(run.status, 1);
    const result = JSON.parse(run.stdout);
    assert.equal(result.failures[0].path, "untracked.ts");
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
