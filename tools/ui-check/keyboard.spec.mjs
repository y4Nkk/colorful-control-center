import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "设备配置"); });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => c.command === "apply_keyboard_settings").map(c => c.args.request));

test("键盘开关逐项提交，保留其他字段并可直接切回", async ({ page }) => {
  const panel = page.getByRole("region", { name: "键盘功能", exact: true });
  const fnLock = panel.getByRole("switch", { name: "Fn 锁", exact: true });
  const winLock = panel.getByRole("switch", { name: "Win 锁", exact: true });
  await expect(panel.getByRole("switch", { name: "Fn / Win 交换", exact: true })).toBeDisabled();
  await expect(panel.getByText("此设备未提供", { exact: true })).toBeVisible();
  await expect(panel.getByText(/^(开启|关闭)$/)).toHaveCount(0);
  expect(await writes(page)).toEqual([]);
  await fnLock.click();
  await expect(fnLock).toBeChecked();
  await winLock.click();
  await expect(winLock).toBeChecked();
  const requests = await writes(page);
  expect(requests.map(request => request.values)).toEqual([
    { fnLocked: true, winLocked: false, fnWinSwap: null },
    { fnLocked: true, winLocked: true, fnWinSwap: null },
  ]);
  expect(requests[1].expectedRevision).not.toBe(requests[0].expectedRevision);
  await expect(panel.getByRole("button", { name: "恢复初始设置", exact: true })).toHaveCount(0);
  await fnLock.click();
  await winLock.click();
  await expect(fnLock).not.toBeChecked();
  await expect(winLock).not.toBeChecked();
  await panel.screenshot({ path: "target/ui-check/keyboard-settings.png" });
  await page.setViewportSize({ width: 360, height: 720 });
  expect(await panel.evaluate(el => el.getBoundingClientRect().right <= innerWidth)).toBe(true);
  await panel.screenshot({ path: "target/ui-check/keyboard-settings-narrow.png" });
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("键盘写入失败显示回读值，支持在原控件重试", async ({ page }) => {
  const panel = page.getByRole("region", { name: "键盘功能", exact: true });
  const fnLock = panel.getByRole("switch", { name: "Fn 锁", exact: true });
  await page.evaluate(() => window.__uiFixture.keyboardFault = "隔离保存失败，已补偿");
  await fnLock.click();
  await expect(panel.getByRole("alert")).toContainText("隔离保存失败");
  await expect(fnLock).not.toBeChecked();
  expect(await page.evaluate(() => window.__uiFixture.keyboardSettings.current.fnLocked)).toBe(false);
  await fnLock.click();
  await expect(fnLock).toBeChecked();
  expect(await writes(page)).toHaveLength(2);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
