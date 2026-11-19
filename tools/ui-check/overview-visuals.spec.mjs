import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const overview = page => page.getByRole("region", { name: "概览", exact: true });
const temperature = page => page.getByRole("region", { name: "温度趋势", exact: true });
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
      return command === "read_system_overview" ? {
        ...reply, volumes, memory: window.__overviewMemory === undefined ? reply.memory : window.__overviewMemory,
      } : reply;
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

test("当前模式只显示设备采样的名称与单张插画，空值或停采显示未知", async ({ page }) => {
  await page.clock.install();
  await setup(page);
  const modeCard = overview(page).getByRole("region", { name: "当前模式", exact: true });
  const mode = modeCard.getByRole("status");
  const art = modeCard.locator("svg[data-mode-art]");
  await expect(mode).toHaveText("性能");
  await expect(art).toHaveAttribute("data-mode-art", "performance");
  for (const [value, name, drawing] of [[0, "安静", "quiet"], [null, "未知", null], [4, "静音增强", "quiet"]]) {
    await page.clock.fastForward(30);
    await page.evaluate(async value => {
      const sample = await window.__TAURI_INTERNALS__.invoke("read_hardware_snapshot");
      window.__uiEmit("hardware-snapshot", { ...sample, oemPowerMode: value });
    }, value);
    await expect(mode).toHaveText(name);
    if (drawing) await expect(art).toHaveAttribute("data-mode-art", drawing);
    else await expect(art).toHaveCount(0);
  }
  await page.clock.fastForward(4_000);
  await expect(mode).toHaveText("未知");
  await expect(art).toHaveCount(0);
  await expect(modeCard.locator("button, input")).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(({ command }) =>
    /^(read|apply|restore)_oem_power_mode$/.test(command)))).toEqual([]);
});

test("环形读数和单位对齐，四个信息图标可用键盘打开规格", async ({ page }) => {
  await setup(page);
  const cpu = overview(page).getByRole("region", { name: "CPU 散热读数", exact: true });
  const gpu = overview(page).getByRole("region", { name: "GPU 运行读数", exact: true });
  const panels = [[cpu, "CPU 详细信息", "CPU 硬件参数"], [gpu, "GPU 详细信息", "GPU 硬件参数"],
    [memory(page), "内存详细信息", "内存详细信息"], [storage(page), "存储详细信息", "存储详细信息"]];
  await expect(cpu.getByRole("meter", { name: "CPU 利用率" })).toBeVisible();
  const sampleLayout = () => page.evaluate(async () => {
    const sample = await window.__TAURI_INTERNALS__.invoke("read_hardware_snapshot");
    window.__uiEmit("hardware-snapshot", { ...sample, oemPowerMode: 3, fan: { ...sample.fan, cpuDutyPct: 100 },
      nvidia: { ...sample.nvidia, memUsedMb: 2171, memTotalMb: 8192 } });
  });
  await sampleLayout();
  await expect(gpu.getByRole("meter", { name: "显存占用", exact: true })).toHaveAttribute("aria-valuetext", "2.12 GB / 8 GB");
  for (const [panel, label, title] of panels) {
    const info = panel.getByRole("button", { name: label, exact: true });
    await expect(info).toBeEmpty();
    await expect(info.locator("svg")).toHaveCount(1);
    await info.focus();
    await info.press("Enter");
    await expect(page.getByRole("dialog", { name: title, exact: true })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(info).toBeFocused();
  }

  for (const [theme, width] of [["light", 1440], ["dark", 360]]) {
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    await page.setViewportSize({ width, height: 900 });
    const ratios = await overview(page).locator(".ccc-sensor-ratios .ccc-meter-ring").evaluateAll(rings => rings.map(ring => {
      const outer = ring.getBoundingClientRect();
      const value = ring.querySelector("output > :first-child").getBoundingClientRect();
      const unit = ring.querySelector(".ccc-meter-unit").getBoundingClientRect();
      return { top: outer.y, valueY: value.y - outer.y, unitY: unit.y - outer.y,
        centerOffset: (value.left + value.right - outer.left - outer.right) / 2,
        inside: value.left > outer.left + 4 && value.right < outer.right - 4 };
    }));
    expect(ratios).toHaveLength(5);
    expect(ratios.every(ring => ring.inside && Math.abs(ring.centerOffset) < 1)).toBe(true);
    for (const ring of ratios) {
      expect(Math.abs(ring.valueY - ratios[0].valueY)).toBeLessThan(1);
      expect(Math.abs(ring.unitY - ratios[0].unitY)).toBeLessThan(1);
      if (width === 1440) expect(Math.abs(ring.top - ratios[0].top)).toBeLessThan(1);
    }
    for (const panel of [cpu, gpu]) {
      const image = await panel.locator(".ccc-hardware-art").boundingBox();
      const button = await panel.getByRole("button").boundingBox();
      const card = await panel.boundingBox();
      expect(button.x).toBeGreaterThan(image.x + image.width);
      expect(button.x + button.width).toBeLessThan(card.x + card.width);
      expect(Math.abs(button.y + button.height / 2 - image.y - image.height / 2)).toBeLessThan(1);
    }
    const modeCard = overview(page).getByRole("region", { name: "当前模式", exact: true });
    await sampleLayout();
    await expect(modeCard.getByRole("status")).toHaveText("娱乐");
    await modeCard.screenshot({ path: `target/ui-check/overview-current-mode-${theme}-${width}.png` });
    await cpu.locator("..").screenshot({ path: `target/ui-check/overview-processor-readouts-${theme}-${width}.png` });
  }
});

test("温度时间带按实测时刻定位，保留短时高温、缺测和当前空值", async ({ page }) => {
  await setup(page);
  const region = temperature(page), latestCpu = region.getByLabel("CPU 当前温度");
  const latestGpu = region.getByLabel("GPU 当前温度");
  const track = series => region.getByRole("img", { name: new RegExp(`^${series} 温度时间带`) });
  const cpuSamples = track("CPU").locator(".ccc-temperature-sample");
  const gpuSamples = track("GPU").locator(".ccc-temperature-sample");
  await expect(latestCpu).toHaveText("50°C");
  await expect(cpuSamples).toHaveCount(1);
  const latestClock = await page.evaluate(async () => {
    const sample = await window.__TAURI_INTERNALS__.invoke("read_hardware_snapshot");
    const start = sample.sampledAtMs + 301_000;
    window.__overviewSnapshot = { ...sample, sampledAtMs: start };
    for (const [offset, cpu, gpu] of [[0, 45, 49], [500, 0, null], [1500, 60.5, 54], [11500, 98, 87], [12500, 95, 88], [13500, 94.8, 89]]) {
      window.__uiEmit("hardware-snapshot", {
        ...sample, sampledAtMs: start + offset, cpuTempC: cpu, nvidia: { ...sample.nvidia, tempC: gpu },
      });
    }
    return new Date(start + 13500).toLocaleTimeString("zh-CN", { hour12: false });
  });
  await expect(latestCpu).toHaveText("94.8°C");
  await expect(latestCpu).not.toHaveClass(/is-hot/);
  await expect(latestGpu).toHaveText("89°C");
  await expect(cpuSamples).toHaveCount(6);
  await expect(gpuSamples).toHaveCount(5);
  await expect(track("CPU").locator(".is-hot")).toHaveCount(2);
  await expect(track("GPU").locator(".is-hot")).toHaveCount(0);
  await expect(cpuSamples.nth(1)).toHaveAttribute("data-temperature", "0");
  await expect(cpuSamples.nth(3)).toHaveAttribute("title", / · 98 °C$/);
  await expect(region.getByText("≥95°C", { exact: true })).toBeVisible();
  await expect(region.getByText(`采样 ${latestClock}`, { exact: true })).toHaveCount(1);
  await expect(region.getByText(/^\d{2}:\d{2}:\d{2}$/)).toHaveCount(0);
  const positions = await cpuSamples.evaluateAll(items => items.map(item => ({
    left: parseFloat(item.style.left), width: parseFloat(item.style.width),
  })));
  expect(positions[1].width).toBeCloseTo(500 / 300_000 * 100, 5);
  expect(positions[3].left - positions[2].left - positions[2].width).toBeCloseTo(9000 / 300_000 * 100, 3);
  expect(positions[5].left + positions[5].width).toBeCloseTo(100, 3);
  await page.evaluate(() => {
    const sample = window.__overviewSnapshot;
    window.__uiEmit("hardware-snapshot", {
      ...sample, sampledAtMs: sample.sampledAtMs + 14500, cpuTempC: null, nvidia: null, nvidiaSuspended: true,
    });
  });
  await expect(latestCpu).toHaveText("—");
  await expect(latestGpu).toHaveText("—");
  await expect(cpuSamples).toHaveCount(6);
  await expect(gpuSamples).toHaveCount(5);
  const remaining = await cpuSamples.last().evaluate(item => parseFloat(item.style.left) + parseFloat(item.style.width));
  expect(100 - remaining).toBeCloseTo(1000 / 300_000 * 100, 3);
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
  await disks.getByRole("button", { name: "存储详细信息", exact: true }).click();
  const detail = page.getByRole("dialog", { name: "存储详细信息", exact: true });
  await expect(detail.getByText("247 GB", { exact: true })).toBeVisible();
  await expect(detail.getByText("852 MB", { exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "GPU 详细信息", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "GPU 硬件参数" }).getByText("7.81 GB", { exact: true })).toBeVisible();
});

test("内存趋势随回读改变，保留采样缺口和五分钟时间范围", async ({ page }) => {
  await page.clock.install();
  await setup(page);
  const ram = memory(page);
  const chart = memoryChart(page);
  const rate = ram.getByLabel("当前内存使用率");
  const line = chart.locator('path[data-series="内存"]');
  const area = chart.locator(".ccc-memory-area");
  await expect(rate).toHaveText("50%");
  await expect(chart.getByText(/^采样 \d{2}:\d{2}:\d{2}$/)).toHaveCount(1);
  await expect(chart.getByText(/^\d{2}:\d{2}:\d{2}$/)).toHaveCount(0);
  await expect(line).toHaveAttribute("d", "M100.000 50.000");
  await expect(area).toHaveCount(0);
  await expect(ram.getByText("总容量", { exact: true })).toBeVisible();
  await sampleMemory(page, 4 * GB);
  await expect(rate).toHaveText("75%");
  await expect(line).toHaveAttribute("d", /^M[\d.]+ 50\.000 L100\.000 25\.000$/);
  await expect(area).toHaveAttribute("d", / L100\.000 100 L[\d.]+ 100 Z$/);
  await expect(ram.getByLabel("内存已用容量")).toHaveText("12 GB");
  await expect(ram.getByText("4 GB", { exact: true })).toBeVisible();

  await sampleMemory(page, null);
  await expect(rate).toHaveCount(0);
  await expect(ram.getByText("容量未知", { exact: true })).toBeVisible();
  await expect(line).toHaveAttribute("d", /^M[\d.]+ 50\.000 L[\d.]+ 25\.000$/);

  await sampleMemory(page, 12 * GB);
  await expect(rate).toHaveText("25%");
  await expect(line).toHaveAttribute("d", /25\.000 M100\.000 75\.000$/);
  await page.clock.fastForward(20_000);
  await expect(line).toHaveAttribute("d", /75\.000 M100\.000 75\.000$/);

  await page.evaluate(gigabyte => { window.__overviewMemory = { totalBytes: 16 * gigabyte, availableBytes: 16 * gigabyte }; }, GB);
  await page.clock.fastForward(300_001);
  await expect(rate).toHaveText("0%");
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
  await expect(memoryChart(page).locator(".ccc-memory-area")).toHaveAttribute("d", /Z M/);
  await page.evaluate(async () => {
    const sample = await window.__TAURI_INTERNALS__.invoke("read_hardware_snapshot");
    for (let index = 1; index <= 300; index++) {
      if (index >= 122 && index <= 137) continue;
      const cpuTempC = Math.round(48 + Math.min(index, 210) * 0.238 + Math.sin(index / 6) * 2);
      const tempC = index >= 235 && index <= 248 ? null : Math.round(44 + Math.min(index, 200) * 0.218 + Math.sin(index / 8) * 1.5);
      window.__uiEmit("hardware-snapshot", { ...sample, sampledAtMs: sample.sampledAtMs + index * 1000,
        cpuTempC, nvidia: { ...sample.nvidia, tempC } });
    }
  });
  await expect(temperature(page).getByLabel("CPU 当前温度")).toHaveClass(/is-hot/);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    for (const width of [360, 900, 1440]) {
      await page.setViewportSize({ width, height: 900 });
      const [ram, disk] = await Promise.all([memory(page).boundingBox(), storage(page).boundingBox()]);
      expect(disk.y).toBeGreaterThan(ram.y + ram.height);
      expect(disk.x).toBeCloseTo(ram.x, 0);
      expect(disk.width).toBeCloseTo(ram.width, 0);
      const [stats, chart] = await Promise.all([
        memory(page).locator(".ccc-memory-stats").boundingBox(), memoryChart(page).boundingBox(),
      ]);
      if (width === 360) expect(chart.y).toBeGreaterThan(stats.y + stats.height);
      else expect(chart.x).toBeGreaterThan(stats.x + stats.width);
      expect(chart.x + chart.width).toBeLessThan(ram.x + ram.width);
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
      const chartFits = await temperature(page).evaluate(element => element.scrollWidth <= element.clientWidth + 1);
      expect(chartFits).toBe(true);
      const tracks = await temperature(page).locator(".ccc-temperature-track").evaluateAll(items => items.map(item => {
        const { left, width } = item.getBoundingClientRect();
        return { left, width };
      }));
      expect(tracks).toHaveLength(2);
      expect(tracks[0]).toEqual(tracks[1]);
      if (width === 360) await temperature(page).screenshot({ path: `target/ui-check/overview-temperature-${theme}-360.png` });
      if (width === 1440) {
        await temperature(page).screenshot({ path: `target/ui-check/overview-temperature-${theme}.png` });
        await page.getByRole("region", { name: "系统内存概览" }).locator("..").screenshot({ path: `target/ui-check/overview-capacity-trend-${theme}.png` });
      }
    }
  }
  const repeatWrites = await page.evaluate(() => window.__uiFixture.calls.filter(({ command }) => /^(apply_|restore_|save_)/.test(command)));
  expect(repeatWrites).toEqual([]);
});
