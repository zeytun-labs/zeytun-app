import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import path from "path";

const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [tailwindcss(), sveltekit()],
  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,

  resolve: {
    alias: [
      { find: "$lib", replacement: path.resolve("./src/lib") },
      // Expose the exact @dnd-kit/dom@0.2.4 instance that @dnd-kit-svelte uses
      // so rule-table.svelte can import Feedback/defaultPreset to disable the
      // drop animation. Exact-match regex so subpath imports
      // (@dnd-kit/dom/sortable, /utilities, ...) still resolve normally.
      {
        find: /^@dnd-kit\/dom$/,
        replacement: path.resolve(
          "./node_modules/.pnpm/@dnd-kit+dom@0.2.4/node_modules/@dnd-kit/dom/index.js",
        ),
      },
    ],
  },

  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: {
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
