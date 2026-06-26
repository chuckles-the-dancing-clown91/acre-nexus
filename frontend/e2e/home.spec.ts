import { test, expect } from "@playwright/test";

/**
 * Smoke test for the public homepage.
 *
 * REQUIRES a running stack: `npm run dev` (frontend) AND the backend API (so the
 * public listings/theme endpoints respond). It is NOT run in the default unit
 * test pass — invoke it explicitly with `npm run test:e2e`. With no server up,
 * `npx playwright test --list` will still resolve this spec.
 */
test.describe("public homepage", () => {
  test("loads and renders the listings site shell", async ({ page }) => {
    await page.goto("/");

    // The site header / brand should be present on the public site.
    await expect(page).toHaveTitle(/.+/);
    await expect(page.locator("body")).toBeVisible();
  });
});
