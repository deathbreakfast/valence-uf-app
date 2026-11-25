import { readFile } from "node:fs/promises";
import { test, expect, seedAuth, waitForHydrated, expandShellNav } from "./fixtures";

// Fixture records `admin` owns (valence-uf-app-e2e/src/e2e_valence.rs).
const MY_TODO_IDS = ["e2e-my-todo-1", "e2e-my-todo-2"];
const MY_PROJECT_ID = "e2e-my-project-1";

test.use({ timezoneId: "UTC" });

test.describe("pw-valence-my-data", () => {
  test("pw-my-data-nav-opens-page", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/valence", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expandShellNav(page);
    await page.getByTestId("nav-my-data").click();
    await expect(page).toHaveURL(/\/valence\/me$/);
    await expect(page.getByTestId("valence-my-data-page")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId("valence-my-data-page")).toContainText(
      "Everything Valence stores that you own",
    );
  });

  test("pw-my-data-lists-owned-records-by-schema", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/valence/me", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);

    const todo = page.getByTestId("my-data-schema-todo");
    await expect(todo).toBeVisible({ timeout: 60_000 });
    await expect(todo.getByText("todo · 2")).toBeVisible();
    await expect(todo.getByTestId("my-data-why-kept")).toContainText("Why it's kept");
    await expect(todo.getByTestId("my-data-why-kept")).not.toContainText("**");
    await expect(todo.getByTestId("my-data-retention")).not.toBeEmpty();
    for (const id of MY_TODO_IDS) {
      await expect(todo.getByRole("link", { name: id })).toHaveAttribute(
        "href",
        `/valence/schema/todo/id/${id}`,
      );
    }

    const project = page.getByTestId("my-data-schema-project");
    await expect(project.getByText("project · 1")).toBeVisible();
    await expect(project.getByRole("link", { name: MY_PROJECT_ID })).toBeVisible();

    await expect(page.getByTestId("my-data-stats")).toContainText("Records you own");
    await expect(page.getByTestId("my-data-account")).toContainText("Deleting your account");
    await expect(page.getByRole("link", { name: "Open Account Settings" })).toHaveAttribute(
      "href",
      "/user/account-settings",
    );
  });

  test("pw-my-data-download-has-only-own-records", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/valence/me", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("my-data-schema-todo")).toBeVisible({ timeout: 60_000 });

    const downloadPromise = page.waitForEvent("download");
    await page.getByTestId("my-data-download").getByRole("button").click();
    const download = await downloadPromise;
    expect(download.suggestedFilename()).toBe("valence-your-data.json");
    const path = await download.path();
    const doc = JSON.parse(await readFile(path, "utf8")) as {
      user: string;
      schemas: Record<string, Array<{ id: unknown }>>;
    };
    expect(doc.user).toContain("admin");
    const todoIds = (doc.schemas.todo ?? []).map((r) => JSON.stringify(r.id));
    for (const id of MY_TODO_IDS) {
      expect(todoIds.some((t) => t.includes(id))).toBeTruthy();
    }
    expect(JSON.stringify(doc.schemas.project ?? [])).toContain(MY_PROJECT_ID);
    // The outsider's user row is never in admin's copy.
    expect(JSON.stringify(doc)).not.toContain("outsider");
  });

  test("pw-my-data-empty-for-user-with-none", async ({ page }) => {
    await seedAuth(page, "outsider");
    await page.goto("/valence/me", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("my-data-empty")).toContainText(
      "Valence holds no records that you own",
      { timeout: 60_000 },
    );
  });

  test("pw-my-data-error-offers-try-again", async ({ page }) => {
    await seedAuth(page, "admin");
    let failed = false;
    await page.route(/\/api\/get_my_data/, async (route) => {
      if (!failed) {
        failed = true;
        await route.fulfill({ status: 500, body: "ServerError|io: valence unavailable" });
        return;
      }
      await route.continue();
    });
    // SSR would answer a direct visit itself; open the page from the dashboard so
    // the server fn runs in the browser and goes through the route.
    await page.goto("/valence", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await page.getByTestId("dashboard-my-data-card").getByRole("link", { name: "Open your data" }).click();
    const error = page.getByTestId("my-data-error");
    await expect(error).toBeVisible({ timeout: 60_000 });
    await error.getByRole("button", { name: "Try again" }).click();
    await expect(page.getByTestId("my-data-schema-todo")).toBeVisible({ timeout: 60_000 });
  });

  test("pw-my-data-mobile-390", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await seedAuth(page, "admin");
    await page.goto("/valence/me", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const todo = page.getByTestId("my-data-schema-todo");
    await expect(todo.getByTestId("my-data-records-list")).toBeVisible({ timeout: 60_000 });
    await expect(todo.getByTestId("my-data-records-table")).toHaveCount(0);
    const stats = page.getByTestId("my-data-stats");
    const width = await stats.evaluate((el) => el.scrollWidth <= el.clientWidth + 1);
    expect(width).toBeTruthy();
  });
});
