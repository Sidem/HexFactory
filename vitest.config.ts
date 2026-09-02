import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "node",
    exclude: ["scripts/**/*.test.mjs", "**/node_modules/**"],
  },
});
