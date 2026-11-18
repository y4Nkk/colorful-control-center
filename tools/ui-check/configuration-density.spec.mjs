import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => { window.__uiFixture.calibrationSupported = true; });
  await page.goto("/");
});

test("配置主位保留控件与状态，详情弹窗且浏览不写硬件", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  await openWorkspace(page, "性能调校");
  const modes = page.getByRole("region", { name: "预设模式", exact: true });
  await expect(modes.getByRole("radio")).toHaveCount(4);
  await expect(modes.getByText("续航优先", { exact: true })).toHaveCount(0);
  await expect(modes.getByText("选择模式后确认应用，插电偏好会同步保存。")).toHaveCount(0);
  expect((await modes.boundingBox()).height).toBeLessThan(280);
  const cpu = page.getByRole("region", { name: "功耗与电压", exact: true });
  const gpu = page.getByRole("region", { name: "GPU 频率", exact: true });
  await expect(cpu.getByRole("slider", { name: "持续功耗上限", exact: true })).toBeVisible();
  const left = await cpu.boundingBox(), right = await gpu.boundingBox();
  expect(Math.abs(left.y - right.y)).toBeLessThan(2);
  expect(right.x).toBeGreaterThan(left.x + left.width);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await cpu.getByRole("button", { name: "详情", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "CPU 参数与诊断", exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await openWorkspace(page, "键盘与输入");
  const keyboard = page.getByRole("region", { name: "键盘功能", exact: true });
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(keyboard.getByRole("switch", { name: "Fn 锁", exact: true })).toBeVisible();
  const writes = await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|set_|enable_|restore_)/.test(call.command) && call.command !== "set_shortcut_recording"));
  expect(writes).toEqual([]);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

for (const theme of ["light", "dark"]) {
  test(`配置卡片 ${theme} 宽窄屏`, async ({ page }) => {
    await openWorkspace(page, "设置");
    await page.getByRole("radio", { name: theme === "light" ? "浅色" : "深色", exact: true }).check();
    for (const width of [1440, 360]) {
      await page.setViewportSize({ width, height: 1000 });
      for (const [name, id] of [["性能调校", "performance"], ["键盘与输入", "input"], ["电池与节能", "battery"], ["显示与显卡", "graphics"]]) {
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
        await page.screenshot({ path: `target/ui-check/configuration-${id}-${theme}-${width}.png` });
      }
    }
    expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
  });
}
