import { expect } from "@playwright/test";

export async function openWorkspace(page, title) {
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: title, exact: true }).click();
  await expect(page.getByRole("main", { name: title, exact: true })).toBeVisible();
}

export async function openPowerArea(page, area) {
  await openWorkspace(page, "电源与档案");
  const option = page.getByRole("radiogroup", { name: "电源区域", exact: true }).getByRole("radio", { name: area, exact: true });
  await option.check();
  await expect(option).toBeChecked();
}
