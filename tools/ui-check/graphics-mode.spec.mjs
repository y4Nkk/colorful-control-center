import { availableAction, configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "显示与显卡"); });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(c => c.command === command).length, command);
async function choose(page, label) {
  await page.getByRole("combobox", { name: "目标显卡模式", exact: true }).click();
  await page.getByRole("option", { name: label, exact: true }).click();
}
test("显卡选择保持旧报告，独立确认重启后再显示确认结果", async ({ page }) => {
  const panel = page.getByRole("region", { name: "显卡模式", exact: true });
  await choose(page, "仅独显"); expect(await calls(page, "apply_graphics_mode")).toBe(0);
  await panel.screenshot({ path: "target/ui-check/graphics-mode-preview.png" });
  await openConfigurationReview(page, "显卡模式");
  await choose(page, "动态切换"); await expect(availableAction(page, "显卡模式")).toBeVisible();
  await choose(page, "仅独显");
  await openConfigurationReview(page, "显卡模式");
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  await expect(panel.getByRole("status")).toContainText("重启后确认");
  await expect(panel).toContainText(/固件报告\s*混合显卡/);
  expect(await calls(page, "reboot_for_graphics_mode")).toBe(0);
  await panel.getByRole("button", { name: "重启 Windows", exact: true }).click();
  expect(await calls(page, "reboot_for_graphics_mode")).toBe(0);
  await panel.getByRole("button", { name: "已保存工作，确认重启 Windows", exact: true }).click();
  expect(await calls(page, "reboot_for_graphics_mode")).toBe(1);
  await page.evaluate(() => { window.__uiFixture.graphicsMode.current.mode = 2; window.__uiFixture.graphicsMode.lastRequest.verification = "confirmed"; });
  await panel.getByRole("button", { name: "重新读取显卡模式", exact: true }).click();
  await expect(panel.getByRole("status")).toContainText("重启后已确认");
  await openConfigurationReview(page, "显卡模式", true);
  await configurationDialog(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(panel).toContainText(/固件报告\s*仅独显/);
  await expect(panel.getByRole("status")).toContainText("混合显卡");
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
test("发送失败保留草稿，未发送记录时不开放重启", async ({ page }) => {
  const panel = page.getByRole("region", { name: "显卡模式", exact: true });
  await choose(page, "仅独显");
  await page.evaluate(() => window.__uiFixture.graphicsModeFault = "选择记录保存失败，未发送硬件命令");
  await openConfigurationReview(page, "显卡模式");
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("记录保存失败");
  await expect(panel.getByRole("combobox", { name: "目标显卡模式", exact: true })).toHaveText("仅独显");
  await expect(panel.getByRole("button", { name: "重启 Windows", exact: true })).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.graphicsReboots)).toBe(0);
});
test("固件候选范围与未完成操作状态都影响可操作入口", async ({ page }) => {
  await page.evaluate(() => {
    const p = window.__uiFixture.graphicsMode; p.current.availableMask = 6;
    p.lastRequest = { initialMode: 3, target: 2, verification: "uncertain" }; p.canRestore = true;
  });
  const panel = page.getByRole("region", { name: "显卡模式", exact: true });
  await panel.getByRole("button", { name: "重新读取显卡模式", exact: true }).click();
  await expect(panel.getByRole("status")).toContainText("上次操作未完成");
  await expect(panel.getByRole("button", { name: "重启 Windows", exact: true })).toHaveCount(0);
  await panel.getByRole("combobox", { name: "目标显卡模式", exact: true }).click();
  await expect(page.getByRole("option", { name: "仅集显", exact: true })).toHaveCount(0);
  await expect(page.getByRole("option", { name: "动态切换", exact: true })).toHaveCount(0);
  await page.getByRole("option", { name: "仅独显", exact: true }).click();
  await expect(availableAction(page, "显卡模式")).toBeEnabled();
});
