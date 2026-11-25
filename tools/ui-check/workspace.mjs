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

const hotkeys = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });

export async function editHotkeys(page) {
  await hotkeys(page).getByRole("button", { name: "配置热键", exact: true }).click();
  return page.getByRole("dialog", { name: "配置热键与屏显", exact: true });
}

export function finishHotkeyEdit(editor) {
  return editor.getByRole("button", { name: "完成编辑", exact: true }).click();
}

export async function openHotkeyDetails(page) {
  await hotkeys(page).getByRole("button", { name: "详情", exact: true }).click();
  return page.getByRole("dialog", { name: "热键状态与诊断", exact: true });
}

export const hotkeyRestoreButton = page => hotkeys(page).getByRole("button", { name: "恢复原厂热键与屏显", exact: true });
export const hotkeyRestoreDialog = page => page.getByRole("dialog", { name: "恢复原厂热键与屏显", exact: true });

export async function previewHotkeyRestore(page, name) {
  const details = page.getByRole("dialog", { name: "热键状态与诊断", exact: true });
  if (await details.isVisible()) await details.getByRole("button", { name: "关闭弹窗", exact: true }).click();
  await hotkeyRestoreButton(page).click();
  const dialog = hotkeyRestoreDialog(page);
  await expect(dialog).toBeVisible();
  if (name && await dialog.getByRole("group", { name: "恢复项目", exact: true }).count()) {
    await dialog.getByRole("checkbox", { name, exact: true }).check();
  }
  return dialog;
}
