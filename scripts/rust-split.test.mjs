import assert from "node:assert/strict";
import {
  existsSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";

const script = fileURLToPath(new URL("./rust-split.mjs", import.meta.url));
const source = `struct Core;

impl Core {
    /// First method.
    fn first(&self) {
        let _raw = r#"{ not code }"#;
        /* nested { /* still not code } */ } */
    }

    #[allow(dead_code)]
    fn second(&self) {
        let _closure = || { 1 };
    }
}
`;

function fixture() {
  const directory = mkdtempSync(join(tmpdir(), "hexfactory-rust-split-"));
  const input = join(directory, "lib.rs");
  writeFileSync(input, source);
  return { directory, input };
}

test("inventory ignores braces in strings and nested comments", () => {
  const { directory, input } = fixture();
  try {
    const run = spawnSync(
      process.execPath,
      [script, "inventory", input, "--impl", "Core", "--json"],
      { encoding: "utf8" },
    );
    assert.equal(run.status, 0, run.stderr);
    const inventory = JSON.parse(run.stdout);
    assert.deepEqual(
      inventory.methods.map(({ name }) => name),
      ["first", "second"],
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("apply validates all collisions before writing", () => {
  const { directory, input } = fixture();
  try {
    const output = join(directory, "core");
    mkdirSync(output);
    writeFileSync(join(output, "second.rs"), "occupied");
    const map = join(directory, "map.json");
    writeFileSync(map, JSON.stringify({ first: "first", second: "second" }));
    const run = spawnSync(
      process.execPath,
      [
        script,
        "apply",
        input,
        "--impl",
        "Core",
        "--map",
        map,
        "--out-dir",
        output,
      ],
      { encoding: "utf8" },
    );
    assert.equal(run.status, 1);
    assert.equal(existsSync(join(output, "first.rs")), false);
    assert.equal(readFileSync(input, "utf8"), source);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("apply can select a bounded method range without a map file", () => {
  const { directory, input } = fixture();
  try {
    const output = join(directory, "core");
    const run = spawnSync(
      process.execPath,
      [
        script,
        "apply",
        input,
        "--impl",
        "Core",
        "--lines",
        "4-9",
        "--module",
        "first",
        "--out-dir",
        output,
      ],
      { encoding: "utf8" },
    );
    assert.equal(run.status, 0, run.stderr);
    assert.match(readFileSync(join(output, "first.rs"), "utf8"), /fn first/);
    assert.doesNotMatch(readFileSync(input, "utf8"), /fn first/);
    assert.match(readFileSync(input, "utf8"), /fn second/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
