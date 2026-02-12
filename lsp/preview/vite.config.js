import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    proxy: {
      "/api": "http://localhost:9999",
      "/ws": { target: "ws://localhost:9999", ws: true },
      "/health": "http://localhost:9999",
      "/render": "http://localhost:9999",
    },
  },
});
