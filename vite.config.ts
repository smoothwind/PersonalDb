import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
// @ts-expect-error type error without @types/node package
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [vue()],

  build: {
    rolldownOptions: {
      output: {
        // Vite 8（rolldown）分包：重依赖独立分包，配合面板异步加载
        advancedChunks: {
          groups: [
            {
              name: "vendor-element-plus",
              test: /[\\/]node_modules[\\/](element-plus|@element-plus)[\\/]/,
            },
            {
              name: "vendor-codemirror",
              test: /[\\/]node_modules[\\/](@codemirror|codemirror)[\\/]/,
            },
            {
              name: "vendor-sql-formatter",
              test: /[\\/]node_modules[\\/]sql-formatter[\\/]/,
            },
            { name: "vendor-misc", test: /[\\/]node_modules[\\/](sortablejs|driver\.js)[\\/]/ },
          ],
        },
      },
    },
    chunkSizeWarningLimit: 900,
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
