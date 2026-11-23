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
    const snapshot = await window.__TAURI_INTERNALS__.invoke("read_hardware_snapshot");
    snapshot.sampledAtMs = data.lastMemorySample = Math.max(Date.now(), data.lastMemorySample ?? 0) + 1000;
    snapshot.dchuAvailable = available;
    if (!available) {
      snapshot.cpuTempC = snapshot.cpuRemoteRaw = null;
      snapshot.fan = { cpuRpm: null, cpuDutyPct: null, gpuRpm: null, gpuDutyPct: null };
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
  await page.clock.fastForward(5100);
  await expect(memory(page).getByLabel("内存频率", { exact: true })).toHaveText("未知");
  await openDetails(page);
  await expect(fact(page, "内存频率")).toHaveText("未知");
  await expect(fact(page, "原厂报告频率")).toHaveText("2400 MHz");
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
  await expect(details(page)).toContainText("1 MB 为 1024 KB");
  await expect(details(page)).toContainText("可能有更新延迟");
  await expect(details(page)).toContainText("参考频率不是硬上限");
  await expect(details(page)).toContainText("DDR 有效传输速率");
});

test("内存趋势在指标上方，深浅主题与宽窄窗口的读数不溢出", async ({ page }) => {
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
        const panel = el.querySelector(".ccc-trend-panel").getBoundingClientRect();
        const stats = el.querySelector(".ccc-memory-stats");
        const readings = [...stats.children];
        return { overflow: el.scrollWidth > el.clientWidth + 1,
          border: parseFloat(style.borderTopWidth), width: bounds.width, viewport: innerWidth,
          gap: parseFloat(getComputedStyle(el.parentElement).gap),
          chartAbove: panel.bottom < stats.getBoundingClientRect().top,
          rows: new Set(readings.map(item => item.getBoundingClientRect().top)).size,
          readingsFit: readings.every(item => item.scrollWidth <= item.clientWidth + 1) };
      });
      expect(layout.overflow).toBe(false);
      expect(layout.border).toBeGreaterThan(0);
      expect(layout.gap).toBeGreaterThan(0);
      expect(layout.width).toBeLessThanOrEqual(layout.viewport);
      expect(layout.chartAbove).toBe(true);
      expect(layout.readingsFit).toBe(true);
      if (width !== 700) expect(layout.rows).toBe(width === 360 ? 2 : 1);
      if (width !== 700) await memory(page).screenshot({ path: `target/ui-check/memory-monitor-${theme}-${width}.png` });
    }
  }
  expect(errors).toEqual([]);
});
