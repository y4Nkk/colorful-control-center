import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";
const panel = page => page.getByRole("region", { name: "电池与充电", exact: true });
const overview = page => panel(page).getByRole("region", { name: "当前电池读数", exact: true });
const reread = page => panel(page).getByRole("button", { name: "重新读取电池", exact: true }).click();
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: `${MOCK_IPC};window.__uiFixture.calibrationSupported=true;` });
  await page.goto("/"); await openWorkspace(page, "电源");
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
test("零电量、未知、无电池和超过设计容量分别展示", async ({ page }) => {
  const o = overview(page);
  await page.evaluate(() => { const b = window.__uiFixture.battery; b.power.percent = 0; b.diagnostics.cycleCount = 0; b.diagnostics.capacityPct = 104; });
  await reread(page); await expect(o.getByRole("meter", { name: "当前电量", exact: true })).toHaveAttribute("aria-valuenow", "0");
  await expect(o.locator('[data-battery-fill]')).toHaveAttribute("width", "0");
  await panel(page).getByRole("button", { name: "详情", exact: true }).click();
  const diagnostics = page.getByRole("dialog", { name: "电池读数与诊断", exact: true });
  await expect(diagnostics.locator(".ccc-fact-row").filter({ hasText: "满充 / 设计容量" })).toContainText("104%");
  await expect(diagnostics.locator(".ccc-fact-row").filter({ hasText: "循环次数" }).locator("dd")).toHaveText("0");
  await page.keyboard.press("Escape");
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
test("校准详情检查任务实际电量，电源页只显示一张电量图", async ({ page }) => {
  await page.evaluate(() => { const c = window.__uiFixture.calibration; c.workerRunning = true; c.canStart = false; c.canStop = true; c.task = { status: "running", phase: "charge2", elapsedMs: 60000, phaseElapsedMs: 10000, error: null, facts: { present: true, acOnline: true, percent: 37 } }; });
  const c = page.getByRole("region", { name: "电池校准", exact: true });
  await c.getByRole("button", { name: "重新读取校准", exact: true }).click();
  await expect(page.getByRole("main", { name: "电源", exact: true }).locator(".ccc-battery-artwork")).toHaveCount(1);
  await expect(panel(page).getByRole("meter", { name: "当前电量", exact: true })).toHaveAttribute("aria-valuenow", "62");
  await expect(c.getByRole("status")).toHaveText("正在校准");
  const details = page.getByRole("dialog", { name: "校准阶段与恢复", exact: true });
  await c.getByRole("button", { name: "详情", exact: true }).click();
  await expect(details).toContainText("任务采样电量 · 37%");
  await details.getByRole("button", { name: "关闭弹窗", exact: true }).click();
  await page.evaluate(() => { window.__uiFixture.calibrationReadFault = "校准读数失败"; });
  await c.getByRole("button", { name: "重新读取校准", exact: true }).click();
  await c.getByRole("button", { name: "详情", exact: true }).click();
  await expect(details).toContainText("任务采样电量 · —");
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

test("电池诊断共享键值列，窄窗只滚动正文", async ({ page }, info) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await panel(page).getByRole("button", { name: "详情", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "电池读数与诊断", exact: true });
  const heading = dialog.getByRole("heading");
  const close = dialog.getByRole("button", { name: "关闭弹窗", exact: true });
  const viewport = dialog.locator(".ccc-scroll-viewport");
  await expect(dialog).toHaveCSS("width", "620px");
  await expect(dialog).toHaveCSS("border-radius", "24px");
  await expect(heading).toHaveCSS("font-size", "18px");
  await expect(close).toHaveCSS("width", "40px");
  await expect(viewport).toHaveCSS("gap", "16px");
  await expect(viewport).toHaveCSS("padding", "24px");
  await expect(dialog.locator("dl")).toHaveCSS("gap", "8px 12px");
  await expect(dialog.locator("dd")).toHaveCount(7);
  const checkFactColumns = () => dialog.locator("dl").evaluate(list => {
    const labels = [...list.querySelectorAll("dt")], values = [...list.querySelectorAll("dd")];
    const start = values[0].getBoundingClientRect().left;
    return values.every(value => value.getBoundingClientRect().left === start && value.scrollWidth <= value.clientWidth + 1)
      && labels.every(label => label.getBoundingClientRect().right <= start);
  });
  expect(await checkFactColumns()).toBe(true);
  await page.screenshot({ path: info.outputPath("battery-details-light-1440.png") });

  await page.evaluate(() => { document.documentElement.dataset.theme = "dark"; });
  await page.setViewportSize({ width: 360, height: 620 });
  await expect(dialog).toHaveCSS("width", "336px");
  expect(await dialog.evaluate(element => getComputedStyle(element, "::backdrop").backgroundColor))
    .toBe("rgba(20, 22, 27, 0.32)");
  await expect(viewport).toHaveCSS("padding", "16px");
  expect(await checkFactColumns()).toBe(true);
  await page.screenshot({ path: info.outputPath("battery-details-dark-360.png") });

  await page.setViewportSize({ width: 360, height: 300 });
  const initial = await heading.boundingBox();
  expect(await viewport.evaluate(element => {
    element.scrollTop = element.scrollHeight;
    return element.scrollTop;
  })).toBeGreaterThan(0);
  expect((await heading.boundingBox()).y).toBe(initial.y);
  await expect(close).toBeInViewport();
  await close.click();
  await expect(dialog).toHaveCount(0);
});
