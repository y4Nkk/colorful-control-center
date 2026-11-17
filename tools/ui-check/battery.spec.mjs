import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "电池与节能"); });
const panel = page => page.getByRole("region", { name: "电池与充电", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(c => c.command === command).length, command);
async function choose(page, label, value) {
  await panel(page).getByRole("combobox", { name: label, exact: true }).click();
  await page.getByRole("option", { name: value, exact: true }).click();
}
test("充电草稿、确认重置、写后预览与初始恢复", async ({ page }) => {
  const p = panel(page);
  await expect(p.getByRole("meter", { name: "当前电量", exact: true })).toHaveAttribute("aria-valuenow", "62");
  await expect(p.getByRole("meter", { name: "满充 / 设计容量", exact: true })).toHaveAttribute("aria-valuenow", "96");
  await p.getByRole("switch", { name: "限制充电范围", exact: true }).click();
  await choose(page, "开始充电阈值", "70%"); await choose(page, "停止充电阈值", "80%");
  expect(await calls(page, "apply_battery_charge")).toBe(0);
  await expect(p.getByRole("status", { name: "充电设置预览" })).toContainText("低于 70% 开始，达到 80% 停止");
  await p.screenshot({ path: "target/ui-check/battery-preview.png" });
  await p.getByRole("button", { name: "应用充电设置", exact: true }).click();
  await choose(page, "开始充电阈值", "60%");
  await expect(p.getByRole("button", { name: "应用充电设置", exact: true })).toBeVisible();
  expect(await calls(page, "apply_battery_charge")).toBe(0);
  await p.getByRole("button", { name: "应用充电设置", exact: true }).click();
  await p.getByRole("button", { name: "确认应用充电设置？", exact: true }).click();
  await expect(p.getByRole("status", { name: "充电设置预览" })).toHaveText("当前：低于 60% 开始，达到 80% 停止");
  expect(await calls(page, "apply_battery_charge")).toBe(1);
  await p.getByRole("button", { name: "恢复充电初始", exact: true }).click();
  await p.getByRole("button", { name: "恢复本会话修改前的充电设置？", exact: true }).click();
  await expect(p.getByRole("status", { name: "充电设置预览" })).toHaveText("当前：充至满电");
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
test("固件阈值约束与错误保留草稿", async ({ page }) => {
  const p = panel(page); await p.getByRole("switch", { name: "限制充电范围", exact: true }).click();
  await choose(page, "停止充电阈值", "80%");
  await expect(p.getByRole("button", { name: "应用充电设置", exact: true })).toBeDisabled();
  await choose(page, "开始充电阈值", "70%");
  await page.evaluate(() => window.__uiFixture.batteryFault = "隔离写入失败；已恢复修改前状态");
  await p.getByRole("button", { name: "应用充电设置", exact: true }).click();
  await p.getByRole("button", { name: "确认应用充电设置？", exact: true }).click();
  await expect(p.getByRole("alert")).toContainText("隔离写入失败");
  await expect(p.getByRole("combobox", { name: "开始充电阈值", exact: true })).toHaveText("70%");
  await expect(p.getByRole("switch", { name: "限制充电范围", exact: true })).toBeChecked();
  await expect(p.getByRole("status", { name: "充电设置预览" })).toContainText("当前：充至满电 →");
});
test("未知和无电池读数保留空环并拒绝写入", async ({ page }) => {
  const p = panel(page);
  await page.evaluate(() => { const b = window.__uiFixture.battery; b.power = { present: null, acOnline: null, charging: null, percent: null, remainingSeconds: null, windowsBatterySaver: null }; b.diagnostics = null; });
  await p.getByRole("button", { name: "重新读取电池", exact: true }).click();
  await expect(p.getByRole("img", { name: "当前电量：暂无读数", exact: true })).toBeVisible();
  await expect(p.getByRole("switch", { name: "限制充电范围", exact: true })).toHaveAttribute("aria-disabled", "true");
  await expect(p.getByRole("button", { name: "应用充电设置", exact: true })).toBeDisabled();
  await page.evaluate(() => { window.__uiFixture.battery.power.present = false; window.__uiFixture.battery.charge = null; window.__uiFixture.battery.revision = null; });
  await p.getByRole("button", { name: "重新读取电池", exact: true }).click();
  await expect(p).toContainText("未检测到电池"); await expect(p.getByRole("switch", { name: "限制充电范围", exact: true })).toHaveCount(0);
  await expect(p.getByRole("switch", { name: "启用节能计划", exact: true })).toHaveAttribute("aria-disabled", "true");
  expect(await calls(page, "apply_battery_charge")).toBe(0);
});
test("窄屏卡片无横向溢出，比例与策略区保持模块边界", async ({ page }) => {
  await page.setViewportSize({ width: 360, height: 800 }); const p = panel(page);
  await p.scrollIntoViewIfNeeded();
  await expect(p.getByRole("meter", { name: "当前电量", exact: true })).toBeVisible();
  expect(await p.evaluate(node => node.scrollWidth <= node.clientWidth + 1)).toBe(true);
  await p.screenshot({ path: "target/ui-check/battery-narrow.png" });
});
