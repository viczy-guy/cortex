import { defineConfig } from "vite";

// Tauri expects a fixed dev port and takes care of opening the window itself.
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    target: "es2022",
    outDir: "dist",
  },
});
