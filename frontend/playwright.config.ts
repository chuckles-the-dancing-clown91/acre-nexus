import { defineConfig, devices } from "@playwright/test";

/**
 * Playwright e2e config.
 *
 * The Chromium executable is preinstalled in this environment, so we point
 * Playwright at it directly instead of running `playwright install` (browser
 * downloads are disabled). Override with PLAYWRIGHT_CHROMIUM if needed.
 *
 * These tests need the dev server (`npm run dev`) and the backend running; CI
 * should start them via the `webServer` block below (disabled by default so
 * `playwright test --list` resolves without a server).
 */
const CHROMIUM_EXECUTABLE =
  process.env.PLAYWRIGHT_CHROMIUM ?? "/opt/pw-browsers/chromium";

export default defineConfig({
  testDir: "./e2e",
  testMatch: "**/*.spec.ts",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: "list",
  use: {
    baseURL: process.env.E2E_BASE_URL ?? "http://localhost:3000",
    trace: "on-first-retry",
  },
  projects: [
    {
      name: "chromium",
      use: {
        ...devices["Desktop Chrome"],
        launchOptions: {
          // Use the preinstalled Chromium rather than a downloaded browser.
          executablePath: CHROMIUM_EXECUTABLE,
        },
      },
    },
  ],
  // Uncomment to have Playwright boot the dev server automatically. Left off so
  // `playwright test --list` works without a running server.
  // webServer: {
  //   command: "npm run dev",
  //   url: "http://localhost:3000",
  //   reuseExistingServer: !process.env.CI,
  // },
});
