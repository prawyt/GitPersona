import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { createRequire } from "node:module";

// Single source of truth for the version shown in the UI. Reading it here
// keeps the header from drifting out of step with the released build, which
// it previously did by carrying a hardcoded literal.
const { version } = createRequire(import.meta.url)("./package.json") as {
  version: string;
};

export default defineConfig({
  plugins: [react()],
  define: { __APP_VERSION__: JSON.stringify(version) },
  clearScreen: false,
  server: { strictPort: true, host: "127.0.0.1" },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  test: {
    environment: "jsdom",
    setupFiles: "./src/test/setup.ts",
    pool: "forks",
    poolOptions: { forks: { singleFork: true } },
  },
});
