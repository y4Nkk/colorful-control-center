import { expect } from "@playwright/test";

export async function openWorkspace(page, title) {
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: title, exact: true }).click();
  await expect(page.getByRole("main", { name: title, exact: true })).toBeVisible();
}

export async function openSystemArea(page, area) {
  await openWorkspace(page, "系统");
  const option = page.getByRole("radiogroup", { name: "系统区域", exact: true }).getByRole("radio", { name: area, exact: true });
  await option.check();
  await expect(option).toBeChecked();
}
