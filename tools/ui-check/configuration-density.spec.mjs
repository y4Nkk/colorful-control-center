import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace, openSystemArea } from "./workspace.mjs";

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => { window.__uiFixture.calibrationSupported = true; });
  await page.goto("/");
});

for (const { name, leftNeighbor, rightNeighbor, switchCount, systemArea } of [
  { name: "键盘功能", leftNeighbor: "显卡模式", rightNeighbor: "飞行模式", switchCount: 3 },
  { name: "性能选项", leftNeighbor: "CPU 功耗与电压", rightNeighbor: "GPU 频率", switchCount: 2, systemArea: "性能调校" },
]) {
  test(`${name}在宽窄窗口下与相邻模块对齐`, async ({ page }) => {
    if (systemArea) await openSystemArea(page, systemArea);
    else await openWorkspace(page, "设备配置");
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
  await openSystemArea(page, "性能调校");
  const modes = page.getByRole("region", { name: "预设模式", exact: true });
  await expect(modes.getByRole("radio")).toHaveCount(4);
  await expect(modes.getByText("续航优先", { exact: true })).toHaveCount(0);
  const artwork = modes.locator("svg[data-mode-art]");
  await expect(artwork).toHaveCount(4);
  for (const [index, drawing] of ["eco", "quiet", "entertainment", "performance"].entries()) {
    const svg = artwork.nth(index);
    await expect(svg).toHaveAttribute("data-mode-art", drawing);
    await expect(svg).toHaveAttribute("aria-hidden", "true");
    await expect(svg).toHaveAttribute("focusable", "false");
    await expect(svg).toHaveAttribute("viewBox", "0 0 48 48");
    await expect(svg).toHaveCSS("width", "56px");
    await expect(svg).toHaveCSS("height", "56px");
    await expect(svg).toHaveCSS("pointer-events", "none");
    await expect(svg.locator("path").first()).toBeVisible();
  }
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
  await openSystemArea(page, "性能调校");
  const modes = page.getByRole("region", { name: "预设模式", exact: true });
  const entertainment = modes.getByRole("radio", { name: /^娱乐/ });
  const writes = () => page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "apply_oem_power_mode"));
  expect(await writes()).toHaveLength(0);
  await modes.getByRole("radio", { name: /^性能/ }).focus();
  await page.keyboard.press("ArrowLeft");
  await expect(entertainment).toBeFocused();
  await expect(entertainment).toBeChecked();
  await expect(modes.getByText("当前使用", { exact: true })).toHaveCount(0);
  await expect(entertainment.locator("..").locator(".ccc-mode-select-mark")).toBeVisible();
  await expect(entertainment.locator("..")).not.toHaveCSS("box-shadow", "none");
  await expect(page.getByRole("region", { name: "待应用更改", exact: true })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "应用更改", exact: true })).toHaveCount(0);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await modes.screenshot({ path: "target/ui-check/preset-direct-light-1440.png" });
  await page.evaluate(async () => {
    window.__uiEmit("hardware-snapshot", await window.__TAURI_INTERNALS__.invoke("read_hardware_snapshot"));
  });
  await openSystemArea(page, "概览");
  const currentMode = page.getByRole("region", { name: "当前模式", exact: true });
  await expect(currentMode.getByRole("status")).toHaveText("娱乐");
  await expect(currentMode.locator("svg[data-mode-art]")).toHaveAttribute("data-mode-art", "entertainment");
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
      for (const [name, id, area] of [["系统", "overview", "概览"], ["系统", "performance", "性能调校"], ["设备配置", "graphics"], ["电源", "power"], ["键盘与灯光", "keyboard"]]) {
        if (area) await openSystemArea(page, area);
        else await openWorkspace(page, name);
        const main = page.getByRole("main", { name, exact: true });
        await expect(main.getByRole("heading").first()).toBeVisible();
        const bounds = await main.boundingBox();
        expect(bounds.x + bounds.width).toBeLessThanOrEqual(width + 1);
        const overflow = await main.evaluate(element => Array.from(element.querySelectorAll("section")).filter(section => {
          const box = section.getBoundingClientRect();
          return box.width > 0 && box.right > innerWidth + 1;
        }).map(section => section.getAttribute("aria-label")));
        expect(overflow).toEqual([]);
        if (area === "性能调校") {
          const modes = main.getByRole("region", { name: "预设模式", exact: true });
          await expectPresetLayout(modes, width === 360);
          await modes.screenshot({ path: `target/ui-check/preset-modes-${theme}-${width}.png` });
        }
        await page.screenshot({ path: `target/ui-check/configuration-${id}-${theme}-${width}.png` });
      }
    }
    expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
  });
}

async function expectPresetLayout(panel, narrow) {
  const layout = await panel.locator(".ccc-mode-option").evaluateAll((cards, stacked) => {
    const frames = cards.map(card => card.getBoundingClientRect());
    return {
      columns: new Set(frames.map(frame => frame.left)).size,
      fits: cards.every((card, index) => {
        const outer = frames[index];
        const caption = card.querySelector(".ccc-mode-copy");
        const title = caption.getBoundingClientRect();
        const artwork = card.querySelector("[data-mode-art]").getBoundingClientRect();
        const size = stacked ? 44 : 56;
        return Math.abs(artwork.width - size) < 1 && Math.abs(artwork.height - size) < 1
          && artwork.left >= outer.left && artwork.top >= outer.top
          && title.right <= outer.right && title.bottom <= outer.bottom
          && caption.scrollWidth <= caption.clientWidth + 1
          && (stacked ? artwork.bottom <= title.top : artwork.right <= title.left);
      }),
    };
  }, narrow);
  expect(layout).toEqual({ columns: narrow ? 2 : 4, fits: true });
}
