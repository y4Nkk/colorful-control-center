import { expect } from "@playwright/test";

export const profilePanel = page => page.getByRole("region", { name: "档案预览", exact: true });

export async function showProfileSettings(page) {
  const button = profilePanel(page).getByRole("button", { name: "查看全部设置", exact: true });
  await expect(button).toBeVisible();
  if (await button.getAttribute("aria-expanded") === "false") await button.click();
  return profilePanel(page).getByLabel("档案参数预览", { exact: true });
}

export async function showProfileMenu(page) {
  const panel = profilePanel(page);
  const button = panel.getByRole("button", { name: "更多", exact: true });
  await expect(button).toBeVisible();
  if (await button.getAttribute("aria-expanded") === "false") await button.click();
  return panel;
}

export const profileSaveDialog = page => page.getByRole("dialog", { name: "保存当前配置", exact: true });
export const saveProfileButton = page => profileSaveDialog(page).getByRole("button", { name: "保存档案", exact: true });

export async function openProfileSave(page, name, groups = ["性能与散热"]) {
  await page.getByRole("button", { name: "保存当前配置", exact: true }).click();
  const editor = profileSaveDialog(page);
  await editor.getByRole("textbox", { name: "档案名", exact: true }).fill(name);
  for (const group of ["性能与散热", "显示与显卡", "灯光", "充电与供电", "设备功能"]) {
    await editor.getByRole("checkbox", { name: group, exact: true }).setChecked(groups.includes(group));
  }
}

export async function openProfileResults(page) {
  await profilePanel(page).getByRole("button", { name: /^执行明细/ }).click();
  return page.getByRole("dialog", { name: /^执行明细/ }).getByLabel("执行结果");
}

export async function openStartupParameters(page) {
  const review = page.getByRole("region", { name: "启动档案 · 授权启动应用档案", exact: true });
  await review.getByRole("button", { name: "启动档案参数", exact: true }).click();
  return page.getByRole("dialog", { name: "启动档案参数", exact: true });
}
