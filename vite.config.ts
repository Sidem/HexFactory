import { defineConfig } from "vite";

export default defineConfig({
  base: "/HexFactory/",
  build: { target: "es2023" },
  server: { port: 5174 },
});
