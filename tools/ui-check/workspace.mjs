import { expect } from "@playwright/test";

export async function openWorkspace(page, title) {
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: title, exact: true }).click();
  await expect(page.getByRole("main", { name: title, exact: true })).toBeVisible();
}

export async function openPowerArea(page, area) {
  await openWorkspace(page, "电源与档案");
  const tab = page.getByRole("tablist", { name: "电源区域", exact: true }).getByRole("tab", { name: area, exact: true });
  await tab.click();
  await expect(tab).toHaveAttribute("aria-selected", "true");
}
