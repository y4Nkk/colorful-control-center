import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openPowerArea } from "./workspace.mjs";
const panel = page => page.getByRole("region", { name: "电池与充电", exact: true });
const overview = page => panel(page).getByRole("region", { name: "当前电池读数", exact: true });
const reread = page => panel(page).getByRole("button", { name: "重新读取电池", exact: true }).click();
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: `${MOCK_IPC};window.__uiFixture.calibrationSupported=true;` });
  await page.goto("/"); await openPowerArea(page, "电源设置");
});
test("电量体积与充电标记只跟随系统报告，插电不等于充电", async ({ page }) => {
  const o = overview(page), meter = o.getByRole("meter", { name: "当前电量", exact: true });
  await expect(meter).toHaveAttribute("aria-valuenow", "62");
  await expect(o.getByText("外接电源 · 未在充电", { exact: true })).toBeVisible();
  await expect(o.getByText("预计剩余", { exact: false })).toHaveCount(0);
  await expect(o.getByText("当前电量", { exact: true })).toHaveCount(0);
  await expect(o.locator('[data-battery-charging]')).toHaveCount(0);
  const before = Number(await o.locator('[data-battery-fill]').getAttribute("width"));
  await page.evaluate(() => Object.assign(window.__uiFixture.battery.power, { percent: 25, charging: true }));
  await reread(page); await expect(meter).toHaveAttribute("aria-valuenow", "25");
  await expect(o.getByText("外接电源 · 正在充电", { exact: true })).toBeVisible();
  await expect(o.locator('[data-battery-charging]')).toHaveCount(1);
  expect(Number(await o.locator('[data-battery-fill]').getAttribute("width"))).toBeLessThan(before);
  await page.evaluate(() => Object.assign(window.__uiFixture.battery.power, { acOnline: false, charging: false, remainingSeconds: 3600 }));
  await reread(page); await expect(o).toContainText("电池供电");
  await expect(o.getByText("预计剩余 60 分钟", { exact: true })).toBeVisible();
  await expect(o.locator('[data-battery-charging]')).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply|start)_battery/.test(c.command)))).toEqual([]);
});
test("外接电源满电停充使用绿色填充，充电标记只随实际充电状态显示", async ({ page }, info) => {
  const o = overview(page), fill = o.locator('[data-battery-fill]'), mark = o.locator('[data-battery-charging]');
  const setPower = async (facts) => {
    await page.evaluate(facts => Object.assign(window.__uiFixture.battery.power, facts), facts);
    await reread(page);
  };
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    await setPower({ percent: 100, acOnline: true, charging: true });
    await expect(mark).toHaveCount(1);
    const green = await fill.evaluate(element => getComputedStyle(element).fill);
    await setPower({ charging: false });
    await expect(o.getByRole("meter")).toHaveAttribute("aria-valuenow", "100");
    await expect(o).toContainText("外接电源 · 未在充电");
    await expect(mark).toHaveCount(0);
    await expect(fill).toHaveCSS("fill", green);
    await o.screenshot({ path: info.outputPath(`battery-full-${theme}.png`) });
    await setPower({ acOnline: false });
    await expect(fill).not.toHaveCSS("fill", green);
    await setPower({ acOnline: true, percent: 80 });
    await expect(o.getByRole("meter")).toHaveAttribute("aria-valuenow", "80");
    await expect(fill).not.toHaveCSS("fill", green);
  }
});
test("零电量、未知和无电池分别展示", async ({ page }) => {
  const o = overview(page);
  await page.evaluate(() => { window.__uiFixture.battery.power.percent = 0; });
  await reread(page); await expect(o.getByRole("meter", { name: "当前电量", exact: true })).toHaveAttribute("aria-valuenow", "0");
  await expect(o.locator('[data-battery-fill]')).toHaveAttribute("width", "0");
  await page.evaluate(() => { window.__uiFixture.battery.power.percent = null; window.__uiFixture.battery.diagnostics = null; });
  await reread(page); await expect(o.getByText("暂无电量读数", { exact: true })).toBeVisible();
  await expect(o.getByRole("meter", { name: "当前电量", exact: true })).toHaveCount(0);
  await expect(o.locator('[data-battery-fill]')).toHaveCount(0);
  await page.evaluate(() => Object.assign(window.__uiFixture.battery.power, { present: false, percent: 70, charging: true }));
  await reread(page); await expect(o).toContainText("未检测到电池");
  await expect(o.locator('[data-battery-fill], [data-battery-charging]')).toHaveCount(0);
});
test("读取失败撤销控件和图形读数，恢复后重读已提交阈值", async ({ page }) => {
  const p = panel(page); await p.getByRole("switch", { name: "限制充电范围", exact: true }).click();
  await p.getByRole("combobox", { name: "开始充电阈值", exact: true }).click(); await page.getByRole("option", { name: "70%", exact: true }).click();
  const committed = p.getByRole("combobox", { name: "开始充电阈值", exact: true });
  await expect(committed).toHaveText("70%");
  await page.evaluate(() => { window.__uiFixture.batteryReadFault = "系统电量读取失败"; });
  await expect(committed).toHaveCount(0, { timeout: 10_000 });
  await expect(p.getByRole("alert")).toContainText("系统电量读取失败");
  await expect(overview(page).locator('[data-battery-fill]')).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.batteryReadFault = null; }); await reread(page);
  await expect(p.getByRole("alert")).toHaveCount(0);
  await expect(p.getByRole("combobox", { name: "开始充电阈值", exact: true })).toHaveText("70%");
  await expect(p.getByRole("switch", { name: "限制充电范围", exact: true })).toBeChecked();
});
test("校准运行时电源页只显示一张电量图", async ({ page }) => {
  await page.evaluate(() => { const c = window.__uiFixture.calibration; c.workerRunning = true; c.canStart = false; c.canStop = true; c.task = { status: "running", phase: "charge2", elapsedMs: 60000, phaseElapsedMs: 10000, error: null, facts: { present: true, acOnline: true, percent: 37 } }; });
  const c = page.getByRole("region", { name: "电池校准", exact: true });
  await c.getByRole("button", { name: "重新读取校准", exact: true }).click();
  await expect(page.getByRole("main", { name: "电源与档案", exact: true }).locator(".ccc-battery-artwork")).toHaveCount(1);
  await expect(panel(page).getByRole("meter", { name: "当前电量", exact: true })).toHaveAttribute("aria-valuenow", "62");
  await expect(c.getByRole("status")).toHaveText("正在校准");
});
test("电池读数和充电控制在深浅主题和宽窄窗口中保持边界", async ({ page }) => {
  for (const width of [360, 760, 1440]) {
    await page.setViewportSize({ width, height: 960 });
    for (const theme of ["light", "dark"]) {
      await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
      const o = overview(page); await o.scrollIntoViewIfNeeded();
      expect(await o.evaluate(node => node.scrollWidth <= node.clientWidth + 1)).toBe(true);
      await expect(o).toBeVisible();
      const board = panel(page).locator(".ccc-charge-board");
      expect(await board.evaluate(node => node.scrollWidth <= node.clientWidth + 1)).toBe(true);
      await o.screenshot({ path: `target/ui-check/battery-live-${width}-${theme}.png` });
    }
  }
});
