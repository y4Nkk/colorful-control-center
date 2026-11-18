import { expect } from "@playwright/test";

export const pageActions = page => page.getByLabel("页面操作", { exact: true }).filter({ visible: true });
export const changeButton = page => pageActions(page).getByRole("button", { name: "预览更改", exact: true });
export const restoreButton = page => pageActions(page).getByRole("button", { name: "恢复设置", exact: true });
export const configurationDialog = (page, restore = false) => page.getByRole("dialog", {
  name: restore ? "恢复设置" : "预览更改", exact: true,
});

export function availableAction(page, scope, restore = false) {
  const list = [`[aria-description=${JSON.stringify(scope)}]`, `[aria-description^=${JSON.stringify(scope + "、")}]`,
    `[aria-description*=${JSON.stringify("、" + scope + "、")}]`, `[aria-description$=${JSON.stringify("、" + scope)}]`];
  return (restore ? restoreButton(page) : changeButton(page)).and(page.locator(list.join(",")));
}

export async function openConfigurationReview(page, scope, restore = false) {
  await (restore ? restoreButton(page) : changeButton(page)).click();
  const dialog = configurationDialog(page, restore);
  if (scope) {
    const choice = dialog.getByRole("button", { name: scope, exact: true });
    if (await choice.count()) await choice.click();
    await expect(dialog.getByRole("heading", { name: scope, exact: true })).toBeVisible();
  }
  return dialog;
}

export async function confirmConfiguration(dialog, label = "确认应用") {
  await dialog.getByRole("button", { name: label, exact: true }).click();
  await expect(dialog).toHaveCount(0);
}
