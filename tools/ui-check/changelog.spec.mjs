import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await openWorkspace(page, "设置");
});

test("关于在版本号旁打开离线日志，当前与历史版本可切换", async ({ page }) => {
  const about = page.getByRole("region", { name: "关于", exact: true });
  const trigger = about.getByRole("button", { name: "更新日志", exact: true });
  await expect(about.getByRole("status")).toHaveText("v1.0.1");
  await trigger.click();

  const dialog = page.getByRole("dialog", { name: "更新日志" });
  const version = dialog.getByRole("combobox", { name: "日志版本" });
  await expect(version).toHaveText("v1.0.1（当前版本）");
  await expect(dialog.getByRole("article")).toContainText("散热页合并为一个「风扇设置」");
  await version.click();
  await dialog.getByRole("option", { name: "v1.0.0", exact: true }).click();
  await expect(dialog.getByRole("heading", { name: "正式首发" })).toBeVisible();
  await expect(dialog.getByRole("link", { name: "GitHub Releases" })).toBeVisible();

  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(trigger).toBeFocused();
  await trigger.click();
  await expect(version).toHaveText("v1.0.1（当前版本）");
});

test("更新日志资源读取失败后可在弹窗内重试", async ({ page }) => {
  const file = url => /\/CHANGELOG\.md$/.test(url.pathname) && !url.search;
  const fail = route => route.abort();
  await page.route(file, fail);
  await page.getByRole("button", { name: "更新日志", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "更新日志" });
  await expect(dialog.getByRole("alert")).toHaveText("读取更新日志失败。");
  await page.unroute(file, fail);
  await dialog.getByRole("button", { name: "重试", exact: true }).click();
  await expect(dialog.getByRole("combobox", { name: "日志版本" })).toHaveText("v1.0.1（当前版本）");
});
