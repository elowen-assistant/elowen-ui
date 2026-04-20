import { expect, test } from "@playwright/test";

test("shows the execution draft as a visible handoff card", async ({ page }) => {
  await login(page);
  const messagePane = page.getByTestId("message-pane");

  await expect(messagePane.getByText("Execution Draft")).toBeVisible();
  await expect(messagePane.getByRole("heading", { name: "Polish chat transcript surfaces" })).toBeVisible();
  await expect(messagePane.locator(".draft-field").filter({ hasText: "Repository" })).toBeVisible();
  await expect(messagePane.getByRole("combobox").first()).toHaveValue("elowen-ui");
  await expect(messagePane.locator(".draft-field").filter({ hasText: "Base Branch" })).toBeVisible();
  await expect(
    messagePane.getByText(
      "Tighten the chat transcript, localize timestamps, keep operational result details behind disclosure by default, and preserve the pinned composer behavior.",
    ),
  ).toBeVisible();
  await expect(messagePane.getByRole("button", { name: "Dispatch Draft" })).toBeVisible();
});

test("keeps operational result details collapsed until the operator opens them", async ({ page }) => {
  await login(page);
  const messagePane = page.getByTestId("message-pane");

  await expect(
    messagePane.getByText(
      "Chat surface polish is ready for review. The transcript now separates activity from final results more clearly.",
    ),
  ).toBeVisible();
  await expect(messagePane.getByText("Operational Details")).toBeVisible();
  await expect(messagePane.getByText("Push approval is pending while the final branch waits for review.")).toBeHidden();

  await messagePane.getByText("Operational Details").click();
  await expect(messagePane.getByText("Push approval is pending while the final branch waits for review.")).toBeVisible();
});

test("submits the composer with Ctrl+Enter and shows the returned draft", async ({ page }) => {
  await login(page);

  const composer = page.getByPlaceholder("Message Elowen");
  await expect(page.getByText("Ctrl+Enter or Cmd+Enter to send")).toBeVisible();

  await composer.fill("Implement transcript timestamp polish in repo `elowen-ui`.");
  await composer.press("Control+Enter");

  const messagePane = page.getByTestId("message-pane");
  await expect(messagePane.getByRole("heading", { name: "Polish transcript timestamps" })).toBeVisible();
  await expect(messagePane.getByRole("combobox").first()).toHaveValue("elowen-ui");
  await expect(
    messagePane.locator(".draft-request pre").filter({
      hasText: "Implement transcript timestamp polish in repo `elowen-ui`.",
    }),
  ).toBeVisible();
});

async function login(page) {
  await page.goto("/");
  await page.getByTestId("auth-username").fill("admin");
  await page.getByTestId("auth-password").fill("slice31-draft");
  await page.getByTestId("auth-submit").click();
  await expect(page.getByTestId("thread-composer")).toBeVisible();
}
