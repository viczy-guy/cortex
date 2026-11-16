import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

// Tauri expects a fixed dev port and takes care of opening the window itself.
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    rollupOptions: {
      input: {
        island: fileURLToPath(new URL("./index.html", import.meta.url)),
        settings: fileURLToPath(new URL("./settings.html", import.meta.url)),
      },
    },
    target: "es2022",
    outDir: "dist",
  },
});
