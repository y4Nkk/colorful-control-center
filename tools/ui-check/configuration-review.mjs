import { expect } from "@playwright/test";

export const pageActions = page => page.locator(".ccc-page-actions, [aria-label='页面操作']");
const moduleAction = (page, scope, restore = false) =>
  page.locator(`[data-module-${restore ? "restore" : "action"}${scope ? "=" + JSON.stringify(scope) : ""}]:visible`);
export const changeButton = page => moduleAction(page).locator("[data-configuration-apply]");
export const restoreButton = (page, scope) => moduleAction(page, scope, true).locator(":scope > button");
export const configurationView = (page, restore = false) => restore
  ? page.locator("dialog[open]").filter({ has: page.getByRole("button", { name: "确认恢复", exact: true }) })
  : moduleAction(page);

export function availableAction(page, scope, restore = false) {
  return restore ? restoreButton(page, scope) : moduleAction(page, scope).locator("[data-configuration-apply]");
}

export async function previewConfiguration(page, scope, restore = false) {
  if (restore) await restoreButton(page, scope).click();
  const view = restore ? configurationView(page, true) : moduleAction(page, scope);
  await expect(view).toBeVisible();
  return view;
}

export async function submitConfiguration(view, label) {
  const button = label === "确认恢复" || label && label !== "应用更改"
    ? view.getByRole("button", { name: label, exact: true }) : view.locator("[data-configuration-apply]");
  await button.click();
  if (label === "确认恢复") await expect(view).toHaveCount(0);
  else await expect(view.locator("[data-configuration-apply]:disabled")).toHaveCount(0);
}
