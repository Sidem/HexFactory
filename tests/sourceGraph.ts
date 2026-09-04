import { readFileSync } from "node:fs";

/** Read the browser application graph for source-contract tests. */
export function readAppSource(): string {
  return [
    "main",
    "app/runtime",
    "app/bootstrap",
    "app/coreView",
    "app/buildController",
    "app/inspectorOverview",
    "app/inspectorControls",
    "app/workspaceWiring",
    "app/workspaceController",
    "app/buildWiring",
    "app/inputWiring",
    "app/constructionInput",
    "app/lifecycleWiring",
    "app/lifecycle",
    "app/createApp",
  ]
    .map((name) =>
      readFileSync(new URL(`../src/${name}.ts`, import.meta.url), "utf8"),
    )
    .join("\n");
}

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
