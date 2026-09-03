#!/usr/bin/env node
/**
 * Hold the production startup payload to a stated ceiling. Use --check to fail on a breach and
 * --json for machine-readable output.
 *
 * Startup is everything the browser must fetch before the title screen answers: the document, the
 * stylesheet, every eagerly loaded script including the module worker, and the Wasm the worker
 * instantiates. The admin page is a separate entry a player never loads, so it is named and
 * excluded rather than silently folded in.
 *
 * Sizes are gzipped, because that is what a player waits for. The raw bytes are reported beside
 * them: they are what the browser then has to parse and compile, and a chunk that compresses well
 * still costs that.
 */

import { gzipSync } from "node:zlib";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const DIST = "dist";

/**
 * Ceilings in gzipped bytes, with the headroom they were set with. Raising one is a decision about
 * what a new player waits through, so it is an edit here and a line in `docs/BENCHMARKS.md` — not
 * something a feature quietly buys by growing a chunk.
 */
const CEILINGS = {
  // Host, renderer, catalogues, and the module worker. 274 KB measured at v0.47.
  javascript: 320 * 1024,
  // The simulation core. 487 KB measured at v0.47; a whole new native system is the reason to move
  // it, and phase 9 onward will need that room named rather than assumed.
  wasm: 560 * 1024,
  // Document and stylesheet together. 35 KB measured at v0.47.
  interface: 48 * 1024,
};
/** The whole first load. Below the sum of the parts, so all three cannot spend their headroom at once. */
const TOTAL_CEILING = 896 * 1024;

/** Which group an emitted asset belongs to, or null when it is not part of a player's first load. */
export function groupOf(name) {
  if (name.startsWith("admin")) return null;
  if (name.endsWith(".wasm")) return "wasm";
  if (name.endsWith(".js")) return "javascript";
  if (name.endsWith(".css") || name.endsWith(".html")) return "interface";
  return null;
}

function assets(dir = DIST) {
  const found = [];
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) {
      found.push(...assets(path));
      continue;
    }
    const group = groupOf(entry);
    if (group) found.push({ name: entry, path, group });
  }
  return found;
}

export function evaluateStartup({
  files = assets(),
  ceilings = CEILINGS,
  totalCeiling = TOTAL_CEILING,
} = {}) {
  const rows = files
    .map((file) => {
      const raw = readFileSync(file.path);
      return {
        ...file,
        raw: raw.length,
        gzip: gzipSync(raw, { level: 9 }).length,
      };
    })
    .sort((a, b) => b.gzip - a.gzip);

  const groups = Object.keys(ceilings).map((group) => {
    const members = rows.filter((row) => row.group === group);
    const gzip = members.reduce((sum, row) => sum + row.gzip, 0);
    return {
      group,
      gzip,
      raw: members.reduce((sum, row) => sum + row.raw, 0),
      ceiling: ceilings[group],
      over: gzip > ceilings[group],
    };
  });
  const gzip = groups.reduce((sum, entry) => sum + entry.gzip, 0);
  const total = {
    gzip,
    raw: groups.reduce((sum, entry) => sum + entry.raw, 0),
    ceiling: totalCeiling,
    over: gzip > totalCeiling,
  };

  const errors = [];
  for (const entry of [...groups, { ...total, group: "startup total" }]) {
    if (entry.over) {
      errors.push(
        `${entry.group} is ${kb(entry.gzip)} gzipped, over its ${kb(entry.ceiling)} budget`,
      );
    }
  }
  return { rows, groups, total, errors };
}

function kb(bytes) {
  return `${(bytes / 1024).toFixed(1)} KB`;
}

function main() {
  if (!existsSync(DIST)) {
    console.error(`no ${DIST}/ to measure; run npm run build first`);
    process.exit(1);
  }
  const result = evaluateStartup();
  if (process.argv.includes("--json")) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    for (const row of result.rows) {
      console.log(
        `  ${row.group.padEnd(10)} ${kb(row.gzip).padStart(9)} gzip  ${kb(row.raw).padStart(9)} raw  ${row.name}`,
      );
    }
    console.log();
    for (const entry of [
      ...result.groups,
      { ...result.total, group: "total" },
    ]) {
      console.log(
        `${entry.group.padEnd(12)} ${kb(entry.gzip).padStart(9)} / ${kb(entry.ceiling)}${entry.over ? "  OVER" : ""}`,
      );
    }
  }
  if (result.errors.length) {
    console.log();
    for (const error of result.errors) console.log(`  ${error}`);
    if (process.argv.includes("--check")) process.exit(1);
  } else if (!process.argv.includes("--json")) {
    console.log("\nstartup budget ok");
  }
}

const isMain = process.argv[1]
  ?.replaceAll("\\", "/")
  .endsWith("/scripts/startup-budget.mjs");
if (isMain) main();
