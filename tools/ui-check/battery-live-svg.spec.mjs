import { configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";
const panel = page => page.getByRole("region", { name: "电池与充电", exact: true });
const overview = page => page.getByLabel("电池实时概览", { exact: true });
const reread = page => panel(page).getByRole("button", { name: "重新读取电池", exact: true }).click();
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: `${MOCK_IPC};window.__uiFixture.calibrationSupported=true;` });
  await page.goto("/"); await openWorkspace(page, "电池与节能");
});
test("电量体积与充电标记只跟随系统报告，插电不等于充电", async ({ page }) => {
  const o = overview(page), meter = o.getByRole("meter", { name: "当前电量", exact: true });
  await expect(meter).toHaveAttribute("aria-valuenow", "62"); await expect(o).toContainText("外接电源");
  await expect(o.locator('[data-battery-charging]')).toHaveCount(0);
  const before = Number(await o.locator('[data-battery-fill]').getAttribute("width"));
  await page.evaluate(() => Object.assign(window.__uiFixture.battery.power, { percent: 25, charging: true }));
  await reread(page); await expect(meter).toHaveAttribute("aria-valuenow", "25");
  await expect(o.locator('[data-battery-charging]')).toHaveCount(1);
  expect(Number(await o.locator('[data-battery-fill]').getAttribute("width"))).toBeLessThan(before);
  await page.evaluate(() => Object.assign(window.__uiFixture.battery.power, { acOnline: false, charging: false, remainingSeconds: 3600 }));
  await reread(page); await expect(o).toContainText("电池供电"); await expect(o).toContainText("60 分钟");
  await expect(o.locator('[data-battery-charging]')).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply|start)_battery/.test(c.command)))).toEqual([]);
});
test("零电量、未知、无电池和超过设计容量分别展示", async ({ page }) => {
  const o = overview(page);
  await page.evaluate(() => { const b = window.__uiFixture.battery; b.power.percent = 0; b.diagnostics.cycleCount = 0; b.diagnostics.capacityPct = 104; });
  await reread(page); await expect(o.getByRole("meter", { name: "当前电量", exact: true })).toHaveAttribute("aria-valuenow", "0");
  await expect(o.locator('[data-battery-fill]')).toHaveAttribute("width", "0");
  await expect(o.getByRole("meter", { name: "满充 / 设计容量", exact: true })).toHaveAttribute("aria-valuetext", "104 % / 100 %");
  await expect(o.locator(".ccc-battery-cycle")).toContainText("0 次");
  await page.evaluate(() => { window.__uiFixture.battery.power.percent = null; window.__uiFixture.battery.diagnostics = null; });
  await reread(page); await expect(o.getByRole("img", { name: "当前电量：暂无读数", exact: true })).toBeVisible();
  await expect(o.locator('[data-battery-fill]')).toHaveCount(0);
  await page.evaluate(() => Object.assign(window.__uiFixture.battery.power, { present: false, percent: 70, charging: true }));
  await reread(page); await expect(o).toContainText("未检测到电池");
  await expect(o.locator('[data-battery-fill], [data-battery-charging]')).toHaveCount(0);
});
test("读取失败撤销旧预览和图形读数，恢复后保留未提交草稿", async ({ page }) => {
  const p = panel(page); await p.getByRole("switch", { name: "限制充电范围", exact: true }).click();
  await p.getByRole("combobox", { name: "开始充电阈值", exact: true }).click(); await page.getByRole("option", { name: "70%", exact: true }).click();
  await openConfigurationReview(page, "充电保护");
  await page.evaluate(() => { window.__uiFixture.batteryReadFault = "系统电量读取失败"; }); await reread(page);
  await expect(p.getByRole("alert")).toContainText("系统电量读取失败");
  await expect(overview(page).locator('[data-battery-fill]')).toHaveCount(0);
  await expect(configurationDialog(page).getByRole("button", { name: "确认应用", exact: true })).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.batteryReadFault = null; }); await reread(page);
  await expect(p.getByRole("alert")).toHaveCount(0);
  await expect(p.getByRole("combobox", { name: "开始充电阈值", exact: true })).toHaveText("70%");
  await expect(p.getByRole("switch", { name: "限制充电范围", exact: true })).toBeChecked();
});
test("校准使用任务实际电量，图形ID隔离且不把阶段映射为百分比", async ({ page }) => {
  await page.evaluate(() => { const c = window.__uiFixture.calibration; c.workerRunning = true; c.canStart = false; c.canStop = true; c.task = { status: "running", phase: "charge2", elapsedMs: 60000, phaseElapsedMs: 10000, error: null, facts: { present: true, acOnline: true, percent: 37 } }; });
  const c = page.getByRole("region", { name: "电池校准", exact: true });
  await c.getByRole("button", { name: "重新读取校准", exact: true }).click();
  await expect(c.getByRole("meter", { name: "校准当前电量", exact: true })).toHaveAttribute("aria-valuenow", "37");
  await expect(c.locator('[data-battery-charging]')).toHaveCount(0);
  const ids = await panel(page).locator(".ccc-battery-artwork defs [id]").evaluateAll(nodes => nodes.map(node => node.id));
  expect(ids.length).toBe(6); expect(new Set(ids).size).toBe(ids.length);
  await page.evaluate(() => { window.__uiFixture.calibrationReadFault = "校准读数失败"; });
  await c.getByRole("button", { name: "重新读取校准", exact: true }).click();
  await expect(c.getByRole("img", { name: "校准当前电量：暂无读数", exact: true })).toBeVisible();
});
test("电池可视化在深浅主题和宽窄卡片中保持独立边框与间距", async ({ page }) => {
  for (const width of [360, 760, 1440]) {
    await page.setViewportSize({ width, height: 960 });
    for (const theme of ["light", "dark"]) {
      await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
      const o = overview(page); await o.scrollIntoViewIfNeeded();
      expect(await o.evaluate(node => node.scrollWidth <= node.clientWidth + 1)).toBe(true);
      await expect(o.locator('[data-surface="inset"]')).toHaveCount(2);
      await o.screenshot({ path: `target/ui-check/battery-live-${width}-${theme}.png` });
    }
  }
});
