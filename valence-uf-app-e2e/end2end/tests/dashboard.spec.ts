import { test, expect, seedAuth, waitForHydrated, expandShellNav } from "./fixtures";

test.use({ timezoneId: "UTC" });

test.describe("pw-valence-dashboard", () => {
  test("pw-dashboard-happy-load", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/valence", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("valence-dashboard-page")).toBeVisible({
      timeout: 60_000,
    });
    await expandShellNav(page);
    await expect(page.getByTestId("nav-my-data")).toBeVisible({ timeout: 30_000 });
    await expect(page.getByTestId("nav-schemas")).toBeVisible();
    await expect(page.getByTestId("nav-traits")).toBeVisible();
    await expect(page.getByTestId("nav-iters")).toBeVisible();
    await expect(page.getByTestId("nav-deletions")).toBeVisible();
  });

  test("pw-dashboard-nav-schemas-happy", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/valence", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expandShellNav(page);
    await page.getByTestId("nav-schemas").click();
    await expect(page.getByTestId("schema-index-page")).toBeVisible({
      timeout: 60_000,
    });
  });

  test("pw-dashboard-my-data-card-opens-your-data", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/valence", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const card = page.getByTestId("dashboard-my-data-card");
    await expect(card.getByTestId("dashboard-my-data-summary")).toContainText(/You own \d+ records? in \d+ kinds?\./, {
      timeout: 60_000,
    });
    await card.getByRole("link", { name: "Open your data" }).click();
    await expect(page).toHaveURL(/\/valence\/me$/);
    await expect(page.getByTestId("valence-my-data-page")).toBeVisible({ timeout: 60_000 });
  });

  test("pw-dashboard-search-no-match-stays", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/valence", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const input = page.getByTestId("valence-dashboard-search").locator("input");
    await input.fill("td_01HV9ZZZZZ");
    await input.press("Enter");
    const noMatch = page.getByTestId("dashboard-search-no-match");
    await expect(noMatch).toContainText("Nothing matches td_01HV9ZZZZZ", { timeout: 60_000 });
    await expect(page).toHaveURL(/\/valence$/);
    await expect(noMatch.getByRole("link")).toHaveAttribute("href", "/valence/deletions");
  });

  test("pw-dashboard-search-schema-redirects", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto("/valence", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const input = page.getByTestId("valence-dashboard-search").locator("input");
    await input.fill(seeded.fixtures.schema_name);
    await input.press("Enter");
    await expect(page).toHaveURL(
      new RegExp(`/valence/schema/${seeded.fixtures.schema_name}$`),
      { timeout: 60_000 },
    );
  });

  test("pw-dashboard-search-expired-session-shows-error", async ({ page, context }) => {
    await seedAuth(page, "admin");
    await page.goto("/valence", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("dashboard-my-data-card")).toBeVisible({ timeout: 60_000 });
    await context.clearCookies();
    const input = page.getByTestId("valence-dashboard-search").locator("input");
    await input.fill("counter");
    await input.press("Enter");
    await expect(page.getByTestId("dashboard-search-error")).toBeVisible({ timeout: 60_000 });
    await expect(page).toHaveURL(/\/valence$/);
  });

  test("pw-dashboard-metrics-permission-message", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.route(/\/api\/get_dashboard_platform_headline/, (route) =>
      route.fulfill({
        status: 500,
        contentType: "text/plain",
        body: "ServerError|permission: `spectra.query.valence_db_reads` is required to query this table",
      }),
    );
    await page.goto("/valence", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    // Changing the range refetches the headline in the browser, through the route.
    await page.getByTestId("dashboard-range-15m").getByRole("button").click();
    const denied = page
      .getByTestId("dashboard-platform-headline-card")
      .getByTestId("dashboard-permission-denied");
    await expect(denied).toContainText("Platform metrics need a Spectra permission", {
      timeout: 60_000,
    });
    await expect(denied).toContainText("spectra.query");
  });
});
