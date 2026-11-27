import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const overview = page => page.getByRole("region", { name: "概览", exact: true });
const sensor = (page, part) => overview(page).getByRole("region", { name: `${part} 运行读数`, exact: true });
const temperature = (page, part) => sensor(page, part).getByRole("img", { name: new RegExp(`^${part} 最近 5 分钟温度趋势`) });
const memory = page => page.getByRole("region", { name: "系统内存概览", exact: true });
const memoryChart = page => memory(page).getByRole("img", { name: /最近 5 分钟内存使用率面积图/ });
const storage = page => page.getByRole("region", { name: "本地存储概览", exact: true });

function volume(mount, label, totalBytes, availableBytes) {
  return { mount, label, fileSystem: "NTFS", capacity: { totalBytes, availableBytes } };
}

const MB = 1024 ** 2, GB = 1024 ** 3, TB = 1024 ** 4;
const volumes = [
  volume("C:", "系统", 460 * GB, 213 * GB),
  volume("D:", "恢复工具", 922 * MB, 852 * MB),
  volume("E:", null, TB, TB),
  volume("F:", "非常长的卷标".repeat(7), 500 * GB, 300 * GB),
  volume("I:", "维护", 4096, 2048),
  volume("R:", "离线卷", 0, 0),
];

async function setup(page, storageVolumes = volumes) {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(storageVolumes => {
    const original = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      const reply = await original(command, args);
      if (command === "read_system_inventory") return { ...reply, volumes: storageVolumes };
      if (command === "read_memory_capacity" && window.__overviewMemory !== undefined) {
        if (window.__overviewMemory === null) throw new Error("隔离内存读取失败");
        return window.__overviewMemory;
      }
      return reply;
    };
  }, storageVolumes);
  await page.goto("/");
}

async function sampleMemory(page, availableBytes) {
  await page.evaluate(({ availableBytes, totalBytes }) => {
    window.__overviewMemory = availableBytes === null ? null : { totalBytes, availableBytes };
  }, { availableBytes, totalBytes: 16 * GB });
  await page.clock.fastForward(5_100);
}

function publishMode(page, value) {
  return page.evaluate(async value => {
    const sample = window.__uiHardwareSnapshot();
    window.__uiEmit("hardware-snapshot", { ...sample, oemPowerMode: value });
  }, value);
}

test("当前模式按采样更新插画，空值、未知编码和停采显示中性色", async ({ page }) => {
  await page.clock.install();
  await setup(page);
  const modeCard = overview(page).getByRole("region", { name: "当前模式", exact: true });
  const mode = modeCard.getByRole("status");
  const art = modeCard.locator("svg.ccc-preset-artwork");
  await expect(art).toHaveCount(1);
  await expect(art).toHaveAttribute("aria-hidden", "true");
  await expect(mode).toHaveText("性能");
  await expect(mode).toHaveCSS("background-image", "none");
  await expect(art).toHaveAttribute("data-preset-palette", "performance");
  for (const [value, name, kind] of [[0, "安静", "quiet"], [4, "未知", "neutral"], [99, "未知", "neutral"], [null, "未知", "neutral"]]) {
    await page.clock.fastForward(30);
    await publishMode(page, value);
    await expect(mode).toHaveText(name);
    await expect(art).toHaveAttribute("data-preset-palette", kind);
  }
  await page.clock.fastForward(4_000);
  await expect(mode).toHaveText("未知");
  await expect(mode).toHaveCSS("background-image", "none");
  await expect(art).toHaveAttribute("data-preset-palette", "neutral");
  await expect(art).not.toHaveAttribute("data-switching", "true");
  await expect(modeCard.locator("button, input")).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(({ command }) =>
    /^(read|apply|restore)_oem_power_mode$/.test(command)))).toEqual([]);
});

async function expectCenteredArtwork(card) {
  const layout = await card.evaluate(section => {
    const frame = section.getBoundingClientRect();
    const art = section.querySelector(":scope > svg.ccc-preset-artwork");
    const box = art?.getBoundingClientRect();
    const heading = section.querySelector("h2").getBoundingClientRect();
    const label = section.querySelector("output").getBoundingClientRect();
    return {
      centered: !!box && Math.abs((box.left + box.right) - (frame.left + frame.right)) < 2,
      stacked: !!box && heading.bottom <= box.top && box.bottom <= label.top,
      contained: !!box && box.left >= frame.left && box.right <= frame.right,
    };
  });
  expect(layout).toEqual({ centered: true, stacked: true, contained: true });
}

test("概览插画嵌入大卡正中，回读后入场且短窗保留温度趋势", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await setup(page);
  const card = overview(page).getByRole("region", { name: "当前模式", exact: true });
  const art = card.locator("svg.ccc-preset-artwork");
  await expect(art).toHaveAttribute("data-preset-palette", "performance");
  await expect(page.locator("html")).toHaveAttribute("data-motion", "reduce");
  await page.evaluate(() => { document.documentElement.dataset.motion = "system"; });
  await art.evaluate(svg => {
    const freeze = event => {
      if (event.animationName !== "ccc-preset-reveal") return;
      svg.removeEventListener("animationstart", freeze);
      for (const animation of svg.getAnimations({ subtree: true })) {
        animation.pause();
        animation.currentTime = 220;
      }
    };
    svg.addEventListener("animationstart", freeze);
  });
  await publishMode(page, 3);
  await expect(card.getByRole("status")).toHaveText("娱乐");
  await expect(art).toHaveAttribute("data-preset-palette", "entertainment");
  const incoming = art.locator(".ccc-preset-layer:not([data-outgoing])");
  await expect(incoming.locator(".ccc-preset-drawing")).not.toHaveCSS("opacity", "1");
  await expectCenteredArtwork(card);
  expect((await art.boundingBox()).height).toBeGreaterThan(150);
  await card.screenshot({ path: test.info().outputPath("overview-preset-reveal.png") });
  await art.evaluate(svg => { for (const animation of svg.getAnimations({ subtree: true })) animation.play(); });
  await expect(art).not.toHaveAttribute("data-switching", "true");
  expect(await incoming.locator(".ccc-preset-orbit-light:not([data-orbit])").evaluate(path => path.getAnimations()[0].animationName))
    .toBe("ccc-preset-orbit");

  await page.emulateMedia({ reducedMotion: "reduce" });
  await publishMode(page, 1);
  await expect(art).toHaveAttribute("data-preset-palette", "eco");
  await expect(art).not.toHaveAttribute("data-switching", "true");
  await page.evaluate(() => { document.documentElement.dataset.theme = "dark"; });
  await page.setViewportSize({ width: 900, height: 560 });
  expect((await art.boundingBox()).height).toBeLessThanOrEqual(149);
  for (const part of ["CPU", "GPU"]) {
    const plot = temperature(page, part);
    await plot.evaluate(element => element.scrollIntoView({ block: "center" }));
    await expect(plot).toBeInViewport({ ratio: .99 });
  }
  await page.setViewportSize({ width: 360, height: 800 });
  await expectCenteredArtwork(card);
  await card.screenshot({ path: test.info().outputPath("overview-chip-dark-360.png") });
});

async function expectHardwareHeader(panel, kind, brand) {
  const header = panel.locator(".ccc-hardware-identity");
  await expect(header.locator(".ccc-hardware-art")).toHaveCount(1);
  const mark = header.locator(".ccc-hardware-mark");
  if (brand) {
    await expect(mark.getByRole("img", { name: brand, exact: true })).toBeVisible();
  } else {
    await expect(mark.locator(`[data-icon="${kind}"]`)).toBeVisible();
    await expect(mark.getByRole("img")).toHaveCount(0);
  }
  const layout = await header.evaluate(element => {
    const artwork = element.querySelector(".ccc-hardware-emblem").getBoundingClientRect();
    const description = element.querySelector(".ccc-hardware-description").getBoundingClientRect();
    const mark = element.querySelector(".ccc-hardware-mark").firstElementChild.getBoundingClientRect();
    const action = element.querySelector(".ccc-hardware-action button").getBoundingClientRect();
    return {
      topRow: artwork.right <= description.left && artwork.top < description.bottom && description.top < artwork.bottom,
      bottomRow: artwork.bottom < mark.top && description.bottom < action.top && mark.right <= action.left
        && mark.top < action.bottom && action.top < mark.bottom,
      leftAligned: Math.abs(artwork.left + artwork.right - mark.left - mark.right) < 2,
      rightAligned: Math.abs(description.left - action.left) < 1,
      fits: element.scrollWidth <= element.clientWidth + 1,
    };
  });
  expect(layout).toEqual({ topRow: true, bottomRow: true, leftAligned: true, rightAligned: true, fits: true });
}

async function expectCapacityHeader(panel) {
  const header = panel.locator(".ccc-hardware-identity");
  await expect(header.locator(".ccc-hardware-art")).toHaveCount(1);
  await expect(header.locator(".ccc-hardware-mark")).toHaveCount(0);
  const [bounds, artwork, title, button] = await Promise.all([
    header.boundingBox(),
    header.locator(".ccc-hardware-emblem").boundingBox(),
    header.getByRole("heading").boundingBox(),
    header.getByRole("button").boundingBox(),
  ]);
  expect(artwork.x + artwork.width).toBeLessThan(title.x);
  expect(title.x + title.width).toBeLessThan(button.x);
  expect(button.x + button.width).toBeCloseTo(bounds.x + bounds.width, 0);
  expect(button.y + button.height / 2).toBeCloseTo(title.y + title.height / 2, 0);
}

async function expectDetailsEntry(page, panel, label, title, iconOnly) {
  const info = panel.getByRole("button", { name: label, exact: true });
  await expect(info).toHaveText(iconOnly ? "" : label);
  await expect(info.locator("svg")).toHaveCount(iconOnly ? 1 : 0);
  await info.focus();
  if (iconOnly) {
    await expect(info.locator("svg")).toHaveAttribute("data-icon", "info");
    await expect(page.getByRole("tooltip")).toHaveText(title);
  }
  await info.press("Enter");
  await expect(page.getByRole("dialog", { name: title, exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(info).toBeFocused();
}

test("概览卡头、绘图区和读数栏对齐，详情可用键盘打开", async ({ page }) => {
  await setup(page);
  const cpu = sensor(page, "CPU");
  const gpu = sensor(page, "GPU");
  const panels = [[cpu, "CPU 规格", "CPU 硬件参数", false], [gpu, "GPU 规格", "GPU 硬件参数", false],
    [memory(page), "内存详情", "内存详细信息", true], [storage(page), "存储详情", "存储详细信息", true]];
  await expect(cpu.getByRole("meter", { name: "CPU 利用率" })).toBeVisible();
  const sampleLayout = () => page.evaluate(async () => {
    const sample = window.__uiHardwareSnapshot();
    window.__uiEmit("hardware-snapshot", { ...sample, oemPowerMode: 3, fan: { ...sample.fan, cpu: { ...sample.fan.cpu, dutyPct: 100 } },
      nvidia: { ...sample.nvidia, name: "NVIDIA GeForce RTX 4060 Laptop GPU", memTotalMb: 8192 } });
  });
  await sampleLayout();
  await expect(gpu.getByText("显存占用", { exact: true })).toHaveCount(0);
  await expect(overview(page).getByText(/^采样 \d{2}:\d{2}:\d{2}$/)).toHaveCount(0);
  for (const [panel, label, title, iconOnly] of panels) await expectDetailsEntry(page, panel, label, title, iconOnly);

  for (const [theme, width] of [["light", 1440], ["dark", 360]]) {
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    await page.setViewportSize({ width, height: 900 });
    await expectHardwareHeader(cpu, "cpu", "Intel");
    await expectHardwareHeader(gpu, "gpu", "NVIDIA");
    await expectCapacityHeader(memory(page));
    await expectCapacityHeader(storage(page));
    for (const [panel, labels, narrowRows] of [
      [cpu, ["当前温度", "活跃频率", "风扇强度"], 3],
      [gpu, ["当前温度", "当前频率", "风扇强度"], 3],
      [memory(page), ["已用内存", "可用", "总容量", "频率"], 2],
    ]) {
      await expect(panel.locator(".ccc-meter-ring")).toHaveCount(1);
      const readouts = panel.locator(".ccc-overview-metrics");
      await expect(readouts.locator(".ccc-meter-label")).toHaveText(labels);
      const [header, grid, chart] = await Promise.all([
        panel.locator(".ccc-hardware-identity").boundingBox(),
        readouts.boundingBox(),
        panel.locator(".ccc-trend-chart").boundingBox(),
      ]);
      const trend = panel.locator(".ccc-trend-chart");
      const surface = await panel.locator(".ccc-overview-utilization").evaluate(node => {
        const { backgroundColor, borderRadius } = getComputedStyle(node);
        return { backgroundColor, borderRadius };
      });
      expect(surface.backgroundColor).not.toBe("rgba(0, 0, 0, 0)");
      await expect(trend).toHaveCSS("background-color", surface.backgroundColor);
      await expect(trend).toHaveCSS("border-radius", surface.borderRadius);
      expect(await trend.locator(".ccc-trend-tick").evaluateAll(ticks => ticks.every(tick => {
        const outer = tick.closest(".ccc-trend-chart").getBoundingClientRect();
        const inner = tick.getBoundingClientRect();
        return inner.right < outer.right && inner.top > outer.top && inner.bottom < outer.bottom;
      }))).toBe(true);
      const cells = await panel.locator(".ccc-meter-compact").evaluateAll(items => items.map(item => ({
        y: item.getBoundingClientRect().top, bottom: item.querySelector("output").getBoundingClientRect().bottom,
        fits: item.scrollWidth <= item.clientWidth + 1,
      })));
      expect(grid.y).toBeGreaterThan(header.y + header.height);
      expect(chart.y).toBeGreaterThan(grid.y + grid.height);
      expect(cells.every(cell => cell.fits)).toBe(true);
      const rows = new Map(cells.map(({ y, bottom }) => [y, bottom]));
      expect(rows.size).toBe(width === 1440 ? 1 : narrowRows);
      expect(cells.every(({ y, bottom }) => Math.abs(rows.get(y) - bottom) < 1)).toBe(true);
      expect(grid.y + grid.height).toBeGreaterThan(Math.max(...cells.map(cell => cell.bottom)));
    }
    if (width === 1440) {
      for (const selector of [".ccc-trend-plot", ".ccc-overview-metrics"]) {
        const [left, right] = await Promise.all([cpu.locator(selector).boundingBox(), gpu.locator(selector).boundingBox()]);
        expect(left.y).toBeCloseTo(right.y, 0);
      }
      const [chart, readout] = await Promise.all([memory(page).locator(".ccc-overview-charts").boundingBox(),
        memory(page).locator(".ccc-overview-metrics").boundingBox()]);
      expect(chart.x).toBeCloseTo(readout.x, 0);
    }
    const modeCard = overview(page).getByRole("region", { name: "当前模式", exact: true });
    await sampleLayout();
    await expect(modeCard.getByRole("status")).toHaveText("娱乐");
    await modeCard.screenshot({ path: `target/ui-check/overview-current-mode-${theme}-${width}.png` });
    await cpu.locator("..").screenshot({ path: `target/ui-check/overview-processor-readouts-${theme}-${width}.png` });
  }
});

test("温度趋势按采样时刻定位，标出高温且缺测时断开折线和填充", async ({ page }) => {
  await setup(page);
  const latestCpu = sensor(page, "CPU").getByLabel("CPU 当前温度");
  const latestGpu = sensor(page, "GPU").getByLabel("GPU 当前温度");
  const cpuPlot = temperature(page, "CPU"), gpuPlot = temperature(page, "GPU");
  const cpuLine = cpuPlot.locator(".ccc-trend-line"), gpuLine = gpuPlot.locator(".ccc-trend-line");
  await expect(latestCpu).toHaveText("50 °C");
  await expect(cpuPlot.locator(".ccc-trend-marker")).toHaveCount(1);
  await expect(cpuPlot.locator(".ccc-trend-area")).toHaveCount(0);
  await page.evaluate(() => {
    const sample = window.__uiHardwareSnapshot();
    window.__uiEmit("hardware-snapshot", { ...sample, sampledAtMs: sample.sampledAtMs + 100,
      fan: { ...sample.fan, cpu: { ...sample.fan.cpu, tempC: 95 } } });
  });
  await expect(latestCpu).toHaveText("95 °C");
  await expect(latestCpu).toHaveAttribute("data-alert", "true");
  await page.evaluate(async () => {
    const sample = window.__uiHardwareSnapshot();
    const start = sample.sampledAtMs + 301_000;
    window.__overviewSnapshot = { ...sample, sampledAtMs: start };
    for (const [offset, cpu, gpu] of [[0, 45, 49], [500, 0, null], [1500, null, 54], [2500, 60.5, 55],
      [11500, 98, 87], [12500, 95, 88], [13500, 94.8, 89]]) {
      window.__uiEmit("hardware-snapshot", {
        ...sample, sampledAtMs: start + offset, fan: { ...sample.fan, cpu: { ...sample.fan.cpu, tempC: cpu } },
        nvidia: { ...sample.nvidia, tempC: gpu },
      });
    }
  });
  await expect(latestCpu).toHaveText("94.8 °C");
  await expect(latestCpu).not.toHaveAttribute("data-alert");
  await expect(latestGpu).toHaveText("89 °C");
  for (const part of ["CPU", "GPU"]) {
    const card = sensor(page, part);
    await expect(card.locator(".ccc-trend-peak")).toHaveCount(0);
    await expect(card.getByText(/^(5 分钟最高|5 分钟前|现在)$/)).toHaveCount(0);
  }
  await expect(cpuPlot.locator(".ccc-temperature-limit")).toHaveAttribute("d", "M0 16.667H100");
  await expect(cpuPlot.locator(".ccc-trend-marker[data-hot]")).toHaveCount(2);
  await expect(gpuPlot.locator(".ccc-trend-marker[data-hot]")).toHaveCount(0);
  await expect(cpuLine).toHaveAttribute("d", /^M95\.500 [\d.]+ L95\.667 100\.000 M96\.333 [\d.]+ M99\.333 [\d.]+ L99\.667 [\d.]+ L100\.000 [\d.]+$/);
  await expect(gpuLine).toHaveAttribute("d", /^M95\.500 [\d.]+ M96\.000 [\d.]+ L96\.333 [\d.]+ M99\.333 [\d.]+ L99\.667 [\d.]+ L100\.000 [\d.]+$/);
  await expect(cpuPlot.locator(".ccc-trend-area")).toHaveAttribute("d", /L95\.667 100 L95\.500 100 Z M99\.333 .*L100\.000 100 L99\.333 100 Z$/);
  await expect(cpuPlot.locator(".ccc-trend-area")).toHaveAttribute("fill", /^url\(#/);
  const plot = cpuPlot.locator(".ccc-trend-plot");
  const { width, height } = await plot.boundingBox();
  await plot.hover({ position: { x: width * .99333, y: height / 2 } });
  await expect(cpuPlot.locator(".ccc-trend-tooltip")).toContainText("98 °C");
  await expect(cpuPlot.locator(".ccc-trend-cursor")).toHaveAttribute("d", "M99.333 0V100");
  await plot.hover({ position: { x: width * .97833, y: height / 2 } });
  await expect(cpuPlot.locator(".ccc-trend-tooltip")).toHaveCount(0);
  await page.evaluate(() => {
    const sample = window.__overviewSnapshot;
    window.__uiEmit("hardware-snapshot", {
      ...sample, sampledAtMs: sample.sampledAtMs + 14500, fan: { ...sample.fan, cpu: { ...sample.fan.cpu, tempC: null } },
      nvidia: null, nvidiaSuspended: true,
    });
  });
  await expect(latestCpu).toHaveText("—");
  await expect(latestGpu).toHaveText("—");
  const missingGpu = sensor(page, "GPU").getByRole("img", { name: "GPU 利用率：暂无读数", exact: true });
  await expect(missingGpu.locator("output")).toHaveText("—");
  await expect(missingGpu.locator(".ccc-meter-ring-fill")).toHaveCount(0);
  await expect(cpuLine).toHaveAttribute("d", /L99\.667 [\d.]+$/);
  await expect(gpuLine).toHaveAttribute("d", /L99\.667 [\d.]+$/);
  await expectHardwareHeader(sensor(page, "GPU"), "gpu");
});

test("磁盘卷、详情和显存总量沿用真实容量与统一单位", async ({ page }) => {
  await setup(page);
  const disks = storage(page);
  await expect(disks.getByRole("listitem")).toHaveCount(6);
  await expect(disks.getByRole("heading", { name: "系统 (C:)", exact: true })).toBeVisible();
  await disks.getByRole("heading", { name: "本地磁盘 (E:)", exact: true }).hover();
  await expect(page.getByRole("tooltip")).toHaveText("本地磁盘 (E:)");
  await expect(disks.getByText("213 GB 可用 · 共 460 GB", { exact: true })).toBeVisible();
  await expect(disks.getByText("852 MB 可用 · 共 922 MB", { exact: true })).toBeVisible();
  await expect(disks.getByText("2 KB 可用 · 共 4 KB", { exact: true })).toBeVisible();
  const meter = mount => disks.getByRole("meter", { name: `${mount} 使用量`, exact: true });
  await expect(meter("C:")).toHaveAttribute("aria-valuenow", String(247 * GB));
  const usedShare = mount => meter(mount).locator(".ccc-capacity-fill").evaluate(fill =>
    fill.getBoundingClientRect().width / fill.parentElement.getBoundingClientRect().width);
  expect(await usedShare("C:")).toBeCloseTo(247 / 460, 3);
  expect(await usedShare("E:")).toBe(0);
  await expect(disks.getByText("1 TB 可用 · 共 1 TB", { exact: true })).toBeVisible();
  await expect(meter("R:")).toHaveCount(0);
  await expect(disks.getByText("容量未知", { exact: true })).toBeVisible();
  await expect(memory(page).getByLabel("内存已用容量")).toHaveText("8 GB");
  await expect(overview(page)).not.toContainText(/(?:Ki|Mi|Gi|Ti)B\b/);
  await disks.getByRole("button", { name: "存储详情", exact: true }).click();
  const detail = page.getByRole("dialog", { name: "存储详细信息", exact: true });
  await expect(detail.getByText("247 GB", { exact: true })).toBeVisible();
  await expect(detail.getByText("852 MB", { exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "GPU 规格", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "GPU 硬件参数" }).getByText("7.81 GB", { exact: true })).toBeVisible();
});

test("磁盘卷的容量条与百分比按显示的已用百分比分档着色", async ({ page }) => {
  await setup(page, [
    volume("B:", null, 1000 * GB, 206 * GB), volume("W:", null, 1000 * GB, 204 * GB),
    volume("X:", null, 1000 * GB, 106 * GB), volume("Y:", null, 1000 * GB, 104 * GB),
    volume("R:", "离线卷", 0, 0),
  ]);
  const disks = storage(page);
  const item = mount => disks.locator("li").filter({ hasText: `(${mount})` });
  const colors = mount => item(mount).evaluate(el => {
    const probe = document.createElement("span");
    el.append(probe);
    const token = name => { probe.style.color = `var(${name})`; return getComputedStyle(probe).color; };
    const value = {
      fill: getComputedStyle(el.querySelector(".ccc-capacity-fill")).backgroundColor,
      percent: getComputedStyle(el.querySelector(".ccc-storage-heading > .ccc-capacity-available")).color,
      caption: getComputedStyle(el.querySelector(".ccc-storage-info > .ccc-capacity-available")).color,
      blue: token("--ccc-chart-blue"), warning: token("--ccc-chart-warning"), alert: token("--ccc-chart-alert"), label: token("--ccc-chart-label"),
    };
    probe.remove();
    return value;
  });
  for (const [mount, percent, level, fill, ink] of [["B:", "79%", null, "blue", "label"], ["W:", "80%", "warning", "warning", "warning"], ["X:", "89%", "warning", "warning", "warning"], ["Y:", "90%", "alert", "alert", "alert"]]) {
    await expect(item(mount).locator(".ccc-storage-heading > .ccc-capacity-available")).toHaveText(percent);
    if (level) await expect(item(mount)).toHaveAttribute("data-usage", level);
    else await expect(item(mount)).not.toHaveAttribute("data-usage", /.+/);
    const actual = await colors(mount);
    expect(actual).toMatchObject({ fill: actual[fill], percent: actual[ink], caption: actual.label });
  }
  await expect(item("R:")).not.toHaveAttribute("data-usage", /.+/);
  await disks.getByRole("button", { name: "存储详情", exact: true }).click();
  const detail = page.getByRole("dialog", { name: "存储详细信息", exact: true });
  await expect(detail.getByText("用到 80% 起提醒留意，90% 起建议清理空间。")).toBeVisible();
});

test("内存趋势随回读改变，保留采样缺口和五分钟时间范围", async ({ page }) => {
  await page.clock.install();
  await setup(page);
  const ram = memory(page);
  const chart = memoryChart(page);
  const utilization = ram.locator(".ccc-overview-utilization");
  const meter = utilization.getByRole("meter", { name: "当前内存使用率", exact: true });
  const fill = utilization.locator(".ccc-meter-ring-fill");
  const rate = utilization.locator("output");
  const line = chart.locator('path[data-series="内存"]');
  const area = chart.locator(".ccc-trend-area");
  await expect(rate).toHaveText("50 %");
  await expect(meter).toHaveAttribute("aria-valuenow", "50");
  await expect(fill).toHaveAttribute("stroke-dasharray", "50 100");
  await expect(chart.getByText("当前使用率", { exact: true })).toHaveCount(0);
  await expect(ram.getByText(/^(?:5 分钟前|现在)$/)).toHaveCount(0);
  await expect(chart.getByText(/^采样 \d{2}:\d{2}:\d{2}$/)).toHaveCount(0);
  await expect(chart.getByText(/^\d{2}:\d{2}:\d{2}$/)).toHaveCount(0);
  await expect(line).toHaveAttribute("d", "M100.000 50.000");
  await expect(area).toHaveCount(0);
  await expect(ram.getByText("总容量", { exact: true })).toBeVisible();
  await sampleMemory(page, 4 * GB);
  await expect(rate).toHaveText("75 %");
  await expect(fill).toHaveAttribute("stroke-dasharray", "75 100");
  await expect(line).toHaveAttribute("d", /^M[\d.]+ 50\.000 L100\.000 25\.000$/);
  await expect(area).toHaveAttribute("d", / L100\.000 100 L[\d.]+ 100 Z$/);
  await expect(ram.getByLabel("内存已用容量")).toHaveText("12 GB");
  await expect(ram.getByText("4 GB", { exact: true })).toBeVisible();

  await sampleMemory(page, null);
  await expect(rate).toHaveText("—");
  await expect(utilization.getByRole("img", { name: "当前内存使用率：暂无读数", exact: true })).toBeVisible();
  await expect(fill).toHaveCount(0);
  await expect(ram.getByLabel("内存已用容量")).toHaveText("未知");
  await expect(ram.getByLabel("内存频率", { exact: true })).toHaveText("5200 MT/s");
  await expect(line).toHaveAttribute("d", /^M[\d.]+ 50\.000 L[\d.]+ 25\.000$/);

  await sampleMemory(page, 12 * GB);
  await expect(rate).toHaveText("25 %");
  await expect(line).toHaveAttribute("d", /25\.000 M100\.000 75\.000$/);
  await page.clock.fastForward(20_000);
  await expect(line).toHaveAttribute("d", /75\.000 M100\.000 75\.000$/);

  await page.evaluate(gigabyte => { window.__overviewMemory = { totalBytes: 16 * gigabyte, availableBytes: 16 * gigabyte }; }, GB);
  await page.clock.fastForward(300_001);
  await expect(rate).toHaveText("0 %");
  await expect(meter).toHaveAttribute("aria-valuenow", "0");
  await expect(fill).toHaveCount(0);
  await expect(line).toHaveAttribute("d", "M100.000 100.000");
  await expect(area).toHaveCount(0);
});

test("多磁盘时内存与存储上下排列，磁盘卷随宽度排布", async ({ page }) => {
  await page.clock.install();
  await setup(page);
  await expect(storage(page).getByRole("listitem")).toHaveCount(6);
  for (let index = 0; index < 51; index++) {
    const used = 9.5 + 0.09 * index + Math.sin(index / 3.5) * 0.59 + Math.sin(index / 1.7) * 0.18;
    await sampleMemory(page, index === 19 || index === 20 ? null : Math.round((16 - used) * GB));
  }
  await expect(memoryChart(page).locator(".ccc-trend-area")).toHaveAttribute("d", /Z M/);
  await page.evaluate(async () => {
    const sample = window.__uiHardwareSnapshot();
    for (let index = 1; index <= 300; index++) {
      if (index >= 122 && index <= 137) continue;
      const cpuTemp = Math.round(48 + Math.min(index, 210) * 0.238 + Math.sin(index / 6) * 2);
      const tempC = index >= 235 && index <= 248 ? null : Math.round(44 + Math.min(index, 200) * 0.218 + Math.sin(index / 8) * 1.5);
      window.__uiEmit("hardware-snapshot", { ...sample, sampledAtMs: sample.sampledAtMs + index * 1000,
        fan: { ...sample.fan, cpu: { ...sample.fan.cpu, tempC: cpuTemp } }, nvidia: { ...sample.nvidia, tempC } });
    }
  });
  await expect(sensor(page, "CPU").getByLabel("CPU 当前温度")).toHaveAttribute("data-alert", "true");
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    for (const width of [360, 900, 1440]) {
      await page.setViewportSize({ width, height: 900 });
      const [ram, disk] = await Promise.all([memory(page).boundingBox(), storage(page).boundingBox()]);
      expect(disk.y).toBeGreaterThan(ram.y + ram.height);
      expect(disk.x).toBeCloseTo(ram.x, 0);
      expect(disk.width).toBeCloseTo(ram.width, 0);
      const cells = await storage(page).getByRole("listitem").evaluateAll(items => items.map(item => {
        const right = item.getBoundingClientRect().right;
        const info = item.querySelector(".ccc-storage-info");
        const mount = item.querySelector("h4 > :last-child");
        return { x: item.offsetLeft, right, fits: info.scrollWidth <= info.clientWidth + 1 && mount.getBoundingClientRect().right <= right + 1 };
      }));
      const columns = new Set(cells.map(item => item.x)).size;
      if (width === 360) expect(columns).toBe(1);
      else expect(columns).toBeGreaterThanOrEqual(width === 1440 ? 3 : 2);
      expect(cells.every(item => item.right <= width && item.fits)).toBe(true);
      for (const part of ["CPU", "GPU"]) {
        const plot = temperature(page, part);
        await expect(plot).toBeVisible();
        const chartFits = await plot.evaluate(element => {
          const card = element.closest("section");
          const frame = card.getBoundingClientRect();
          return card.scrollWidth <= card.clientWidth + 1 &&
            [element, ...element.querySelectorAll(".ccc-trend-marker")].every(item => {
              const bounds = item.getBoundingClientRect();
              return bounds.left >= frame.left && bounds.right <= frame.right;
            });
        });
        expect(chartFits).toBe(true);
        if (width === 360 || width === 1440) {
          await plot.screenshot({ path: test.info().outputPath(`overview-temperature-${part}-${theme}-${width}.png`) });
        }
      }
      if (width === 1440) {
        await sensor(page, "CPU").locator("..").screenshot({ path: test.info().outputPath(`overview-processor-trends-${theme}.png`) });
        await memory(page).locator("..").screenshot({ path: test.info().outputPath(`overview-capacity-trend-${theme}.png`) });
        await memory(page).screenshot({ path: `target/ui-check/overview-memory-trend-${theme}.png` });
      }
    }
  }
  const repeatWrites = await page.evaluate(() => window.__uiFixture.calls.filter(({ command }) => /^(apply_|restore_|save_)/.test(command)));
  expect(repeatWrites).toEqual([]);
});
