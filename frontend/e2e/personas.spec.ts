import { test, expect, type Page } from "@playwright/test";

/**
 * End-to-end smoke across the three personas. REQUIRES the running stack
 * (`npm run dev` + the backend API with seed data). Run with `npm run test:e2e`.
 */

async function login(page: Page, email: string) {
  await page.goto("/login");
  await page.fill('input[type="email"]', email);
  await page.fill('input[type="password"]', "password");
  await page.getByRole("button", { name: "Sign in" }).click();
  await page.waitForURL("**/console", { timeout: 20000 });
}

test.describe("personas smoke", () => {
  test("public listings site renders real listings", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText(/homes available/i)).toBeVisible({
      timeout: 15000,
    });
  });

  test("tenant admin reaches the portfolio and properties", async ({ page }) => {
    await login(page, "jordan@northwind.com");
    await expect(page.getByText(/Good to see you/i)).toBeVisible({
      timeout: 15000,
    });
    await page.getByRole("link", { name: "Properties" }).first().click();
    await expect(
      page.getByRole("heading", { name: "Properties" })
    ).toBeVisible();
  });

  test("platform staff reaches the platform overview", async ({ page }) => {
    await login(page, "avery@acrehq.com");
    await page.goto("/console/platform");
    await expect(
      page.getByRole("heading", { name: /Platform overview/i })
    ).toBeVisible({ timeout: 15000 });
    await expect(page.getByText("Northwind Property Group")).toBeVisible();
  });
});
