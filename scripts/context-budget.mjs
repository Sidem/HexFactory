#!/usr/bin/env node
/**
 * Enforce small agent-facing files and ratchet legacy debt down without allowing
 * a grandfathered file to grow. Use --json for machine-readable output.
 */

import { existsSync, readFileSync, statSync } from "node:fs";
import { execFileSync } from "node:child_process";

const KB = 1024;
const TODAY = new Date().toISOString().slice(0, 10);

const RULES = [
  {
    name: "agent context",
    warn: 3 * KB,
    fail: 6 * KB,
    test: (path) => /(^|\/)AGENTS\.md$/.test(path),
  },
  {
    name: "route map",
    warn: 4 * KB,
    fail: 8 * KB,
    test: (path) => /AGENT-MAP\.md$/.test(path) || path.startsWith(".agent/"),
  },
  {
    name: "test",
    warn: 25 * KB,
    fail: 50 * KB,
    test: (path) => /\.test\.[jt]s$/.test(path) || /(^|\/)tests?\//.test(path),
  },
  {
    name: "source",
    warn: 25 * KB,
    fail: 50 * KB,
    test: (path) => /\.(rs|ts|tsx|js|mjs)$/.test(path),
  },
  {
    name: "stylesheet",
    warn: 25 * KB,
    fail: 60 * KB,
    test: (path) => /\.css$/.test(path),
  },
  {
    name: "template",
    warn: 20 * KB,
    fail: 50 * KB,
    test: (path) => /\.html$/.test(path),
  },
  {
    name: "doc",
    warn: 30 * KB,
    fail: 80 * KB,
    test: (path) => /^docs\/.*\.md$/.test(path) || path === "README.md",
  },
];

const SKIP =
  /^(package-lock\.json|.*\.(png|jpe?g|gif|webp|svg|ico|wasm|lock)$)/i;

function listedFiles() {
  return execFileSync(
    "git",
    ["ls-files", "--cached", "--others", "--exclude-standard"],
    { encoding: "utf8" },
  )
    .split(/\r?\n/)
    .filter(Boolean)
    .filter((path) => !SKIP.test(path) && existsSync(path));
}

function readConfig() {
  return existsSync(".agent-budget.json")
    ? JSON.parse(readFileSync(".agent-budget.json", "utf8"))
    : { version: 1, allow: {} };
}

export function evaluateBudget({
  files = listedFiles(),
  config = readConfig(),
  today = TODAY,
  sizeOf = (path) => statSync(path).size,
} = {}) {
  const errors = [];
  if (config.version !== 1)
    errors.push(".agent-budget.json must declare version 1");
  const allow = config.allow ?? {};
  const fileSet = new Set(files);

  for (const [path, debt] of Object.entries(allow)) {
    if (!fileSet.has(path)) errors.push(`stale debt entry: ${path}`);
    if (!debt || typeof debt !== "object" || Array.isArray(debt)) {
      errors.push(`debt entry must be an object: ${path}`);
      continue;
    }
    if (!Number.isInteger(debt.baselineBytes) || debt.baselineBytes <= 0)
      errors.push(`debt entry needs positive baselineBytes: ${path}`);
    if (!Number.isInteger(debt.targetBytes) || debt.targetBytes <= 0)
      errors.push(`debt entry needs positive targetBytes: ${path}`);
    if (!debt.owner || typeof debt.owner !== "string")
      errors.push(`debt entry needs an owner: ${path}`);
    if (!/^\d{4}-\d{2}-\d{2}$/.test(debt.expires ?? ""))
      errors.push(`debt entry needs an ISO expiry date: ${path}`);
    else if (debt.expires < today)
      errors.push(`debt entry expired ${debt.expires}: ${path}`);
  }

  const rows = [];
  for (const path of files) {
    const rule = RULES.find((candidate) => candidate.test(path));
    if (!rule) continue;
    const bytes = sizeOf(path);
    let status = "ok";
    if (bytes >= rule.fail) status = "FAIL";
    else if (bytes >= rule.warn) status = "warn";
    const debt = allow[path] ?? null;
    const waived = status === "FAIL" && debt !== null;
    const growth = waived && bytes > debt.baselineBytes;
    if (growth)
      errors.push(
        `${path} grew ${bytes - debt.baselineBytes} bytes above its debt baseline`,
      );
    if (debt && status !== "FAIL")
      errors.push(`debt is paid; remove its entry: ${path}`);
    rows.push({
      path,
      bytes,
      rule: rule.name,
      limit: rule.fail,
      status,
      waived,
      growth,
      debt,
    });
  }

  rows.sort((a, b) => b.bytes - a.bytes);
  const failures = rows.filter((row) => row.status === "FAIL" && !row.waived);
  const waived = rows.filter((row) => row.waived);
  const warnings = rows.filter((row) => row.status === "warn");
  const bytesAt = (path) => rows.find((row) => row.path === path)?.bytes ?? 0;
  const rootInstructions = bytesAt("AGENTS.md");
  const routeIndex = bytesAt("docs/AGENT-MAP.md");
  const scopedInstructions = rows
    .filter(
      (row) => row.path !== "AGENTS.md" && /(^|\/)AGENTS\.md$/.test(row.path),
    )
    .reduce((total, row) => total + row.bytes, 0);

  return {
    rootInstructions,
    routeIndex,
    scopedInstructions,
    failures,
    warnings,
    waived,
    errors,
    rows,
  };
}

const kb = (bytes) => `${(bytes / KB).toFixed(0)} KB`;
const tok = (bytes) => `~${Math.round(bytes / 3.6 / 100) / 10}k tok`;

function report(result, json) {
  if (json) {
    console.log(JSON.stringify(result, null, 2));
    return;
  }
  console.log(
    `root instructions: ${kb(result.rootInstructions)} (${tok(result.rootInstructions)})`,
  );
  console.log(
    `optional route index: ${kb(result.routeIndex)} (${tok(result.routeIndex)})`,
  );
  console.log(`all scoped instructions: ${kb(result.scopedInstructions)}`);
  console.log(
    `largest file in budget scope: ${result.rows[0] ? `${result.rows[0].path} ${kb(result.rows[0].bytes)}` : "n/a"}`,
  );
  console.log();

  if (result.failures.length) {
    console.log(`FAIL (${result.failures.length})`);
    for (const row of result.failures)
      console.log(
        `  ${row.path}\n    ${row.bytes.toLocaleString()} bytes exceeds ${row.rule} limit (${row.limit.toLocaleString()})`,
      );
    console.log();
  }
  if (result.errors.length) {
    console.log(`DEBT ERRORS (${result.errors.length})`);
    for (const error of result.errors) console.log(`  ${error}`);
    console.log();
  }
  if (result.waived.length) {
    console.log(
      `ratcheted debt (${result.waived.length}) — growth is forbidden`,
    );
    for (const row of result.waived)
      console.log(
        `  ${row.path}  ${kb(row.bytes)} -> ${kb(row.debt.targetBytes)}  — ${row.debt.owner}, expires ${row.debt.expires}`,
      );
    console.log();
  }
  if (result.warnings.length && !process.argv.includes("--check")) {
    console.log(`approaching limit (${result.warnings.length})`);
    for (const row of result.warnings.slice(0, 15))
      console.log(`  ${row.path}  ${kb(row.bytes)}`);
    console.log();
  }
}

const isMain = process.argv[1]
  ?.replaceAll("\\", "/")
  .endsWith("/scripts/context-budget.mjs");
if (isMain) {
  const result = evaluateBudget();
  report(result, process.argv.includes("--json"));
  if (result.failures.length || result.errors.length) {
    if (!process.argv.includes("--json"))
      console.error("context budget check failed");
    process.exitCode = 1;
  } else if (process.argv.includes("--check")) {
    console.log("context budget ok");
  }
}
