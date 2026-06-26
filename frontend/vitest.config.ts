import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      // Mirror the tsconfig "@/*" -> src/* path alias.
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./vitest.setup.ts"],
    // e2e specs are Playwright's (*.spec.ts under e2e/), not Vitest's.
    exclude: ["node_modules", ".next", "e2e/**"],
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
