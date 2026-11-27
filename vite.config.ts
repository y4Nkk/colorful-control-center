import { defineConfig, type Connect, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
// @ts-expect-error type error without @types/node package
import { readFile } from "node:fs/promises";
// @ts-expect-error type error without @types/node package
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;

function serveBundledFonts(): Plugin {
  const files = new Map([
    ["/fonts.css", "text/css; charset=utf-8"],
    ["/google-sans-code.ttf", "font/ttf"],
    ["/wqy-zenhei.ttf", "font/ttf"],
  ]);
  const directory = new URL("./src-tauri/resources/fonts/", import.meta.url);
  const serve = ({ middlewares }: { middlewares: Connect.Server }) => {
    middlewares.use("/fonts", (request: Connect.IncomingMessage & { url?: string; method?: string }, response, next) => {
      const path = request.url ?? "";
      const contentType = files.get(path);
      if (request.method !== "GET" || !contentType) return next();
      void readFile(new URL(path.slice(1), directory)).then((bytes: Uint8Array) => {
        response.setHeader("Content-Type", contentType);
        response.end(bytes);
      }, next);
    });
  };
  return { name: "ccc-bundled-fonts", configureServer: serve, configurePreviewServer: serve };
}

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [react(), serveBundledFonts()],

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
