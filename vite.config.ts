import { resolve } from "node:path";
import { defineConfig } from "vite";

export default defineConfig({
  base: "/HexFactory/",
  build: {
    target: "es2023",
    rollupOptions: {
      input: {
        main: resolve(import.meta.dirname, "index.html"),
        admin: resolve(import.meta.dirname, "admin.html"),
      },
    },
  },
  server: { port: 5174 },
});
