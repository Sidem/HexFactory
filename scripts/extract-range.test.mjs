import assert from "node:assert/strict";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";

const script = fileURLToPath(new URL("./extract-range.mjs", import.meta.url));

test("an exact range is included in place", () => {
  const directory = mkdtempSync(join(tmpdir(), "hexfactory-extract-"));
  try {
    const source = join(directory, "lib.rs");
    const target = join(directory, "model", "middle.rs");
    writeFileSync(source, "first\nsecond\nthird\nfourth\n");
    const run = spawnSync(
      process.execPath,
      [script, source, "2", "3", target, "model/middle.rs"],
      { encoding: "utf8" },
    );
    assert.equal(run.status, 0, run.stderr);
    assert.equal(readFileSync(target, "utf8"), "second\nthird\n");
    assert.equal(
      readFileSync(source, "utf8"),
      'first\ninclude!("model/middle.rs");\nfourth\n',
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a collision leaves the source untouched", () => {
  const directory = mkdtempSync(join(tmpdir(), "hexfactory-extract-"));
  try {
    const source = join(directory, "lib.rs");
    const target = join(directory, "target.rs");
    writeFileSync(source, "first\nsecond\n");
    writeFileSync(target, "occupied\n");
    const run = spawnSync(
      process.execPath,
      [script, source, "1", "1", target, "target.rs"],
      { encoding: "utf8" },
    );
    assert.equal(run.status, 1);
    assert.equal(readFileSync(source, "utf8"), "first\nsecond\n");
    assert.equal(existsSync(target), true);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("CSS ranges become ordered imports", () => {
  const directory = mkdtempSync(join(tmpdir(), "hexfactory-extract-"));
  try {
    const source = join(directory, "styles.css");
    const target = join(directory, "styles", "base.css");
    writeFileSync(source, "a {}\nb {}\n");
    const run = spawnSync(
      process.execPath,
      [script, source, "1", "2", target, "./styles/base.css"],
      { encoding: "utf8" },
    );
    assert.equal(run.status, 0, run.stderr);
    assert.equal(readFileSync(target, "utf8"), "a {}\nb {}\n");
    assert.equal(
      readFileSync(source, "utf8"),
      '@import "./styles/base.css";\n',
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
