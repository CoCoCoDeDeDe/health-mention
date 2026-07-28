import { defineConfig } from "vite";

export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // 忽略 Rust 构建产物，避免 watcher 与 cargo 冲突
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    target: "es2022",
    rollupOptions: {
      input: {
        main: "index.html",
        overlay: "overlay.html",
      },
    },
  },
});
