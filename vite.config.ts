import { defineConfig, type Plugin } from "vite";
import vue from "@vitejs/plugin-vue";
import { fileURLToPath, URL } from "node:url";

const host = process.env.TAURI_DEV_HOST;
const tauriMock = fileURLToPath(new URL("./e2e/mocks/tauri.ts", import.meta.url));

function mockTauriForE2e(): Plugin {
  return {
    name: "convertzz-e2e-tauri-mock",
    enforce: "pre",
    resolveId(id) {
      if (process.env.CONVERTZZ_E2E !== "1") return null;
      if (id === "@tauri-apps/api" || id.startsWith("@tauri-apps/")) return tauriMock;
      return null;
    },
  };
}

export default defineConfig({
  plugins: [mockTauriForE2e(), vue()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
      "@shared": fileURLToPath(new URL("./shared", import.meta.url)),
    },
  },
  clearScreen: false,
  server: {
    port: process.env.CONVERTZZ_E2E === "1" ? 1422 : 1420,
    strictPort: true,
    host: process.env.CONVERTZZ_E2E === "1" ? "127.0.0.1" : host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**", "**/ConvertZZ/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    target: "es2022",
    chunkSizeWarningLimit: 1500,
    minify: process.env.TAURI_ENV_DEBUG ? false : "esbuild",
    sourcemap: Boolean(process.env.TAURI_ENV_DEBUG),
  },
});
