import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const processor = (page, part) => page.getByRole("region", { name: part + " 运行读数", exact: true });
const cpu = page => processor(page, "CPU");
const frequency = (panel, part = "CPU") => panel.getByLabel(part === "CPU" ? "活跃频率" : "当前频率", { exact: true });
const usage = page => cpu(page).getByRole("meter", { name: "CPU 利用率", exact: true });
const missing = page => cpu(page).getByRole("img", { name: "CPU 利用率：暂无读数", exact: true });
async function publish(page, valuePct, age = 0, error = null) {
  await page.evaluate(async ({ valuePct, age, error }) => {
    const data = window.__uiFixture;
    const snapshot = window.__uiHardwareSnapshot();
    snapshot.sampledAtMs = data.lastCpuSample = Math.max(Date.now(), data.lastCpuSample ?? 0) + 1000;
    snapshot.cpuUtility = valuePct == null ? null : { valuePct, clockMhz: 2900, maxActiveClockMhz: 4700, sampledAtMs: snapshot.sampledAtMs - age };
    snapshot.warnings = error ? [error] : [];
    window.__uiEmit("hardware-snapshot", snapshot);
  }, { valuePct, age, error });
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      const result = await invoke(command, args);
      return command === "read_system_inventory" ? { ...result, warnings: ["隔离系统资料诊断"] } : result;
    };
  });
  await page.goto("/");
  await expect(usage(page)).toHaveAttribute("aria-valuenow", "42");
});
test("CPU 指标显示微型图表和实际读数，查看不会写入", async ({ page }) => {
  const fan = cpu(page).getByRole("meter", { name: "风扇强度", exact: true });
  await expect(fan).toHaveAttribute("aria-valuenow", "30");
  await expect(fan.locator(".ccc-meter-ring-fill")).toHaveAttribute("stroke-dasharray", "30 100");
  await expect(usage(page)).toHaveAttribute("aria-valuemax", "100");
  await expect(frequency(cpu(page))).toHaveText("2900 MHz");
  await expect(cpu(page).locator(".ccc-processor-metrics .ccc-meter-compact > svg")).toHaveCount(3);
  await expect(usage(page).locator(".ccc-meter-ring-fill")).toHaveAttribute("stroke-dasharray", "42 100");
  const writes = await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|restore_|set_)/.test(call.command) && call.command !== "set_shortcut_recording"));
  expect(writes).toEqual([]);
});
test("首次采样和读取失败显示未知，真实零值保留为零并可恢复", async ({ page }) => {
  await publish(page, null); await expect(missing(page)).toBeVisible();
  await expect(missing(page).locator("output")).toHaveText("—");
  await expect(missing(page).locator(".ccc-meter-ring-track")).toBeVisible();
  await expect(missing(page).locator(".ccc-meter-ring-fill")).toHaveCount(0);
  await publish(page, 0); await expect(usage(page)).toHaveAttribute("aria-valuenow", "0");
  await expect(usage(page).locator("output")).toHaveText("0 %");
  await expect(usage(page).locator(".ccc-meter-ring-fill")).toHaveCount(0);
  await publish(page, null, 0, "CPU效用读取失败：隔离计数器不可用"); await expect(missing(page)).toBeVisible();
  await expect(page.getByRole("button", { name: "监控诊断", exact: true })).toHaveCount(0);
  await publish(page, 76); await expect(usage(page)).toHaveAttribute("aria-valuenow", "76");
});
test("过期和未来时间戳不显示为当前利用率或频率", async ({ page }) => {
  await publish(page, 90, 2501); await expect(missing(page)).toBeVisible();
  await expect(frequency(cpu(page))).toHaveText("—");
  await publish(page, 90, -1); await expect(missing(page)).toBeVisible();
  await expect(frequency(cpu(page))).toHaveText("—");
  await publish(page, 90, 2500); await expect(usage(page)).toHaveAttribute("aria-valuenow", "90");
  await expect(frequency(cpu(page))).toHaveText("2900 MHz");
  await cpu(page).getByRole("button", { name: "CPU 规格" }).click();
  const dialog = page.getByRole("dialog", { name: "CPU 硬件参数" });
  const peak = dialog.locator(".ccc-fact-row").filter({ hasText: "最高活跃频率（估算）" });
  await expect(peak.locator("dd")).toHaveText("4700 MHz");
  await publish(page, 90, 2501);
  await expect(peak.locator("dd")).toHaveText("未知");
});

test("频率短暂缺测时保留近期值，持续缺测或停采时结束保留", async ({ page }) => {
  const start = await page.evaluate(() => {
    window.__uiFixture.pauseHardwareSnapshots = true;
    return Date.now() + 10_000;
  });
  const emit = async (offset, clockMhz, valuePct) => {
    await page.evaluate(({ sampledAtMs, clockMhz, valuePct }) => {
      const base = window.__uiHardwareSnapshot();
      const cpuUtility = valuePct === null ? null : { sampledAtMs, clockMhz, valuePct, maxActiveClockMhz: clockMhz };
      window.__uiEmit("hardware-snapshot", { ...base, sampledAtMs, cpuUtility });
    }, { sampledAtMs: start + offset, clockMhz, valuePct });
    if (valuePct === null) await expect(missing(page)).toBeVisible();
    else await expect(usage(page)).toHaveAttribute("aria-valuenow", String(valuePct));
  };
  const clock = frequency(cpu(page));
  await emit(0, 3266, 43);
  await expect(clock).toHaveText("3266 MHz");
  await emit(1000, null, 44);
  await expect(clock).toHaveText("3266 MHz");
  await emit(2000, 3350, 45);
  await expect(clock).toHaveText("3350 MHz");
  await emit(4500, null, 46);
  await expect(clock).toHaveText("3350 MHz");
  await emit(4501, null, 47);
  await expect(clock).toHaveText("—");

  await emit(5000, 3200, 48);
  await emit(6000, null, 49);
  await expect(clock).toHaveText("3200 MHz");
  await expect(page.getByText("读数停滞", { exact: true })).toBeVisible({ timeout: 6000 });
  await expect(clock).toHaveText("—");

  await emit(10_000, 3100, 50);
  await expect(clock).toHaveText("3100 MHz");
  await emit(11_000, null, null);
  await expect(clock).toHaveText("—");
  await emit(12_000, null, 51);
  await expect(clock).toHaveText("—");
  await emit(13_000, 3380, 52);
  await expect(clock).toHaveText("3380 MHz");
});

test("频率短折线使用实际采样时间，保留缺口并区分过期、缺测和零值", async ({ page }) => {
  const end = await page.evaluate(() => {
    window.__uiFixture.pauseHardwareSnapshots = true;
    const base = window.__uiHardwareSnapshot();
    const end = base.sampledAtMs + 75_000;
    for (const [offset, cpuClock, gpuClock, cpuAge = 0] of [
      [-61_000, 100, 200], [-12_000, 2600, 1700], [-11_000, 3100, 1850], [-10_000, 3100, 1850, 1000],
      [-9_000, null, null], [-8_000, 2650, 1670], [-1_000, 3000, 2200], [0, 2977, 2310],
    ]) {
      const at = end + offset;
      window.__uiEmit("hardware-snapshot", { ...base, sampledAtMs: at,
        cpuUtility: { valuePct: 44, sampledAtMs: at - cpuAge, clockMhz: cpuClock, maxActiveClockMhz: cpuClock },
        fan: { ...base.fan, cpu: { ...base.fan.cpu, tempC: 97, dutyPct: 56 }, gpu1: { ...base.fan.gpu1, dutyPct: 56 } },
        nvidia: { ...base.nvidia, tempC: 84, graphicsClockMhz: gpuClock },
      });
    }
    return end;
  });
  const gpu = processor(page, "GPU");
  const sparkline = part => part.locator(".ccc-processor-frequency-line");
  await expect(frequency(cpu(page))).toHaveText("2977 MHz");
  await expect(frequency(gpu, "GPU")).toHaveText("2310 MHz");
  await expect(sparkline(cpu(page))).toHaveAttribute("d",
    /^M80\.000 [\d.]+ L81\.667 [\d.]+ M86\.667 [\d.]+ M98\.333 [\d.]+ L100\.000 [\d.]+$/);
  await expect(sparkline(gpu)).toHaveAttribute("d",
    /^M80\.000 [\d.]+ L81\.667 [\d.]+ L83\.333 [\d.]+ M86\.667 [\d.]+ M98\.333 [\d.]+ L100\.000 [\d.]+$/);
  await expect(cpu(page).locator(".ccc-processor-temp-point")).toHaveAttribute("data-hot", "true");
  await expect(gpu.locator(".ccc-processor-temp-point")).not.toHaveAttribute("data-hot", "true");
  const fan = gpu.getByRole("meter", { name: "风扇强度", exact: true });
  await expect(fan).toHaveAttribute("aria-valuenow", "56");
  const fill = await fan.locator(".ccc-meter-ring-fill").getAttribute("stroke-dasharray");
  expect(Number.parseFloat(fill)).toBeCloseTo(56);

  await page.evaluate(end => {
    const base = window.__uiHardwareSnapshot();
    window.__uiEmit("hardware-snapshot", { ...base, sampledAtMs: end + 1000, cpuUtility: null,
      fan: { ...base.fan, cpu: { ...base.fan.cpu, tempC: null, dutyPct: 0 }, gpu1: { ...base.fan.gpu1, dutyPct: null } },
      nvidia: { ...base.nvidia, tempC: null, graphicsClockMhz: 0 },
    });
  }, end);
  await expect(cpu(page).locator(".ccc-processor-temp-point")).toHaveCount(0);
  await expect(frequency(cpu(page))).toHaveText("—");
  await expect(frequency(gpu, "GPU")).toHaveText("0 MHz");
  const stopped = cpu(page).getByRole("meter", { name: "风扇强度", exact: true });
  await expect(stopped).toHaveAttribute("aria-valuenow", "0");
  await expect(stopped.locator(".ccc-meter-ring-fill")).toHaveCount(0);
  await expect(gpu.getByRole("img", { name: "风扇强度：暂无读数", exact: true })).toBeVisible();
});

async function expectProcessorCharts(page, part, rows, sideBySide) {
  const panel = processor(page, part);
  const charts = panel.locator(".ccc-overview-charts");
  await expect(charts.getByRole("meter", { name: part + " 利用率", exact: true })).toBeVisible();
  await expect(charts.getByText("最近 5 分钟温度", { exact: true })).toHaveCount(0);
  const temperature = charts.getByRole("img", { name: new RegExp("^" + part + " 最近 5 分钟温度趋势") });
  await expect(temperature).toBeVisible();
  const [readouts, group, arc, plot] = await Promise.all([
    panel.locator(".ccc-overview-metrics").boundingBox(),
    charts.boundingBox(),
    charts.locator(".ccc-overview-utilization").boundingBox(),
    temperature.boundingBox(),
  ]);
  expect(group.y).toBeGreaterThan(readouts.y + readouts.height);
  expect(group.x).toBeCloseTo(readouts.x, 0);
  const metricLayout = await panel.locator(".ccc-processor-metrics").evaluate(grid => {
    const cells = [...grid.children];
    const bounds = grid.getBoundingClientRect();
    const rows = new Set(cells.map(cell => cell.getBoundingClientRect().top)).size;
    const unitsFit = cells.every(cell => {
      const [number, unit] = cell.querySelector("output").children;
      const valueBox = number.getBoundingClientRect(), unitBox = unit.getBoundingClientRect();
      return unitBox.height === 0 || unitBox.top < valueBox.bottom && unitBox.right <= cell.getBoundingClientRect().right + 1;
    });
    const plots = cells.map(cell => {
      const picture = cell.querySelector("svg").getBoundingClientRect();
      const value = cell.querySelector("output").getBoundingClientRect();
      const label = cell.querySelector(".ccc-meter-label").getBoundingClientRect();
      return { x: picture.left - bounds.left, y: picture.top - bounds.top, width: picture.width, readingY: value.top - bounds.top,
        aligned: picture.right < value.left && value.left === label.left && label.bottom <= value.top
          && label.top >= cell.getBoundingClientRect().top - 1 };
    });
    return { rows, unitsFit, plots };
  });
  if (rows != null) expect(metricLayout.rows).toBe(rows);
  expect(metricLayout.unitsFit).toBe(true);
  expect(metricLayout.plots.every(plot => plot.aligned)).toBe(true);
  if (sideBySide) {
    expect(arc.x + arc.width).toBeLessThan(plot.x);
    expect(arc.y).toBeCloseTo(plot.y, 0);
    expect(arc.y + arc.height).toBeCloseTo(plot.y + plot.height, 0);
  } else {
    expect(arc.y + arc.height).toBeLessThan(plot.y);
    expect(arc.x).toBeCloseTo(plot.x, 0);
  }
  return metricLayout;
}

test("深浅宽窄布局中利用率和温度图可读，CPU 读数不溢出", async ({ page }) => {
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const [width, rows] of [[360, 3], [760, 1], [900, null], [1000, null], [1200, 1], [1440, 1], [1570, 1]]) {
      await page.setViewportSize({ width, height: 900 }); await cpu(page).scrollIntoViewIfNeeded();
      const layout = await cpu(page).evaluate(el => {
        const bounds = el.getBoundingClientRect();
        return { overflow: el.scrollWidth > el.clientWidth + 1, right: bounds.right,
          border: parseFloat(getComputedStyle(el).borderTopWidth), gap: parseFloat(getComputedStyle(el.parentElement).gap) };
      });
      expect(layout.overflow).toBe(false); expect(layout.right).toBeLessThanOrEqual(width);
      expect(layout.border).toBeGreaterThan(0); expect(layout.gap).toBeGreaterThan(0);
      const left = await expectProcessorCharts(page, "CPU", rows, width !== 360);
      const right = await expectProcessorCharts(page, "GPU", rows, width !== 360);
      expect(left.rows).toBe(right.rows);
      for (let index = 0; index < left.plots.length; index++) {
        for (const property of ["x", "y", "width", "readingY"]) {
          expect(left.plots[index][property], `${width}px ${property}`).toBeCloseTo(right.plots[index][property], 0);
        }
      }
      if (width === 360 || width === 1440) await cpu(page).screenshot({ path: `target/ui-check/cpu-utility-${theme}-${width}.png` });
    }
  }
});
