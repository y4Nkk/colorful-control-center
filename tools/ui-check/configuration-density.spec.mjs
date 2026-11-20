import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => { window.__uiFixture.calibrationSupported = true; });
  await page.goto("/");
});

for (const { name, leftNeighbor, rightNeighbor, switchCount, systemArea } of [
  { name: "键盘功能", leftNeighbor: "显卡模式", rightNeighbor: "飞行模式", switchCount: 3 },
  { name: "性能选项", leftNeighbor: "CPU 功耗与电压", rightNeighbor: "GPU 频率", switchCount: 2, systemArea: "性能与散热" },
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
      expect(columns).toBe(width === 1440 ? switchCount : 1);
    }
  });
}

test("配置主位保留控件与状态，详情弹窗且浏览不写硬件", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  await openWorkspace(page, "性能与散热");
  const modes = page.getByRole("region", { name: "预设模式", exact: true });
  await expect(modes.getByRole("radio")).toHaveCount(4);
  await expect(modes.getByText("续航优先", { exact: true })).toHaveCount(0);
  const artwork = modes.locator("svg.ccc-preset-chip");
  await expect(artwork).toHaveCount(1);
  await expect(artwork).toHaveAttribute("data-preset-palette", "performance");
  await expect(artwork).toHaveAttribute("aria-hidden", "true");
  await expect(artwork).toHaveAttribute("focusable", "false");
  await expect(artwork).toHaveAttribute("viewBox", "40 0 520 250");
  await expect(artwork).toHaveCSS("pointer-events", "none");
  await expect(modes.locator(".ccc-preset-current")).toHaveText("性能模式");
  const fan = page.getByRole("region", { name: "风扇设置", exact: true });
  const fanInfo = fan.locator('button:has(> svg[data-icon="info"])');
  await expect(fanInfo).toHaveCount(1);
  await fanInfo.press("Enter");
  const fanDetails = page.getByRole("dialog", { name: "风扇说明与诊断", exact: true });
  await expect(fanDetails.getByRole("heading", { name: "参数说明", exact: true })).toBeVisible();
  await expect(fanDetails).toContainText("隔离风扇诊断");
  await page.keyboard.press("Escape");
  await expect(fanInfo).toBeFocused();
  const cpu = page.getByRole("region", { name: "CPU 功耗与电压", exact: true });
  const gpu = page.getByRole("region", { name: "GPU 频率", exact: true });
  await expect(cpu.getByRole("slider", { name: "持续功耗上限", exact: true })).toBeVisible();
  const left = await cpu.boundingBox(), right = await gpu.boundingBox();
  expect(Math.abs(left.y - right.y)).toBeLessThan(2);
  expect(right.x).toBeGreaterThan(left.x + left.width);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  const info = cpu.getByRole("button", { name: "详情", exact: true });
  await expect(info).toBeEmpty();
  await expect(info.locator('[data-icon="info"]')).toHaveCount(1);
  await expect(info).toHaveAttribute("title", "CPU 参数与诊断");
  await info.press("Enter");
  await expect(page.getByRole("dialog", { name: "CPU 参数与诊断", exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(info).toBeFocused();
  await openWorkspace(page, "设备配置");
  const keyboard = page.getByRole("region", { name: "键盘功能", exact: true });
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(keyboard.getByRole("switch", { name: "Fn 锁", exact: true })).toBeVisible();
  const writes = await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|set_|enable_|restore_)/.test(call.command) && call.command !== "set_shortcut_recording"));
  expect(writes).toEqual([]);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

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
  await expect(modes.locator(".ccc-preset-chip")).toHaveAttribute("data-preset-palette", "entertainment");
  await expect(entertainment.locator("..")).not.toHaveCSS("box-shadow", "none");
  await expect(page.getByRole("region", { name: "待应用更改", exact: true })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "应用更改", exact: true })).toHaveCount(0);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await modes.screenshot({ path: "target/ui-check/preset-direct-light-1440.png" });
  await page.evaluate(async () => {
    window.__uiEmit("hardware-snapshot", await window.__TAURI_INTERNALS__.invoke("read_hardware_snapshot"));
  });
  await openWorkspace(page, "概览");
  const currentMode = page.getByRole("region", { name: "当前模式", exact: true });
  await expect(currentMode.getByRole("status")).toHaveText("娱乐");
  await expect(currentMode.locator("svg.ccc-preset-chip")).toHaveAttribute("data-preset-palette", "entertainment");
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
    const artwork = block(":scope > .ccc-preset-chip");
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
