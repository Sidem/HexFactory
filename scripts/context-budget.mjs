#!/usr/bin/env node
/**
 * context-budget.mjs — fails the quality gate when a file grows past the size
 * at which an agent can no longer read it safely.
 *
 *   node scripts/context-budget.mjs           # report
 *   node scripts/context-budget.mjs --check   # exit 1 on any failure
 *   node scripts/context-budget.mjs --json
 *
 * Waivers live in .agent-budget.json:
 *   { "allow": { "factory-wasm/src/lib.rs": "phase 3 in progress" } }
 * Every waiver is a debt entry — the report prints them so they stay visible.
 */

import { readFileSync, existsSync, statSync } from "node:fs";
import { execSync } from "node:child_process";

const KB = 1024;

const RULES = [
  {
    name: "agent context (always paid)",
    warn: 3 * KB,
    fail: 6 * KB,
    test: (p) => /(^|\/)AGENTS\.md$/.test(p),
  },
  {
    name: "route map",
    warn: 4 * KB,
    fail: 8 * KB,
    test: (p) => /AGENT-MAP\.md$/.test(p) || p.startsWith(".agent/"),
  },
  {
    name: "test",
    warn: 25 * KB,
    fail: 50 * KB,
    test: (p) => /\.test\.[jt]s$/.test(p) || /(^|\/)tests?\//.test(p),
  },
  {
    name: "source",
    warn: 25 * KB,
    fail: 50 * KB,
    test: (p) => /\.(rs|ts|tsx|js|mjs)$/.test(p),
  },
  {
    name: "stylesheet",
    warn: 25 * KB,
    fail: 60 * KB,
    test: (p) => /\.css$/.test(p),
  },
  {
    name: "template",
    warn: 20 * KB,
    fail: 50 * KB,
    test: (p) => /\.html$/.test(p),
  },
  {
    name: "doc",
    warn: 30 * KB,
    fail: 80 * KB,
    test: (p) => /^docs\/.*\.md$/.test(p) || /^README\.md$/.test(p),
  },
];

const SKIP =
  /^(package-lock\.json|.*\.(png|jpe?g|gif|webp|svg|ico|wasm|lock)$)/i;

const config = existsSync(".agent-budget.json")
  ? JSON.parse(readFileSync(".agent-budget.json", "utf8"))
  : { allow: {} };
const allow = config.allow ?? {};

const files = execSync("git ls-files", { encoding: "utf8" })
  .split("\n")
  .filter(Boolean)
  .filter((p) => !SKIP.test(p) && existsSync(p));

const rows = [];
for (const path of files) {
  const rule = RULES.find((r) => r.test(path));
  if (!rule) continue;
  const bytes = statSync(path).size;
  let status = "ok";
  if (bytes >= rule.fail) status = "FAIL";
  else if (bytes >= rule.warn) status = "warn";
  const waived = status === "FAIL" && path in allow;
  rows.push({
    path,
    bytes,
    rule: rule.name,
    limit: rule.fail,
    status,
    waived,
    reason: allow[path] ?? null,
  });
}

rows.sort((a, b) => b.bytes - a.bytes);
const failures = rows.filter((r) => r.status === "FAIL" && !r.waived);
const waived = rows.filter((r) => r.waived);
const warnings = rows.filter((r) => r.status === "warn");

// Always-paid context: what every task loads before it does anything.
const alwaysPaid = rows
  .filter(
    (r) => /(^|\/)AGENTS\.md$/.test(r.path) || /AGENT-MAP\.md$/.test(r.path),
  )
  .reduce((a, r) => a + r.bytes, 0);

if (process.argv.includes("--json")) {
  console.log(
    JSON.stringify({ alwaysPaid, failures, warnings, waived }, null, 2),
  );
  process.exit(failures.length ? 1 : 0);
}

const kb = (b) => `${(b / KB).toFixed(0)} KB`;
const tok = (b) => `~${Math.round(b / 3.6 / 100) / 10}k tok`;

console.log(
  `always-paid agent context: ${kb(alwaysPaid)} (${tok(alwaysPaid)})`,
);
console.log(
  `largest file in budget scope: ${rows[0] ? `${rows[0].path} ${kb(rows[0].bytes)}` : "n/a"}`,
);
console.log();

if (failures.length) {
  console.log(`FAIL (${failures.length})`);
  for (const r of failures)
    console.log(
      `  ${r.path}\n    ${r.bytes.toLocaleString()} bytes exceeds ${r.rule} limit (${r.limit.toLocaleString()})`,
    );
  console.log();
}
if (waived.length) {
  console.log(
    `waived debt (${waived.length}) — remove from .agent-budget.json when fixed`,
  );
  for (const r of waived)
    console.log(`  ${r.path}  ${kb(r.bytes)}  — ${r.reason}`);
  console.log();
}
if (warnings.length && !process.argv.includes("--check")) {
  console.log(`approaching limit (${warnings.length})`);
  for (const r of warnings.slice(0, 15))
    console.log(`  ${r.path}  ${kb(r.bytes)}`);
  console.log();
}

if (process.argv.includes("--check")) {
  if (failures.length) {
    console.error("context budget check failed");
    process.exit(1);
  }
  console.log("context budget ok");
}
