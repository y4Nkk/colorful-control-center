import { expect } from "@playwright/test";

export async function openWorkspace(page, title) {
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: title, exact: true }).click();
  await expect(page.getByRole("heading", { name: title, level: 1, exact: true })).toBeVisible();
}
