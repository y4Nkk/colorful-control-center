import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const memory = page => page.getByRole("region", { name: "系统内存概览", exact: true });
const chart = page => memory(page).getByRole("img", { name: /最近 5 分钟内存使用率面积图/ });
const details = page => page.getByRole("dialog", { name: "内存详细信息", exact: true });
const fact = (page, label) => details(page).locator(".ccc-fact-row").filter({ has: page.getByText(label, { exact: true }) }).locator(".ccc-fact-value");
const openDetails = page => memory(page).getByRole("button", { name: "内存详情", exact: true }).click();

async function publish(page, reading, available = true) {
  await page.evaluate(async ({ reading, available }) => {
    const data = window.__uiFixture;
    data.oemMemory = reading;
    const snapshot = window.__uiHardwareSnapshot();
    snapshot.sampledAtMs = data.lastMemorySample = Math.max(Date.now(), data.lastMemorySample ?? 0) + 1000;
    snapshot.dchuAvailable = available;
    if (!available) {
      const missing = { tempC: null, rpm: null, dutyPct: null };
      snapshot.fan = { cpu: { ...missing }, gpu1: { ...missing }, gpu2: { ...missing }, sys: { ...missing } };
    }
    window.__uiEmit("hardware-snapshot", snapshot);
  }, { reading, available });
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await expect(chart(page)).toBeVisible();
});

test("原厂频率与系统容量分别显示，查看不写硬件或保存配置", async ({ page }) => {
  await expect(memory(page).getByLabel("内存频率", { exact: true })).toHaveText("5200 MT/s");
  await openDetails(page);
  await expect(fact(page, "内存频率")).toHaveText("5200 MT/s");
  await expect(fact(page, "原厂报告频率")).toHaveText("2400 MHz");
  await expect(fact(page, "原厂参考频率")).toHaveText("3200 MHz");
  await expect(fact(page, "总容量")).toHaveText("16 GB");
  await expect(fact(page, "已用内存插槽")).toHaveText("1/2");
  await expect(fact(page, "DIMM 0")).toHaveText("DDR5 · 16 GB · 5200 MT/s · Isolated Memory · 16G-5200");
  await page.keyboard.press("Escape");
  await expect(chart(page)).toHaveCount(1);
  const writes = await page.evaluate(() => window.__uiFixture.calls.filter(({ command, args }) =>
    /^(apply_|restore_|set_|save_|start_|import_|delete_)/.test(command)
      && !(command === "set_shortcut_recording" && args.enabled === false)));
  expect(writes).toEqual([]);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("新的报告直接更新，超过参考值也不缩放或截断", async ({ page }) => {
  await openDetails(page);
  await publish(page, { reportedClockMhz: 4800, referenceClockMhz: 3200 });
  await expect(fact(page, "原厂报告频率")).toHaveText("4800 MHz");
  await expect(fact(page, "原厂参考频率")).toHaveText("3200 MHz");
  await page.keyboard.press("Escape");
  await expect(chart(page)).toHaveCount(1);
});

test("系统配置速率缺失时保留未知，原厂报告仍独立显示", async ({ page }) => {
  await page.clock.install();
  await page.reload();
  await expect(memory(page).getByLabel("内存频率", { exact: true })).toHaveText("5200 MT/s");
  await page.evaluate(() => { window.__uiFixture.memorySpeedMts = null; });
  await page.clock.fastForward(30_100);
  await expect(memory(page).getByLabel("内存频率", { exact: true })).toHaveText("未知");
  await openDetails(page);
  await expect(fact(page, "内存频率")).toHaveText("未知");
  await expect(fact(page, "原厂报告频率")).toHaveText("2400 MHz");
});

test("板载与部分资料独立显示，无法确认的插槽不展示占用", async ({ page }) => {
  await page.clock.install();
  await page.reload();
  await expect(memory(page).getByLabel("内存频率", { exact: true })).toHaveText("5200 MT/s");
  await page.evaluate(() => {
    Object.assign(window.__uiFixture, {
      memorySpeedMts: null, memorySlots: null,
      memoryModules: [{ locator: null, capacityBytes: null, memoryType: "LPDDR5", manufacturer: null,
        partNumber: null, configuredSpeedMts: 6400, soldered: true }],
    });
  });
  await page.clock.fastForward(30_100);
  await expect(memory(page).getByLabel("内存频率", { exact: true })).toHaveText("未知");
  await openDetails(page);
  await expect(fact(page, "已用内存插槽")).toHaveCount(0);
  await expect(fact(page, "内存设备 1")).toHaveText("板载 · LPDDR5 · 6400 MT/s");
});

test("两个字段独立未知，连接失败清旧值，恢复后重新显示", async ({ page }) => {
  await openDetails(page);
  await publish(page, { reportedClockMhz: 2400, referenceClockMhz: null });
  await expect(fact(page, "原厂参考频率")).toHaveText("未知");
  await expect(fact(page, "原厂报告频率")).toHaveText("2400 MHz");
  await publish(page, { reportedClockMhz: null, referenceClockMhz: 3200 });
  await expect(fact(page, "原厂报告频率")).toHaveText("未知");
  await expect(fact(page, "原厂参考频率")).toHaveText("3200 MHz");
  await publish(page, null, false);
  await expect(fact(page, "原厂报告频率")).toHaveText("未知");
  await expect(fact(page, "原厂参考频率")).toHaveText("未知");
  await expect(fact(page, "总容量")).toHaveText("16 GB");
  await publish(page, { reportedClockMhz: 2600, referenceClockMhz: 3200 });
  await expect(fact(page, "原厂报告频率")).toHaveText("2600 MHz");
});

test("离线仍能阅读单位与共享报告说明", async ({ page }) => {
  await publish(page, null, false);
  await openDetails(page);
  await expect(details(page)).toContainText("1 MB 是 1024 KB");
  await expect(details(page)).toContainText("更新可能有延迟");
  await expect(details(page)).toContainText("参考频率只供对照");
  await expect(details(page)).toContainText("DDR 有效传输速率");
});

test("内存使用率与趋势图同高，窄窗纵排，深浅主题的读数不溢出", async ({ page }) => {
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 700, 1440]) {
      await page.setViewportSize({ width, height: 900 });
      await memory(page).scrollIntoViewIfNeeded();
      const layout = await memory(page).evaluate(el => {
        const bounds = el.getBoundingClientRect();
        const style = getComputedStyle(el);
        const header = el.querySelector(".ccc-hardware-identity").getBoundingClientRect();
        const panel = el.querySelector(".ccc-overview-charts").getBoundingClientRect();
        const utilization = el.querySelector(".ccc-overview-utilization").getBoundingClientRect();
        const plot = el.querySelector(".ccc-trend-chart").getBoundingClientRect();
        const sideBySide = utilization.right < plot.left;
        const stats = el.querySelector(".ccc-overview-metric-grid");
        const statsBounds = stats.getBoundingClientRect();
        const readings = [...stats.children];
        return { overflow: el.scrollWidth > el.clientWidth + 1,
          border: parseFloat(style.borderTopWidth), width: bounds.width, viewport: innerWidth,
          gap: parseFloat(getComputedStyle(el.parentElement).gap),
          statsBetween: header.bottom < statsBounds.top && statsBounds.bottom < panel.top,
          sideBySide,
          chartsAligned: sideBySide
            ? utilization.top === plot.top && utilization.bottom === plot.bottom
            : utilization.left === plot.left && utilization.bottom < plot.top,
          matchingColor: getComputedStyle(el.querySelector(".ccc-meter-ring")).color
            === getComputedStyle(el.querySelector(".ccc-trend-plot")).color,
          rows: new Set(readings.map(item => item.getBoundingClientRect().top)).size,
          primaryPaired: readings[0].getBoundingClientRect().top === readings[1].getBoundingClientRect().top,
          readingsFit: readings.every(item => item.scrollWidth <= item.clientWidth + 1) };
      });
      expect(layout.overflow).toBe(false);
      expect(layout.border).toBeGreaterThan(0);
      expect(layout.gap).toBeGreaterThan(0);
      expect(layout.width).toBeLessThanOrEqual(layout.viewport);
      expect(layout.statsBetween).toBe(true);
      expect(layout.sideBySide).toBe(width !== 360);
      expect(layout.chartsAligned).toBe(true);
      expect(layout.matchingColor).toBe(true);
      expect(layout.readingsFit).toBe(true);
      expect(layout.primaryPaired).toBe(true);
      if (width !== 700) expect(layout.rows).toBe(width === 360 ? 2 : 1);
      if (width !== 700) await memory(page).screenshot({ path: `target/ui-check/memory-monitor-${theme}-${width}.png` });
    }
  }
  expect(errors).toEqual([]);
});
