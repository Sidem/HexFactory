#!/usr/bin/env node
/**
 * rust-split.mjs — mechanical decomposition helper for oversized Rust files.
 *
 * Moves whole `impl` methods (with their doc comments and attributes) out of a
 * source file into sibling modules, byte-for-byte. Nothing is rewritten, so no
 * LLM needs to read the method bodies.
 *
 *   node scripts/rust-split.mjs inventory factory-wasm/src/lib.rs --impl Core
 *   node scripts/rust-split.mjs inventory factory-wasm/src/lib.rs --impl Core --json > /tmp/core.json
 *   node scripts/rust-split.mjs apply factory-wasm/src/lib.rs --impl Core \
 *        --lines 2816-3164 --module catalog --out-dir factory-wasm/src/core --dry-run
 *
 * The map file is `{ "method_name": "module_stem", ... }`. Methods absent from
 * the map stay put. Emitted modules contain `impl Core { ... }` with the moved
 * methods verbatim; you add `use super::*;` imports and `pub(crate)` field
 * visibility yourself (the compiler will list exactly what is missing).
 */

import { readFileSync, writeFileSync, mkdirSync, existsSync } from "node:fs";
import { join, basename } from "node:path";

/* ------------------------------------------------------------------ */
/* Lexical mask: mark every byte as code (1) or comment/string (0).     */
/* ------------------------------------------------------------------ */

function codeMask(src) {
  const n = src.length;
  const mask = new Uint8Array(n).fill(1);
  let i = 0;
  const blank = (from, to) => {
    for (let k = from; k < to && k < n; k++) mask[k] = 0;
  };
  while (i < n) {
    const c = src[i];
    // line comment
    if (c === "/" && src[i + 1] === "/") {
      const end = src.indexOf("\n", i);
      const stop = end === -1 ? n : end;
      blank(i, stop);
      i = stop;
      continue;
    }
    // block comment (Rust nests them)
    if (c === "/" && src[i + 1] === "*") {
      let depth = 1;
      let j = i + 2;
      while (j < n && depth > 0) {
        if (src[j] === "/" && src[j + 1] === "*") {
          depth++;
          j += 2;
        } else if (src[j] === "*" && src[j + 1] === "/") {
          depth--;
          j += 2;
        } else j++;
      }
      blank(i, j);
      i = j;
      continue;
    }
    // raw string: r"..." / r#"..."# / br#"..."#
    const rawStart = /^b?r(#*)"/.exec(src.slice(i, i + 12));
    if (rawStart && (c === "r" || (c === "b" && src[i + 1] === "r"))) {
      const hashes = rawStart[1];
      const open = i + rawStart[0].length;
      const closer = '"' + hashes;
      const end = src.indexOf(closer, open);
      const stop = end === -1 ? n : end + closer.length;
      blank(i, stop);
      i = stop;
      continue;
    }
    // normal string / byte string
    if (c === '"' || (c === "b" && src[i + 1] === '"')) {
      let j = c === '"' ? i + 1 : i + 2;
      while (j < n) {
        if (src[j] === "\\") j += 2;
        else if (src[j] === '"') {
          j++;
          break;
        } else j++;
      }
      blank(i, j);
      i = j;
      continue;
    }
    // char literal vs lifetime
    if (c === "'") {
      if (src[i + 1] === "\\") {
        let j = i + 2;
        while (j < n && src[j] !== "'") j++;
        blank(i, j + 1);
        i = j + 1;
        continue;
      }
      if (src[i + 2] === "'") {
        blank(i, i + 3);
        i += 3;
        continue;
      }
      i++; // lifetime — ordinary code
      continue;
    }
    i++;
  }
  return mask;
}

/** Match the brace opened at `open`, ignoring braces inside comments/strings. */
function matchBrace(src, mask, open) {
  let depth = 0;
  for (let i = open; i < src.length; i++) {
    if (!mask[i]) continue;
    if (src[i] === "{") depth++;
    else if (src[i] === "}") {
      depth--;
      if (depth === 0) return i;
    }
  }
  throw new Error(`unbalanced brace opened at offset ${open}`);
}

const lineOf = (src, off) => src.slice(0, off).split("\n").length;

/* ------------------------------------------------------------------ */
/* Locate `impl <Type> {` at top level.                                */
/* ------------------------------------------------------------------ */

function findImplBlock(src, mask, typeName) {
  const re = new RegExp(`\\bimpl\\s+${typeName}\\s*\\{`, "g");
  const hits = [];
  let m;
  while ((m = re.exec(src))) {
    if (!mask[m.index]) continue;
    const open = m.index + m[0].length - 1;
    hits.push({ start: m.index, open, end: matchBrace(src, mask, open) });
  }
  if (hits.length === 0) throw new Error(`no \`impl ${typeName}\` found`);
  return hits;
}

/* ------------------------------------------------------------------ */
/* Enumerate methods inside an impl block.                             */
/* ------------------------------------------------------------------ */

const FN_RE =
  /\b(?:pub(?:\s*\([^)]*\))?\s+)?(?:default\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?(?:extern\s+"[^"]*"\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)/g;

/** Walk backwards from `sigStart` over attributes, doc comments and blank space. */
function leadingTrivia(src, sigStart, floor) {
  let lineStart = src.lastIndexOf("\n", sigStart - 1) + 1;
  let cursor = lineStart;
  for (;;) {
    const prevEnd = cursor - 1;
    if (prevEnd <= floor) break;
    const prevStart = src.lastIndexOf("\n", prevEnd - 1) + 1;
    if (prevStart < floor) break;
    const text = src.slice(prevStart, prevEnd).trim();
    const isTrivia =
      text.startsWith("///") ||
      text.startsWith("//!") ||
      text.startsWith("//") ||
      text.startsWith("#[") ||
      text.startsWith("#!") ||
      // continuation of a multi-line attribute
      (text.endsWith(",") && /^[A-Za-z_"(]/.test(text));
    if (!isTrivia) break;
    cursor = prevStart;
  }
  return cursor;
}

function methodsIn(src, mask, block) {
  const out = [];
  FN_RE.lastIndex = block.open;
  let m;
  while ((m = FN_RE.exec(src))) {
    if (m.index > block.end) break;
    if (!mask[m.index]) continue;
    // depth 1 only (skip nested fns inside bodies)
    let depth = 0;
    for (let i = block.open; i < m.index; i++) {
      if (!mask[i]) continue;
      if (src[i] === "{") depth++;
      else if (src[i] === "}") depth--;
    }
    if (depth !== 1) continue;

    // find body opening brace, skipping generics/params/where clause
    let i = m.index + m[0].length;
    let bodyOpen = -1;
    let paren = 0;
    let angle = 0;
    for (; i <= block.end; i++) {
      if (!mask[i]) continue;
      const c = src[i];
      if (c === "(") paren++;
      else if (c === ")") paren--;
      else if (c === "<") angle++;
      else if (c === ">") angle = Math.max(0, angle - 1);
      else if (c === ";" && paren === 0) {
        bodyOpen = -1;
        break;
      } // trait stub
      else if (c === "{" && paren === 0) {
        bodyOpen = i;
        break;
      }
    }
    if (bodyOpen === -1) continue;
    const bodyEnd = matchBrace(src, mask, bodyOpen);
    const start = leadingTrivia(src, m.index, block.open + 1);
    out.push({
      name: m[1],
      start,
      end: bodyEnd + 1,
      bytes: bodyEnd + 1 - start,
      line: lineOf(src, start),
      endLine: lineOf(src, bodyEnd),
    });
    FN_RE.lastIndex = bodyEnd;
  }
  return out;
}

/* ------------------------------------------------------------------ */
/* Commands                                                            */
/* ------------------------------------------------------------------ */

const argv = process.argv.slice(2);
const cmd = argv[0];
const file = argv[1];
const flag = (name, dflt = null) => {
  const i = argv.indexOf(`--${name}`);
  return i === -1 ? dflt : argv[i + 1];
};
const has = (name) => argv.includes(`--${name}`);

if (!cmd || !file || !["inventory", "apply"].includes(cmd)) {
  console.error(
    "usage: rust-split.mjs <inventory|apply> <file.rs> --impl <Type> [--map m.json | --lines first-last --module name] --out-dir dir [--json] [--dry-run]",
  );
  process.exit(2);
}

const src = readFileSync(file, "utf8");
const mask = codeMask(src);
const typeName = flag("impl");
if (!typeName) {
  console.error("--impl <Type> is required");
  process.exit(2);
}

const blocks = findImplBlock(src, mask, typeName);
const methods = blocks.flatMap((b) => methodsIn(src, mask, b));

if (cmd === "inventory") {
  if (has("json")) {
    console.log(
      JSON.stringify(
        { file, type: typeName, blocks: blocks.length, methods },
        null,
        2,
      ),
    );
  } else {
    console.log(
      `${file}  impl ${typeName}  (${blocks.length} block(s), ${methods.length} methods)`,
    );
    for (const b of blocks)
      console.log(
        `  block lines ${lineOf(src, b.start)}-${lineOf(src, b.end)}`,
      );
    console.log();
    for (const m of methods)
      console.log(
        `${String(m.bytes).padStart(7)}  L${String(m.line).padStart(6)}-${String(m.endLine).padEnd(6)}  ${m.name}`,
      );
    const total = methods.reduce((a, m) => a + m.bytes, 0);
    console.log(
      `\ntotal in methods: ${(total / 1024).toFixed(0)} KB of ${(src.length / 1024).toFixed(0)} KB file`,
    );
  }
  process.exit(0);
}

/* ---- apply ---- */

const mapPath = flag("map");
const outDir = flag("out-dir");
const lineRange = flag("lines");
const rangeModule = flag("module");
if ((!mapPath && !(lineRange && rangeModule)) || !outDir) {
  console.error(
    "apply requires --out-dir and either --map or --lines with --module",
  );
  process.exit(2);
}
let map;
if (mapPath) {
  map = JSON.parse(readFileSync(mapPath, "utf8"));
} else {
  const match = /^(\d+)-(\d+)$/.exec(lineRange);
  if (!match) {
    console.error("--lines must be first-last");
    process.exit(2);
  }
  const first = Number(match[1]);
  const last = Number(match[2]);
  map = Object.fromEntries(
    methods
      .filter((method) => method.line >= first && method.endLine <= last)
      .map((method) => [method.name, rangeModule]),
  );
  if (!Object.keys(map).length) {
    console.error(`no methods wholly inside lines ${lineRange}`);
    process.exit(1);
  }
}
const dryRun = has("dry-run");

const byName = new Map(methods.map((m) => [m.name, m]));
const unknown = Object.keys(map).filter((k) => !byName.has(k));
if (unknown.length) {
  console.error(`unknown method(s) in map: ${unknown.join(", ")}`);
  process.exit(1);
}

const groups = new Map();
for (const [name, mod] of Object.entries(map)) {
  if (!groups.has(mod)) groups.set(mod, []);
  groups.get(mod).push(byName.get(name));
}

// Cut spans back-to-front so offsets stay valid.
const cuts = [...groups.values()].flat().sort((a, b) => b.start - a.start);
let rewritten = src;
for (const c of cuts)
  rewritten = rewritten.slice(0, c.start) + rewritten.slice(c.end);
rewritten = rewritten.replace(/\n{3,}/g, "\n\n");

const header = (mod) =>
  `//! ${mod} — extracted from ${basename(file)} by scripts/rust-split.mjs.\n` +
  `//! Methods moved verbatim; add the imports the compiler asks for.\n\n` +
  `use super::*;\n\n`;

function crateVisibleMethod(source) {
  return source.replace(
    /^(\s*)(?=(?:(?:default|const|async|unsafe)\s+|extern\s+"[^"]*"\s+)*fn\s)/m,
    "$1pub(crate) ",
  );
}

let moved = 0;
const plan = [];
for (const [mod, list] of groups) {
  list.sort((a, b) => a.start - b.start);
  const body = list
    .map((m) => crateVisibleMethod(src.slice(m.start, m.end)))
    .join("\n\n");
  const content = `${header(mod)}impl ${typeName} {\n${body}\n}\n`;
  const target = join(outDir, `${mod}.rs`);
  plan.push({ target, methods: list.length, bytes: content.length, content });
  moved += list.reduce((a, m) => a + m.bytes, 0);
}

if (!dryRun) {
  const collisions = plan.filter(({ target }) => existsSync(target));
  if (collisions.length) {
    console.error(
      `refusing to overwrite: ${collisions.map(({ target }) => target).join(", ")}`,
    );
    process.exit(1);
  }
  mkdirSync(outDir, { recursive: true });
  for (const { target, content } of plan) writeFileSync(target, content);
}

for (const p of plan)
  console.log(
    `${dryRun ? "would write" : "wrote"} ${p.target}  ${p.methods} methods  ${(p.bytes / 1024).toFixed(0)} KB`,
  );
console.log(
  `${dryRun ? "would shrink" : "shrank"} ${file}: ${(src.length / 1024).toFixed(0)} KB -> ${(rewritten.length / 1024).toFixed(0)} KB (moved ${(moved / 1024).toFixed(0)} KB)`,
);

const declHint = [...groups.keys()].map((m) => `mod ${m};`).join("\n");
console.log(`\nadd to the module that owns \`${typeName}\`:\n${declHint}`);

if (!dryRun) writeFileSync(file, rewritten);
