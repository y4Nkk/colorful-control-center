import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与输入"); });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => c.command === "apply_keyboard_settings").length);

test("键盘草稿与确认绑定，保留其他状态并可恢复初始", async ({ page }) => {
  const panel = page.getByRole("region", { name: "键盘功能", exact: true });
  await expect(panel.getByRole("checkbox", { name: "Fn / Win 交换", exact: true })).toBeDisabled();
  await panel.getByRole("checkbox", { name: "Fn 锁", exact: true }).check();
  expect(await writes(page)).toBe(0);
  await panel.getByRole("button", { name: "应用键盘设置", exact: true }).click();
  expect(await writes(page)).toBe(0);
  await panel.getByRole("checkbox", { name: "Win 锁", exact: true }).check();
  await expect(panel.getByRole("button", { name: "应用键盘设置", exact: true })).toBeVisible();
  await panel.getByRole("button", { name: "应用键盘设置", exact: true }).click();
  await panel.getByRole("button", { name: "确认应用键盘设置？", exact: true }).click();
  await expect(panel.getByRole("button", { name: "应用键盘设置", exact: true })).toBeDisabled();
  expect(await writes(page)).toBe(1);
  expect(await page.evaluate(() => window.__uiFixture.keyboardSettings.current)).toEqual({ fnLocked: true, winLocked: true, fnWinSwap: null });
  await panel.getByRole("button", { name: "恢复键盘初始", exact: true }).click();
  await panel.getByRole("button", { name: "恢复本会话修改前的键盘设置？", exact: true }).click();
  await expect(panel.getByRole("checkbox", { name: "Fn 锁", exact: true })).not.toBeChecked();
  await expect(panel.getByRole("checkbox", { name: "Win 锁", exact: true })).not.toBeChecked();
  await panel.screenshot({ path: "target/ui-check/keyboard-settings.png" });
  await page.setViewportSize({ width: 360, height: 720 });
  expect(await panel.evaluate(el => el.getBoundingClientRect().right <= innerWidth)).toBe(true);
  await panel.screenshot({ path: "target/ui-check/keyboard-settings-narrow.png" });
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("键盘写入失败保留草稿与恢复入口", async ({ page }) => {
  const panel = page.getByRole("region", { name: "键盘功能", exact: true });
  await panel.getByRole("checkbox", { name: "Fn 锁", exact: true }).check();
  await page.evaluate(() => window.__uiFixture.keyboardFault = "隔离保存失败，已补偿");
  await panel.getByRole("button", { name: "应用键盘设置", exact: true }).click();
  await panel.getByRole("button", { name: "确认应用键盘设置？", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("隔离保存失败");
  await expect(panel.getByRole("checkbox", { name: "Fn 锁", exact: true })).toBeChecked();
  await expect(panel.getByRole("button", { name: "恢复键盘初始", exact: true })).toBeEnabled();
  expect(await page.evaluate(() => window.__uiFixture.keyboardSettings.current.fnLocked)).toBe(false);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
