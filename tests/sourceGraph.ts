import { readFileSync } from "node:fs";

/** Read the stylesheet graph in cascade order for source-contract tests. */
export function readStyles(): string {
  return ["base", "catalogue", "world", "forms", "dock", "responsive"]
    .map((name) =>
      readFileSync(
        new URL(`../src/styles/${name}.css`, import.meta.url),
        "utf8",
      ),
    )
    .join("\n");
}
