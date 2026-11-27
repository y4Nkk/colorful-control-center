import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";
import { previewConfiguration, submitConfiguration } from "./configuration-review.mjs";

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: `${MOCK_IPC}
    window.__uiFixture.calibrationSupported = true;
    if (sessionStorage.getItem("standard-account") === "true") window.__uiFixture.administratorAccess = "standard";` });
  await page.goto("/");
});

for (const { name, leftNeighbor, rightNeighbor, switchCount, systemArea, wideColumns = switchCount } of [
  { name: "键盘功能", leftNeighbor: "显卡模式", rightNeighbor: "飞行模式", switchCount: 3 },
  { name: "散热控制", leftNeighbor: "CPU 功耗与电压", rightNeighbor: "GPU 频率", switchCount: 2, systemArea: "性能与散热", wideColumns: 1 },
]) {
  test(`${name}在宽窄窗口下与相邻模块对齐`, async ({ page }) => {
    await openWorkspace(page, systemArea ?? "设备配置");
    const panel = page.getByRole("region", { name, exact: true });
    const leftPanel = page.getByRole("region", { name: leftNeighbor, exact: true });
    const rightPanel = page.getByRole("region", { name: rightNeighbor, exact: true });
    const switches = panel.getByRole("switch");
    await expect(switches).toHaveCount(switchCount);

    for (const width of [1440, 360]) {
      await page.setViewportSize({ width, height: 900 });
      const [panelBox, leftBox, rightBox] = await Promise.all([panel.boundingBox(), leftPanel.boundingBox(), rightPanel.boundingBox()]);
      expect(panelBox.x, `${width}px 左边缘`).toBeCloseTo(leftBox.x, 0);
      expect(panelBox.x + panelBox.width, `${width}px 右边缘`).toBeCloseTo(rightBox.x + rightBox.width, 0);

      const columns = await switches.evaluateAll(items => new Set(items.map(item => Math.round(item.getBoundingClientRect().left))).size);
      expect(columns).toBe(width === 1440 ? wideColumns : 1);
    }
  });
}

test("配置主位保留控件与状态，详情弹窗且浏览不写硬件", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.evaluate(() => {
    window.__uiFixture.dust = { revision: "dust-0", running: false, current: null, initial: null, softwareActive: false,
      lastStart: null, startBlockedReason: null, scheduleBlockedReason: null };
  });
  await openWorkspace(page, "性能与散热");
  const modes = page.getByRole("region", { name: "预设模式", exact: true });
  await expect(modes.getByRole("radio")).toHaveCount(4);
  await expect(modes.getByText("续航优先", { exact: true })).toHaveCount(0);
  const artwork = modes.locator("svg.ccc-preset-artwork");
  await expect(artwork).toHaveCount(1);
  await expect(artwork).toHaveAttribute("data-preset-palette", "performance");
  await expect(artwork).toHaveAttribute("aria-hidden", "true");
  await expect(artwork).toHaveAttribute("focusable", "false");
  await expect(artwork).toHaveAttribute("viewBox", "0 0 480 204");
  await expect(artwork).toHaveCSS("pointer-events", "none");
  await expect(modes.locator(".ccc-preset-current")).toHaveText("性能模式");
  const fan = page.getByRole("region", { name: "散热控制", exact: true });
  await expect(fan.getByRole("region", { name: "整机策略", exact: true })).toHaveAttribute("data-surface", "inset");
  await expect(fan.getByRole("region", { name: "风扇实况与曲线", exact: true })).toHaveAttribute("data-surface", "inset");
  const cpu = page.getByRole("region", { name: "CPU 功耗与电压", exact: true });
  const gpu = page.getByRole("region", { name: "GPU 频率", exact: true });
  await expect(cpu.getByRole("slider", { name: "持续功耗上限", exact: true })).toBeVisible();
  const cpuBox = await cpu.boundingBox(), gpuBox = await gpu.boundingBox();
  expect(cpuBox.x).toBeCloseTo(gpuBox.x, 0);
  expect(cpuBox.width).toBeCloseTo(gpuBox.width, 0);
  expect(gpuBox.y).toBeGreaterThan(cpuBox.y + cpuBox.height);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  for (const [panel, name] of [[cpu, "Isolated CPU"], [gpu, "Isolated GPU"]]) {
    const heading = panel.getByRole("heading", { level: 2 });
    const hardware = panel.getByText(name, { exact: true });
    await expect(hardware).toBeVisible();
    const [titleBox, hardwareBox] = await Promise.all([heading.boundingBox(), hardware.boundingBox()]);
    const titleTextLeft = await heading.evaluate(element => {
      const text = document.createRange();
      text.selectNode(element.lastChild);
      return text.getBoundingClientRect().left;
    });
    expect(hardwareBox.y).toBeGreaterThanOrEqual(titleBox.y + titleBox.height);
    expect(hardwareBox.x).toBeCloseTo(titleTextLeft, 1);
  }
  const dust = page.getByRole("region", { name: "风扇除尘", exact: true });
  await expect(dust.getByRole("img", { name: "除尘风扇", exact: true })).toBeVisible();
  const dialogs = [
    [modes, "模式说明", "预设模式与风扇、电源"],
    [cpu, "CPU 参数与诊断", "持续功耗上限 · W"],
    [gpu, "GPU 参数与诊断", "核心频率偏移 · MHz"],
    [fan, "风扇说明与诊断", "参数说明", "风扇强度与目标转速 · %", "隔离风扇诊断"],
    [dust, "除尘说明", "除尘由电脑自行完成"],
  ];
  for (const width of [1440, 360]) {
    await page.setViewportSize({ width, height: 1000 });
    for (const [panel, title, ...facts] of dialogs) {
      const info = await expectHeaderInfo(panel, title);
      if (width === 1440) {
        await info.hover();
        await expect(page.getByRole("tooltip", { name: title, exact: true })).toBeVisible();
        await info.click();
      } else await info.press("Enter");
      const dialog = page.getByRole("dialog", { name: title, exact: true });
      await expect(dialog).toBeVisible();
      for (const fact of facts) await expect(dialog).toContainText(fact);
      await page.keyboard.press("Escape");
      await expect(info).toBeFocused();
    }
  }
  await openWorkspace(page, "设备配置");
  const keyboard = page.getByRole("region", { name: "键盘功能", exact: true });
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(keyboard.getByRole("switch", { name: "Fn 锁", exact: true })).toBeVisible();
  const writes = await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|set_|enable_|restore_|start_fan_)/.test(call.command) && call.command !== "set_shortcut_recording"));
  expect(writes).toEqual([]);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

async function expectHeaderInfo(panel, title) {
  const header = panel.locator(".ccc-panel-head");
  const heading = header.getByRole("heading", { level: 2 });
  const info = header.getByRole("button", { name: title, exact: true });
  await expect(panel.locator('button:has(> svg[data-icon="info"])')).toHaveCount(1);
  await expect(info.locator('svg[data-icon="info"]')).toBeVisible();
  await expect(info).toHaveText("");
  await expect(info).toHaveAttribute("data-size", "icon-compact");
  await expect(info).toHaveAttribute("aria-haspopup", "dialog");
  const [headerBox, titleBox, infoBox] = await Promise.all([header.boundingBox(), heading.boundingBox(), info.boundingBox()]);
  expect(infoBox.x + infoBox.width).toBeCloseTo(headerBox.x + headerBox.width, 1);
  expect(infoBox.y + infoBox.height / 2).toBeCloseTo(titleBox.y + titleBox.height / 2, 1);
  return info;
}

test("调校分组随窗口回流，GPU 草稿经预览提交并可恢复", async ({ page }) => {
  await page.evaluate(() => {
    const ipc = window.__TAURI_INTERNALS__;
    const invoke = ipc.invoke;
    const { gpu, calls } = window.__uiFixture;
    ipc.invoke = async (command, args) => {
      if (command !== "apply_gpu_clock_offsets" && command !== "restore_gpu_baseline") return invoke(command, args);
      calls.push({ command, args });
      if (command === "apply_gpu_clock_offsets") {
        gpu.baseline ??= { coreOffsetMhz: gpu.coreOffsetMhz, memOffsetMhz: gpu.memOffsetMhz };
        gpu.coreOffsetMhz = args.coreMhz ?? gpu.coreOffsetMhz;
        gpu.memOffsetMhz = args.memMhz ?? gpu.memOffsetMhz;
      } else {
        Object.assign(gpu, gpu.baseline);
      }
      return { ok: true, coreOffsetMhz: gpu.coreOffsetMhz, memOffsetMhz: gpu.memOffsetMhz, rollbackError: null };
    };
  });
  await openWorkspace(page, "性能与散热");
  const cpu = page.getByRole("region", { name: "CPU 功耗与电压", exact: true });
  const gpu = page.getByRole("region", { name: "GPU 频率", exact: true });
  const core = gpu.getByRole("slider", { name: "核心频率偏移", exact: true });
  await expect(core).toBeEnabled();
  await expect(cpu.getByRole("region", { name: "功耗上限", exact: true }).getByRole("slider")).toHaveCount(2);
  await expect(cpu.getByRole("region", { name: "电压偏移", exact: true }).getByRole("slider")).toHaveCount(2);
  for (const [width, theme] of [[1440, "light"], [360, "dark"]]) {
    await page.setViewportSize({ width, height: 1000 });
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    const sliderSize = await page.evaluate(() => {
      const tokens = getComputedStyle(document.documentElement);
      return {
        track: tokens.getPropertyValue("--ccc-range-track").trim(),
        thumb: tokens.getPropertyValue("--ccc-range-thumb").trim(),
        height: tokens.getPropertyValue("--ccc-control-height-compact").trim(),
      };
    });
    for (const panel of [cpu, gpu]) {
      const items = await panel.locator(".ccc-tuning-columns > *").evaluateAll(nodes => nodes.map(node => {
        const box = node.getBoundingClientRect();
        return { top: box.top, left: box.left, right: box.right };
      }));
      expect(items).toHaveLength(2);
      expect(items.every(item => item.left >= 0 && item.right <= width)).toBe(true);
      expect(new Set(items.map(item => item.top)).size).toBe(width === 1440 ? 1 : 2);
      await expect(panel.locator(".ccc-range-track").first()).toHaveCSS("height", sliderSize.track);
      await expect(panel.locator(".ccc-range-thumb").first()).toHaveCSS("width", sliderSize.thumb);
    }
    await expect(core).toHaveCSS("height", sliderSize.height);
    await cpu.locator("..").screenshot({ path: `target/ui-check/processor-tuning-${theme}-${width}.png` });
  }
  await core.press("ArrowRight");
  await expect(core).toHaveAttribute("aria-valuetext", "1 MHz");
  await core.fill("45");
  const writes = () => page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_gpu_clock_offsets|restore_gpu_baseline)$/.test(call.command)));
  const preview = await previewConfiguration(page, "GPU 频率");
  await expect(preview.getByText("45 MHz", { exact: true })).toBeVisible();
  expect(await writes()).toEqual([]);
  await submitConfiguration(preview);
  await expect(core).toHaveAttribute("aria-valuetext", "45 MHz");
  expect(await writes()).toEqual([{ command: "apply_gpu_clock_offsets", args: { coreMhz: 45, memMhz: null } }]);
  await submitConfiguration(await previewConfiguration(page, "GPU 频率", true), "确认恢复");
  await expect(core).toHaveAttribute("aria-valuetext", "0 MHz");
  expect((await writes()).map(({ command }) => command)).toEqual(["apply_gpu_clock_offsets", "restore_gpu_baseline"]);
});

test("GPU 调校保留只读偏移，确认后请求管理员重启并可在取消后重试", async ({ page }) => {
  await page.evaluate(() => Object.assign(window.__uiFixture.gpu, {
    coreOffsetMhz: 35, baseline: { coreOffsetMhz: 0, memOffsetMhz: 0 },
    writeBlockedReason: "调整或恢复 GPU 偏移需要管理员权限。",
  }));
  await openWorkspace(page, "性能与散热");
  const gpu = page.getByRole("region", { name: "GPU 频率", exact: true });
  const core = gpu.getByRole("slider", { name: "核心频率偏移" });
  await expect(core).toBeDisabled();
  await expect(core).toHaveAttribute("aria-valuetext", "35 MHz");
  await expect(gpu.getByRole("slider", { name: "显存频率偏移" })).toBeDisabled();
  await expect(gpu).toContainText("需要管理员权限");
  await expect(gpu.getByRole("button", { name: /应用更改|恢复GPU 频率/ })).toHaveCount(0);
  await page.evaluate(() => {
    const ipc = window.__TAURI_INTERNALS__;
    const invoke = ipc.invoke;
    ipc.invoke = async (command, args) => {
      const result = await invoke(command, args);
      if (command === "restart_as_administrator") {
        await new Promise((_, reject) => { window.__cancelElevation = reject; });
      }
      return result;
    };
  });
  const restarts = () => page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "restart_as_administrator"));
  const button = gpu.getByRole("button", { name: "以管理员身份重启", exact: true });
  const dialog = page.getByRole("dialog", { name: "以管理员运行", exact: true });
  await button.click();
  await expect(dialog).toContainText("当前 Windows 账户");
  await dialog.getByRole("button", { name: "取消", exact: true }).click();
  expect(await restarts()).toHaveLength(0);
  await button.click();
  await dialog.getByRole("button", { name: "请求 Windows 授权", exact: true }).click();
  await expect(button).toBeDisabled();
  expect(await restarts()).toHaveLength(1);
  await page.evaluate(() => window.__cancelElevation("已取消 Windows 管理员授权"));
  await expect(gpu.getByRole("alert")).toHaveText("已取消 Windows 管理员授权");
  await expect(button).toBeEnabled();
  await expect(core).toBeDisabled();
  await expect(core).toHaveAttribute("aria-valuetext", "35 MHz");
  await button.click();
  await dialog.getByRole("button", { name: "请求 Windows 授权", exact: true }).click();
  await expect.poll(async () => (await restarts()).length).toBe(2);
  await expect(button).toBeDisabled();
});

test("GPU 调校对无法提权的标准账户显示原因并保留只读偏移", async ({ page }) => {
  await page.evaluate(() => sessionStorage.setItem("standard-account", "true"));
  await page.reload();
  await page.evaluate(() => Object.assign(window.__uiFixture.gpu, {
    coreOffsetMhz: 35,
    writeBlockedReason: "当前 Windows 账户没有可用的管理员权限，请使用管理员账户登录 Windows 后打开应用。",
  }));
  await openWorkspace(page, "性能与散热");
  const gpu = page.getByRole("region", { name: "GPU 频率", exact: true });
  const core = gpu.getByRole("slider", { name: "核心频率偏移" });
  await expect(core).toBeDisabled();
  await expect(core).toHaveAttribute("aria-valuetext", "35 MHz");
  await expect(gpu).toContainText("当前 Windows 账户没有可用的管理员权限");
  await expect(gpu.getByRole("button", { name: "以管理员身份重启" })).toHaveCount(0);
});

for (const readFails of [false, true]) {
  test(`GPU 回滚失败${readFails ? "且首次回读失败" : ""}后更新实际读数并保留草稿`, async ({ page }) => {
    await page.evaluate(failRead => {
      const ipc = window.__TAURI_INTERNALS__;
      const invoke = ipc.invoke;
      let rejectNextRead = false;
      ipc.invoke = async (command, args) => {
        if (command === "read_gpu_profile" && rejectNextRead) {
          rejectNextRead = false;
          throw new Error("隔离回读失败");
        }
        if (command !== "apply_gpu_clock_offsets") return invoke(command, args);
        Object.assign(window.__uiFixture.gpu, { coreOffsetMhz: 25, memOffsetMhz: 80 });
        rejectNextRead = failRead;
        return { ok: false, message: "写入 GPU 频率偏移失败: NVAPI_INVALID_USER_PRIVILEGE (-137)",
          rollbackError: "回滚失败: NVAPI_ERROR (-1)", coreOffsetMhz: null, memOffsetMhz: null };
      };
    }, readFails);
    await openWorkspace(page, "性能与散热");
    const gpu = page.getByRole("region", { name: "GPU 频率", exact: true });
    const core = gpu.getByRole("slider", { name: "核心频率偏移" });
    await core.fill("45");
    const preview = await previewConfiguration(page, "GPU 频率");
    const apply = preview.locator("[data-configuration-apply]");
    const writeError = gpu.getByRole("alert").filter({ hasText: "写入 GPU 频率偏移失败" });
    await apply.click();
    await expect(writeError).toContainText("NVAPI_INVALID_USER_PRIVILEGE (-137)");
    await expect(writeError).toContainText("回滚失败: NVAPI_ERROR (-1)");
    if (readFails) {
      await expect(gpu.getByRole("alert").filter({ hasText: "无法读取当前偏移 · 隔离回读失败" })).toBeVisible();
      await expect(core).toBeVisible();
      await expect(core).toBeDisabled();
      await expect(apply).toBeDisabled();
      await gpu.getByRole("button", { name: "重试", exact: true }).click();
    }
    await expect(core).toBeEnabled();
    await expect(preview).toContainText("25 MHz → 45 MHz");
    await expect(core).toHaveAttribute("aria-valuetext", "45 MHz");
    await expect(gpu.getByRole("slider", { name: "显存频率偏移" })).toHaveAttribute("aria-valuetext", "80 MHz");
    await expect(writeError).toContainText("回滚失败");
  });
}

test("性能调校可用键盘切换预设，概览只展示回读后的模式", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  await openWorkspace(page, "性能与散热");
  const modes = page.getByRole("region", { name: "预设模式", exact: true });
  const entertainment = modes.getByRole("radio", { name: /^娱乐/ });
  const writes = () => page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "apply_oem_power_mode"));
  expect(await writes()).toHaveLength(0);
  await modes.getByRole("radio", { name: /^性能/ }).focus();
  await page.keyboard.press("ArrowLeft");
  await expect(entertainment).toBeFocused();
  await expect(entertainment).toBeChecked();
  await expect(modes.getByText("当前使用", { exact: true })).toHaveCount(0);
  await expect(modes.locator(".ccc-preset-artwork")).toHaveAttribute("data-preset-palette", "entertainment");
  await expect(entertainment.locator("..")).toHaveCSS("outline-style", "solid");
  await expect(page.getByRole("region", { name: "待应用更改", exact: true })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "应用更改", exact: true })).toHaveCount(0);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await modes.screenshot({ path: "target/ui-check/preset-direct-light-1440.png" });
  await page.evaluate(async () => {
    window.__uiEmit("hardware-snapshot", window.__uiHardwareSnapshot());
  });
  await openWorkspace(page, "概览");
  const currentMode = page.getByRole("region", { name: "当前模式", exact: true });
  await expect(currentMode.getByRole("status")).toHaveText("娱乐");
  await expect(currentMode.locator("svg.ccc-preset-artwork")).toHaveAttribute("data-preset-palette", "entertainment");
  await expect(modes).toHaveCount(0);
  await expect(page.getByRole("region", { name: "页面操作", exact: true })).toHaveCount(0);
  expect(await writes()).toHaveLength(1);
});

for (const theme of ["light", "dark"]) {
  test(`配置卡片 ${theme} 宽窄屏`, async ({ page }) => {
    await openWorkspace(page, "设置");
    const themeOption = page.getByRole("radio", { name: theme === "light" ? "浅色" : "深色", exact: true });
    await themeOption.click();
    await expect(themeOption).toBeChecked();
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
    for (const width of [1440, 360]) {
      await page.setViewportSize({ width, height: 1000 });
      for (const [name, id] of [["概览", "overview"], ["性能与散热", "performance"], ["设备配置", "graphics"], ["电源与档案", "power"], ["键盘与灯光", "keyboard"]]) {
        await openWorkspace(page, name);
        const main = page.getByRole("main", { name, exact: true });
        await expect(main.getByRole("heading").first()).toBeVisible();
        const bounds = await main.boundingBox();
        expect(bounds.x + bounds.width).toBeLessThanOrEqual(width + 1);
        const overflow = await main.evaluate(element => Array.from(element.querySelectorAll("section")).filter(section => {
          const box = section.getBoundingClientRect();
          return box.width > 0 && box.right > innerWidth + 1;
        }).map(section => section.getAttribute("aria-label")));
        expect(overflow).toEqual([]);
        if (id === "performance") {
          const modes = main.getByRole("region", { name: "预设模式", exact: true });
          await expectPresetLayout(modes);
          await modes.screenshot({ path: `target/ui-check/preset-modes-${theme}-${width}.png` });
        }
        await page.screenshot({ path: `target/ui-check/configuration-${id}-${theme}-${width}.png` });
      }
    }
    expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
  });
}

async function expectPresetLayout(panel) {
  const layout = await panel.evaluate(region => {
    const block = selector => region.querySelector(selector).getBoundingClientRect();
    const center = ({ left, width }) => left + width / 2;
    const stage = region.getBoundingClientRect();
    const artwork = block(":scope > .ccc-preset-artwork");
    const caption = block(".ccc-preset-current");
    const controls = block(".ccc-preset-switcher");
    const inputs = [...region.querySelectorAll(".ccc-segmented-option")];
    return {
      columns: new Set(inputs.map(input => input.getBoundingClientRect().left)).size,
      centered: [artwork, caption, controls].every(box => Math.abs(center(box) - center(stage)) < 1),
      ordered: artwork.bottom <= caption.top + 1 && caption.bottom < controls.top,
      fits: inputs.every(input => input.scrollWidth <= input.clientWidth + 1) && controls.width <= stage.width + 1,
    };
  });
  expect(layout).toEqual({ columns: 4, centered: true, ordered: true, fits: true });
}
