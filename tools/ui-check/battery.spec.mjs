import { availableAction, configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openPowerArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openPowerArea(page, "电源设置"); });
const panel = page => page.getByRole("region", { name: "电池与充电", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(c => c.command === command).length, command);
async function choose(page, label, value) {
  await panel(page).getByRole("combobox", { name: label, exact: true }).click();
  await page.getByRole("option", { name: value, exact: true }).click();
}
test("充电开关和合法阈值立即提交，初始范围单独恢复", async ({ page }) => {
  const p = panel(page);
  await expect(p.getByRole("meter", { name: "当前电量", exact: true })).toHaveAttribute("aria-valuenow", "62");
  const limit = p.getByRole("switch", { name: "限制充电范围", exact: true });
  const start = p.getByRole("combobox", { name: "开始充电阈值", exact: true });
  const stop = p.getByRole("combobox", { name: "停止充电阈值", exact: true });
  await expect(start).toHaveCount(0);
  await expect(limit).toBeEnabled();
  await limit.focus();
  await page.keyboard.press("Space");
  await expect(limit).toBeChecked();
  expect(await calls(page, "apply_battery_charge")).toBe(1);
  await choose(page, "开始充电阈值", "70%");
  await expect(start).toHaveText("70%");
  await choose(page, "停止充电阈值", "80%");
  await expect(stop).toHaveText("80%");
  await expect(availableAction(page, "充电保护")).toHaveCount(0);
  expect(await calls(page, "apply_battery_charge")).toBe(3);
  const restore = await previewConfiguration(page, "充电保护", true);
  await restore.getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(limit).not.toBeChecked();
  await expect(start).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
test("空阈值等有效组合才写入，失败后回读实际状态", async ({ page }) => {
  const p = panel(page);
  const limit = p.getByRole("switch", { name: "限制充电范围", exact: true });
  await expect(limit).toBeEnabled();
  await page.evaluate(() => {
    window.__uiFixture.battery.charge.current = { enabled: false, start: 0, stop: 0 };
    window.__uiFixture.battery.revision = "unconfigured-charging";
  });
  await p.getByRole("button", { name: "重新读取电池", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__uiFixture.calls.filter(c => c.command === "read_battery_profile").length)).toBeGreaterThan(1);
  await limit.click();
  const start = p.getByRole("combobox", { name: "开始充电阈值", exact: true });
  await expect(start).toHaveText("未设置");
  await choose(page, "停止充电阈值", "80%");
  expect(await calls(page, "apply_battery_charge")).toBe(0);
  await page.evaluate(() => window.__uiFixture.batteryFault = "隔离写入失败；已恢复修改前状态");
  await choose(page, "开始充电阈值", "70%");
  await expect(p.getByRole("alert")).toContainText("隔离写入失败");
  await expect(limit).not.toBeChecked();
  expect(await calls(page, "apply_battery_charge")).toBe(1);
});
test("未知和无电池读数显示文字状态并拒绝写入", async ({ page }) => {
  const p = panel(page);
  await page.evaluate(() => { const b = window.__uiFixture.battery; b.power = { present: null, acOnline: null, charging: null, percent: null, remainingSeconds: null, windowsBatterySaver: null }; b.diagnostics = null; });
  await p.getByRole("button", { name: "重新读取电池", exact: true }).click();
  await expect(p.getByText("暂无电量读数", { exact: true })).toBeVisible();
  await expect(p.getByRole("switch", { name: "限制充电范围", exact: true })).toHaveAttribute("aria-disabled", "true");
  await expect(availableAction(page, "充电保护")).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.battery.power.present = false; window.__uiFixture.battery.charge = null; window.__uiFixture.battery.revision = null; });
  await p.getByRole("button", { name: "重新读取电池", exact: true }).click();
  await expect(p).toContainText("未检测到电池"); await expect(p.getByRole("switch", { name: "限制充电范围", exact: true })).toHaveCount(0);
  await expect(page.getByRole("region", { name: "电池节能计划", exact: true })
    .getByRole("switch", { name: "启用节能计划", exact: true })).toHaveAttribute("aria-disabled", "true");
  expect(await calls(page, "apply_battery_charge")).toBe(0);
});
test("窄屏阈值用留白分组并保持卡片边界", async ({ page }) => {
  await page.setViewportSize({ width: 360, height: 800 }); const p = panel(page);
  await p.scrollIntoViewIfNeeded();
  await expect(p.getByRole("meter", { name: "当前电量", exact: true })).toBeVisible();
  await p.getByRole("switch", { name: "限制充电范围", exact: true }).check();
  const settings = p.locator(".ccc-charge-board, .ccc-charge-board .ccc-power-subsettings > *");
  await expect(settings).toHaveCount(3);
  expect(await settings.evaluateAll(items => items.every(item => {
    const { borderBlockStartWidth, borderBlockEndWidth } = getComputedStyle(item);
    return borderBlockStartWidth === "0px" && borderBlockEndWidth === "0px";
  }))).toBe(true);
  expect(await p.evaluate(node => node.scrollWidth <= node.clientWidth + 1)).toBe(true);
  await p.screenshot({ path: "target/ui-check/battery-narrow.png" });
});
