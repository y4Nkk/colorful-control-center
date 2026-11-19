import { restoreDevice } from "./device-settings.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

test("显示试用使用后端剩余时长，保留后仍可恢复初始模式", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/"); await openWorkspace(page, "设备配置");
  const panel = page.getByRole("region", { name: "屏幕显示", exact: true });
  await panel.getByRole("combobox", { name: "目标刷新率" }).click();
  await page.getByRole("option", { name: "144 Hz", exact: true }).click();
  await expect(panel.getByRole("status")).toContainText(/试用 144 Hz · \d+ 秒后自动恢复/);
  await expect(page.getByRole("region", { name: "页面操作", exact: true })).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(c => c.command === "apply_display_refresh").length)).toBe(1);
  await page.evaluate(() => window.__uiFixture.displayExpiresAt = Date.now() + 8000);
  await expect(panel.getByRole("status")).toContainText(/· [1-8] 秒后/);
  await panel.getByRole("button", { name: "保留刷新率", exact: true }).click();
  await expect(panel.getByRole("status")).toHaveCount(0);
  await panel.getByRole("combobox", { name: "目标刷新率" }).click();
  await page.getByRole("option", { name: "60 Hz", exact: true }).click();
  await expect(panel.getByRole("status")).toContainText("试用 60 Hz");
  await page.evaluate(() => window.__uiFixture.displayExpiresAt = Date.now() - 1);
  await expect(panel.getByRole("status")).toHaveCount(0);
  await expect(panel).toContainText("144 Hz");
  await restoreDevice(panel);
  await expect(panel).toContainText("60 Hz");
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
