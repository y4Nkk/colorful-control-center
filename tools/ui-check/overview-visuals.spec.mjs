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

async function setup(page) {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(volumes => {
    const original = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      const reply = await original(command, args);
      if (command === "read_system_inventory") return { ...reply, volumes };
      if (command === "read_memory_capacity" && window.__overviewMemory !== undefined) {
        if (window.__overviewMemory === null) throw new Error("隔离内存读取失败");
        return window.__overviewMemory;
      }
      return reply;
    };
  }, volumes);
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
  await expect(mode).toHaveCSS("background-image", /linear-gradient/);
  await expect(art).toHaveAttribute("data-preset-palette", "performance");
  for (const [value, name, palette] of [[0, "安静", "quiet"], [99, "未知", "neutral"], [null, "未知", "neutral"], [4, "静音增强", "quiet"]]) {
    await page.clock.fastForward(30);
    await publishMode(page, value);
    await expect(mode).toHaveText(name);
    await expect(art).toHaveAttribute("data-preset-palette", palette);
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
  expect(await incoming.locator(".ccc-preset-orbit-front").evaluate(path => path.getAnimations()[0].animationName))
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
    await plot.scrollIntoViewIfNeeded();
    await expect(plot).toBeInViewport({ ratio: 1 });
  }
  await page.setViewportSize({ width: 360, height: 800 });
  await expectCenteredArtwork(card);
  await card.screenshot({ path: test.info().outputPath("overview-chip-dark-360.png") });
});

test("温度趋势与图标读数分区，规格与详情使用可键盘操作的文字入口", async ({ page }) => {
  await setup(page);
  const cpu = sensor(page, "CPU");
  const gpu = sensor(page, "GPU");
  const panels = [[cpu, "CPU 规格", "CPU 硬件参数"], [gpu, "GPU 规格", "GPU 硬件参数"],
    [memory(page), "内存详情", "内存详细信息"], [storage(page), "存储详情", "存储详细信息"]];
  await expect(cpu.getByRole("meter", { name: "CPU 利用率" })).toBeVisible();
  const sampleLayout = () => page.evaluate(async () => {
    const sample = window.__uiHardwareSnapshot();
    window.__uiEmit("hardware-snapshot", { ...sample, oemPowerMode: 3, fan: { ...sample.fan, cpu: { ...sample.fan.cpu, dutyPct: 100 } },
      nvidia: { ...sample.nvidia, memUsedMb: 2171, memTotalMb: 8192 } });
  });
  await sampleLayout();
  const vram = gpu.getByRole("meter", { name: "显存占用", exact: true });
  await expect(vram).toHaveAttribute("aria-valuetext", "2.12 GB / 8 GB");
  await expect(vram.locator("output")).toHaveText("2.12 GB");
  await expect(vram.getByText("共 8 GB", { exact: true })).toHaveCount(0);
  await expect(overview(page).getByText(/^采样 \d{2}:\d{2}:\d{2}$/)).toHaveCount(0);
  for (const [panel, label, title] of panels) {
    const info = panel.getByRole("button", { name: label, exact: true });
    await expect(info).toHaveText(label);
    await expect(info.locator("svg")).toHaveCount(0);
    await info.focus();
    await info.press("Enter");
    await expect(page.getByRole("dialog", { name: title, exact: true })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(info).toBeFocused();
  }

  for (const [theme, width] of [["light", 1440], ["dark", 360]]) {
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    await page.setViewportSize({ width, height: 900 });
    for (const [panel, artCount, textCount] of [[cpu, 2, 4], [gpu, 2, 4], [memory(page), 1, 2]]) {
      const layout = await panel.locator(".ccc-hardware-identity").evaluate(header => {
        const art = [...header.querySelectorAll(".ccc-hardware-art, .ccc-hardware-brand")].map(el => el.getBoundingClientRect());
        const text = [...header.querySelectorAll(".ccc-hardware-description > :is(h3, p, button)")].map(el => el.getBoundingClientRect());
        return { art: art.length, text: text.length, artOnLeft: art.every(box => box.right < text[0].left),
          stacked: text.every((box, i) => i === 0 || (box.top >= text[i - 1].bottom && Math.abs(box.left - text[0].left) < 1)) };
      });
      expect(layout).toEqual({ art: artCount, text: textCount, artOnLeft: true, stacked: true });
    }
    for (const [panel, count] of [[cpu, 3], [gpu, 4]]) {
      await expect(panel.locator(".ccc-meter-ring")).toHaveCount(0);
      const icons = panel.locator(".ccc-meter-compact .ccc-meter-heading > svg");
      await expect(icons).toHaveCount(count);
      expect(await icons.evaluateAll(items => items.map(icon => icon.dataset.icon)))
        .toEqual(count === 3 ? ["activity", "performance", "fan"] : ["activity", "performance", "fan", "memory"]);
      await expect(icons.first()).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
      const chart = await panel.locator(".ccc-trend-panel").boundingBox();
      const grid = await panel.locator(".ccc-sensor-metrics").boundingBox();
      const cells = await panel.locator(".ccc-meter-compact").evaluateAll(items => items.map(item => ({
        y: item.getBoundingClientRect().top, bottom: item.querySelector("output").getBoundingClientRect().bottom,
        fits: item.scrollWidth <= item.clientWidth + 1,
      })));
      expect(grid.y).toBeGreaterThan(chart.y + chart.height);
      expect(cells.every(cell => cell.fits)).toBe(true);
      const rows = new Map(cells.map(({ y, bottom }) => [y, bottom]));
      expect(rows.size).toBe(width === 1440 ? 1 : 2);
      expect(cells.every(({ y, bottom }) => Math.abs(rows.get(y) - bottom) < 1)).toBe(true);
      const card = await panel.boundingBox();
      expect(card.y + card.height - Math.max(...cells.map(cell => cell.bottom))).toBeCloseTo(13, 0);
    }
    if (width === 1440) {
      for (const selector of [".ccc-trend-plot", ".ccc-sensor-metrics"]) {
        const [left, right] = await Promise.all([cpu.locator(selector).boundingBox(), gpu.locator(selector).boundingBox()]);
        expect(left.y).toBeCloseTo(right.y, 0);
      }
      for (const panel of [cpu, gpu, memory(page)]) {
        const [chart, readout] = await Promise.all([panel.locator(".ccc-trend-plot").boundingBox(),
          panel.locator(".ccc-sensor-metric-grid, .ccc-memory-stats > :first-child").boundingBox()]);
        expect(chart.x).toBeCloseTo(readout.x, 0);
      }
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
  await expect(latestCpu).toHaveText("50°C");
  await expect(cpuPlot.locator(".ccc-trend-marker")).toHaveCount(1);
  await expect(cpuPlot.locator(".ccc-trend-area")).toHaveCount(0);
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
  await expect(latestCpu).toHaveText("94.8°C");
  await expect(latestCpu).not.toHaveClass(/is-hot/);
  await expect(latestGpu).toHaveText("89°C");
  await expect(sensor(page, "CPU").locator(".ccc-trend-peak")).toContainText("98 °C");
  await expect(cpuPlot.getByText("5 分钟前", { exact: true })).toBeVisible();
  await expect(cpuPlot.getByText("现在", { exact: true })).toBeVisible();
  await expect(cpuPlot.locator(".ccc-temperature-limit")).toHaveAttribute("d", "M0 16.667H100");
  for (const degree of [98, 95]) {
    const marker = cpuPlot.locator(".ccc-trend-marker").and(cpuPlot.locator(`[title$=" · ${degree} °C"]`));
    await expect(marker).toHaveAttribute("data-hot", "true");
  }
  await expect(gpuPlot.locator(".ccc-trend-marker[data-hot]")).toHaveCount(0);
  await expect(cpuLine).toHaveAttribute("d", /^M95\.500 [\d.]+ L95\.667 100\.000 M96\.333 [\d.]+ M99\.333 [\d.]+ L99\.667 [\d.]+ L100\.000 [\d.]+$/);
  await expect(gpuLine).toHaveAttribute("d", /^M95\.500 [\d.]+ M96\.000 [\d.]+ L96\.333 [\d.]+ M99\.333 [\d.]+ L99\.667 [\d.]+ L100\.000 [\d.]+$/);
  await expect(cpuPlot.locator(".ccc-trend-area")).toHaveAttribute("d", /L95\.667 100 L95\.500 100 Z M99\.333 .*L100\.000 100 L99\.333 100 Z$/);
  await page.evaluate(() => {
    const sample = window.__overviewSnapshot;
    window.__uiEmit("hardware-snapshot", {
      ...sample, sampledAtMs: sample.sampledAtMs + 14500, fan: { ...sample.fan, cpu: { ...sample.fan.cpu, tempC: null } },
      nvidia: null, nvidiaSuspended: true,
    });
  });
  await expect(latestCpu).toHaveText("—");
  await expect(latestGpu).toHaveText("—");
  await expect(cpuLine).toHaveAttribute("d", /L99\.667 [\d.]+$/);
  await expect(gpuLine).toHaveAttribute("d", /L99\.667 [\d.]+$/);
});

test("每个磁盘卷的横条、详情和显存沿用真实容量与统一单位", async ({ page }) => {
  await setup(page);
  const disks = storage(page);
  await expect(disks.getByRole("listitem")).toHaveCount(6);
  await expect(disks.getByRole("heading", { name: "系统 (C:)", exact: true })).toBeVisible();
  await expect(disks.getByRole("heading", { name: "本地磁盘 (E:)", exact: true })).toBeVisible();
  await expect(disks.getByText("213 GB 可用 · 共 460 GB", { exact: true })).toBeVisible();
  await expect(disks.getByText("852 MB 可用 · 共 922 MB", { exact: true })).toBeVisible();
  await expect(disks.getByText("2 KB 可用 · 共 4 KB", { exact: true })).toBeVisible();
  const meter = mount => disks.getByRole("meter", { name: `${mount} 使用量`, exact: true });
  await expect(meter("C:")).toHaveAttribute("aria-valuenow", String(247 * GB));
  const width = await meter("C:").locator(".ccc-capacity-fill").getAttribute("width");
  expect(Number(width)).toBeCloseTo(247 / 460 * 100, 3);
  await expect(meter("E:").locator(".ccc-capacity-fill")).toHaveAttribute("width", "0");
  await expect(disks.getByText("1 TB 可用 · 共 1 TB", { exact: true })).toBeVisible();
  await expect(meter("R:")).toHaveCount(0);
  await expect(disks.getByText("容量未知", { exact: true })).toBeVisible();
  await expect(memory(page).getByLabel("内存已用容量")).toHaveText("8 GB");
  await expect(page.getByRole("meter", { name: "显存占用", exact: true })).toHaveAttribute("aria-valuetext", "1000 MB / 7.81 GB");
  await expect(overview(page)).not.toContainText(/(?:Ki|Mi|Gi|Ti)B\b/);
  await disks.getByRole("button", { name: "存储详情", exact: true }).click();
  const detail = page.getByRole("dialog", { name: "存储详细信息", exact: true });
  await expect(detail.getByText("247 GB", { exact: true })).toBeVisible();
  await expect(detail.getByText("852 MB", { exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "GPU 规格", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "GPU 硬件参数" }).getByText("7.81 GB", { exact: true })).toBeVisible();
});

test("内存趋势随回读改变，保留采样缺口和五分钟时间范围", async ({ page }) => {
  await page.clock.install();
  await setup(page);
  const ram = memory(page);
  const chart = memoryChart(page);
  const rate = ram.getByLabel("当前内存使用率");
  const peak = ram.locator(".ccc-trend-peak strong");
  const line = chart.locator('path[data-series="内存"]');
  const area = chart.locator(".ccc-trend-area");
  await expect(rate).toHaveText("50%");
  await expect(peak).toHaveText("50%");
  await expect(chart.getByText("5 分钟前", { exact: true })).toBeVisible();
  await expect(chart.getByText("现在", { exact: true })).toBeVisible();
  await expect(chart.getByText(/^采样 \d{2}:\d{2}:\d{2}$/)).toHaveCount(0);
  await expect(chart.getByText(/^\d{2}:\d{2}:\d{2}$/)).toHaveCount(0);
  await expect(line).toHaveAttribute("d", "M100.000 50.000");
  await expect(area).toHaveCount(0);
  await expect(ram.getByText("总容量", { exact: true })).toBeVisible();
  await sampleMemory(page, 4 * GB);
  await expect(rate).toHaveText("75%");
  await expect(peak).toHaveText("75%");
  await expect(line).toHaveAttribute("d", /^M[\d.]+ 50\.000 L100\.000 25\.000$/);
  await expect(area).toHaveAttribute("d", / L100\.000 100 L[\d.]+ 100 Z$/);
  await expect(ram.getByLabel("内存已用容量")).toHaveText("12 GB");
  await expect(ram.getByText("4 GB", { exact: true })).toBeVisible();

  await sampleMemory(page, null);
  await expect(rate).toHaveText("—");
  await expect(peak).toHaveText("75%");
  await expect(ram.getByLabel("内存已用容量")).toHaveText("未知");
  await expect(ram.getByLabel("内存频率", { exact: true })).toHaveText("5200 MT/s");
  await expect(line).toHaveAttribute("d", /^M[\d.]+ 50\.000 L[\d.]+ 25\.000$/);

  await sampleMemory(page, 12 * GB);
  await expect(rate).toHaveText("25%");
  await expect(line).toHaveAttribute("d", /25\.000 M100\.000 75\.000$/);
  await page.clock.fastForward(20_000);
  await expect(line).toHaveAttribute("d", /75\.000 M100\.000 75\.000$/);

  await page.evaluate(gigabyte => { window.__overviewMemory = { totalBytes: 16 * gigabyte, availableBytes: 16 * gigabyte }; }, GB);
  await page.clock.fastForward(300_001);
  await expect(rate).toHaveText("0%");
  await expect(peak).toHaveText("0%");
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
  await expect(sensor(page, "CPU").getByLabel("CPU 当前温度")).toHaveClass(/is-hot/);
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
