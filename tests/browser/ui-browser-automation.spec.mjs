import { expect, test } from "@playwright/test";

test("signs into the protected workspace and signs back out", async ({ page }) => {
  await login(page);

  await expect(page.getByTestId("operator-chip")).toHaveText("Playwright Admin · Admin");
  await expect(page.getByText("Slice 30 browser automation").first()).toBeVisible();

  await page.getByTestId("signout-button").click();
  await expect(page.getByTestId("auth-form")).toBeVisible();
});

test("viewer sessions stay read-only in the workspace shell", async ({ page }) => {
  await login(page, { username: "viewer", password: "slice32-viewer" });

  await expect(page.getByTestId("operator-chip")).toHaveText("Viewer User · Viewer");
  await expect(page.getByPlaceholder("Message Elowen")).toBeDisabled();
  await expect(page.getByRole("button", { name: "Create Thread" })).toBeDisabled();
});

test("operator sessions can work in chat but cannot resolve approvals", async ({ page }) => {
  await login(page, { username: "operator", password: "slice32-operator" });

  await expect(page.getByTestId("operator-chip")).toHaveText("Operator User · Operator");
  await expect(page.getByPlaceholder("Message Elowen")).toBeEnabled();
  await expect(page.getByRole("button", { name: "Approve And Push" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Reject Push" })).toHaveCount(0);
});

test.describe("mobile details interactions", () => {
  test.use({ viewport: { width: 390, height: 844 } });

  test("opens and dismisses the mobile drawer and details sheet backdrops", async ({ page }) => {
    await login(page);

    await page.getByTestId("mobile-threads").click();
    await expect
      .poll(() => openState(page.getByTestId("sidebar-backdrop")))
      .toBe(true);

    await page.getByTestId("sidebar-backdrop").click({ position: { x: 10, y: 10 } });
    await expect
      .poll(() => openState(page.getByTestId("sidebar-backdrop")))
      .toBe(false);

    await page.getByTestId("mobile-details").click();
    await expect
      .poll(() => openState(page.getByTestId("context-backdrop")))
      .toBe(true);
    await expect
      .poll(() => openState(page.getByTestId("context-sheet")))
      .toBe(true);

    await page.getByTestId("context-backdrop").click({ position: { x: 10, y: 10 } });
    await expect
      .poll(() => openState(page.getByTestId("context-backdrop")))
      .toBe(false);
    await expect
      .poll(() => openState(page.getByTestId("context-sheet")))
      .toBe(false);
  });
});

test("keeps the composer pinned while the message pane owns scroll", async ({ page }) => {
  await login(page);

  const messagePane = page.getByTestId("message-pane");
  const composer = page.getByTestId("thread-composer");

  const initialComposerBox = await composer.boundingBox();
  expect(initialComposerBox).not.toBeNull();

  await messagePane.evaluate((element) => {
    element.scrollTop = 0;
  });
  await messagePane.evaluate((element) => {
    element.style.scrollBehavior = "auto";
    element.scrollTop = Math.min(1_600, element.scrollHeight);
  });
  await page.waitForTimeout(100);

  const scrollState = await messagePane.evaluate((element) => ({
    scrollTop: element.scrollTop,
    scrollHeight: element.scrollHeight,
    clientHeight: element.clientHeight,
    pageScrollY: window.scrollY,
  }));

  expect(scrollState.scrollHeight).toBeGreaterThan(scrollState.clientHeight);
  expect(scrollState.scrollTop).toBeGreaterThan(0);
  expect(scrollState.pageScrollY).toBe(0);

  const nextComposerBox = await composer.boundingBox();
  expect(nextComposerBox).not.toBeNull();

  const bottomShift = Math.abs(
    initialComposerBox.y + initialComposerBox.height - (nextComposerBox.y + nextComposerBox.height),
  );

  expect(bottomShift).toBeLessThan(6);
  expect(nextComposerBox.y + nextComposerBox.height).toBeLessThanOrEqual(page.viewportSize().height);
});

test("shows created job activity in the thread when the backend only exposes job events", async ({ page }) => {
  await login(page, { username: "admin", password: "slice30-created" });

  await expect(page.getByText("Job Update").first()).toBeVisible();
  await expect(
    page.getByText("Created job `job-030` for Repository elowen-ui on device `laptop-edge-01`."),
  ).toBeVisible();
  await expect(page.getByText("Elowen is checking for an available edge device now.")).toBeVisible();
});

test("refreshes the job presentation from Job Update to Job Complete after realtime delivery", async ({
  page,
}) => {
  await login(page, { username: "admin", password: "slice30-realtime" });

  await expect(page.getByText("Job Update").first()).toBeVisible();
  await expect(
    page.getByText("Runner is still applying the requested UI automation changes."),
  ).toBeVisible();

  await expect(page.getByText("Job Complete").first()).toBeVisible();
  await expect(
    page.getByText(
      "Browser automation summary ready. The UI shell now has deterministic browser coverage.",
    ),
  ).toBeVisible();
});

test("does not use timer-driven polling after realtime catch-up", async ({ page }) => {
  test.setTimeout(45_000);

  const automaticGetCounts = new Map();
  const watchedPaths = new Set([
    "/api/v1/threads",
    "/api/v1/jobs",
    "/api/v1/devices",
    "/api/v1/repositories",
  ]);

  page.on("request", (request) => {
    const url = new URL(request.url());
    if (request.method() === "GET" && watchedPaths.has(url.pathname)) {
      automaticGetCounts.set(url.pathname, (automaticGetCounts.get(url.pathname) ?? 0) + 1);
    }
  });

  await login(page);
  await expect(page.getByText("Realtime connected").first()).toBeVisible();
  automaticGetCounts.clear();

  await page.waitForTimeout(31_000);

  expect([...automaticGetCounts.entries()]).toEqual([]);
});

test("shows trust state separately from device freshness in the details panel", async ({ page }) => {
  await login(page);

  await page.getByRole("button", { name: "Show Details" }).click();
  await page.getByRole("button", { name: "Devices" }).click();

  const deviceTrustList = page.getByTestId("device-trust-list");
  await expect(deviceTrustList).toBeVisible();
  await expect(deviceTrustList.getByRole("heading", { name: "Laptop Edge" })).toBeVisible();
  await expect(deviceTrustList.getByText("Trusted", { exact: true }).first()).toBeVisible();
  await expect(deviceTrustList.getByRole("heading", { name: "Travel Edge" })).toBeVisible();
  await expect(deviceTrustList.getByText("Needs Attention", { exact: true }).first()).toBeVisible();
  await expect(deviceTrustList.getByRole("heading", { name: "Retired Edge" })).toBeVisible();
  await expect(deviceTrustList.getByText("Revoked", { exact: true }).first()).toBeVisible();
  await expect(deviceTrustList.getByText(/Seen: 2026-04-15T14:39:00Z/)).toBeVisible();
  await expect(deviceTrustList.getByText(/Trusted: 2026-04-15T13:55:00Z/)).toBeVisible();
});

test("marks blocked edge options with trust guidance in manual dispatch", async ({ page }) => {
  await login(page);

  await page.getByRole("button", { name: "Show Details" }).click();
  await page.getByRole("button", { name: "Manual Job" }).click();

  const trustCard = page.getByTestId("manual-job-device-trust");
  await expect(trustCard).toContainText("Laptop Edge");
  await expect(trustCard).toContainText("Trusted");

  const manualJobPanel = page.getByTestId("context-tab-manual");
  const blockedOption = manualJobPanel.locator("select").first().locator("option[value='travel-edge-02']");
  await expect(blockedOption).toHaveText(/Travel Edge .* Needs Attention .* Dispatch blocked/);
  await expect(blockedOption).toBeDisabled();
});

test("lets operators retry jobs that failed because the edge was unavailable", async ({ page }) => {
  await login(page, { username: "admin", password: "slice43-edge-unavailable" });

  await page.getByRole("button", { name: "Show Details" }).click();
  await page.getByRole("button", { name: "Selected Job" }).click();

  await expect(page.getByTestId("context-tab-job").getByText("Failure class edge_unavailable")).toBeVisible();
  await page.getByRole("button", { name: "Retry Job" }).click();

  await expect(page.getByText("Retried job job-030; status is dispatched.").first()).toBeVisible();
  await expect(page.getByTestId("context-tab-job").getByText("dispatched").first()).toBeVisible();
});

async function login(page, { username = "admin", password = "slice30" } = {}) {
  await page.goto("/");
  await expect(page.getByTestId("auth-form")).toBeVisible();
  await page.getByTestId("auth-username").fill(username);
  await page.getByTestId("auth-password").fill(password);
  await page.getByTestId("auth-submit").click();
  await expect(page.getByTestId("thread-composer")).toBeVisible();
}

async function openState(locator) {
  return locator.evaluate((element) => element.classList.contains("open"));
}
