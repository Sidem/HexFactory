#!/usr/bin/env node
/** Move an exact inclusive line range to a new file and include it in place. */

import { existsSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname } from "node:path";

const [file, firstText, lastText, target, includePath] = process.argv.slice(2);
const first = Number(firstText);
const last = Number(lastText);

if (
  !file ||
  !target ||
  !includePath ||
  !Number.isInteger(first) ||
  !Number.isInteger(last) ||
  first < 1 ||
  last < first
) {
  console.error(
    "usage: extract-range.mjs <file> <first-line> <last-line> <target> <include-path>",
  );
  process.exit(2);
}
if (existsSync(target)) {
  console.error(`refusing to overwrite ${target}`);
  process.exit(1);
}

const source = readFileSync(file, "utf8");
const lines = source.split(/(?<=\n)/);
if (last > lines.length) {
  console.error(`${file} has only ${lines.length} lines`);
  process.exit(1);
}

const extracted = lines.slice(first - 1, last).join("");
const replacement = file.endsWith(".css")
  ? `@import "${includePath}";\n`
  : `include!("${includePath}");\n`;
const rewritten = [
  ...lines.slice(0, first - 1),
  replacement,
  ...lines.slice(last),
].join("");

mkdirSync(dirname(target), { recursive: true });
writeFileSync(target, extracted);
writeFileSync(file, rewritten);
console.log(
  `moved ${file}:${first}-${last} to ${target} (${Buffer.byteLength(extracted)} bytes)`,
);
