import { expect, test } from "@playwright/test";

test("signs into the protected workspace and signs back out", async ({ page }) => {
  await login(page);

  await expect(page.getByText("Playwright Operator")).toBeVisible();
  await expect(page.getByText("Slice 30 browser automation").first()).toBeVisible();

  await page.getByRole("button", { name: "Sign Out" }).click();
  await expect(page.getByTestId("auth-form")).toBeVisible();
});

test.describe("mobile details interactions", () => {
  test.use({ viewport: { width: 390, height: 844 } });

  test("opens and dismisses the mobile drawer and details sheet backdrops", async ({ page }) => {
    await login(page);

    await page.getByRole("button", { name: "Threads", exact: true }).click();
    await expect
      .poll(() => openState(page.getByTestId("sidebar-backdrop")))
      .toBe(true);

    await page.getByTestId("sidebar-backdrop").click({ position: { x: 10, y: 10 } });
    await expect
      .poll(() => openState(page.getByTestId("sidebar-backdrop")))
      .toBe(false);

    await page.getByRole("button", { name: "Details" }).click();
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

test("refreshes the job presentation from Job Update to Job Complete after realtime delivery", async ({
  page,
}) => {
  await login(page, { password: "slice30-realtime" });

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

async function login(page, { password = "slice30" } = {}) {
  await page.goto("/");
  await expect(page.getByTestId("auth-form")).toBeVisible();
  await page.getByTestId("auth-password").fill(password);
  await page.getByTestId("auth-submit").click();
  await expect(page.getByTestId("thread-composer")).toBeVisible();
}

async function openState(locator) {
  return locator.evaluate((element) => element.classList.contains("open"));
}
