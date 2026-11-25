import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const cpu = page => page.getByRole("region", { name: "CPU 运行读数", exact: true });
const usage = page => cpu(page).getByRole("meter", { name: "CPU 利用率", exact: true });
const missing = page => cpu(page).getByRole("img", { name: "CPU 利用率：暂无读数", exact: true });
async function publish(page, valuePct, age = 0, error = null) {
  await page.evaluate(async ({ valuePct, age, error }) => {
    const data = window.__uiFixture;
    const snapshot = window.__uiHardwareSnapshot();
    snapshot.sampledAtMs = data.lastCpuSample = Math.max(Date.now(), data.lastCpuSample ?? 0) + 1000;
    snapshot.cpuUtility = valuePct == null ? null : { valuePct, clockMhz: null, sampledAtMs: snapshot.sampledAtMs - age };
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
test("CPU 指标显示语义图标和实读数值，查看不会写入", async ({ page }) => {
  await expect(cpu(page).getByRole("meter", { name: "风扇占空比", exact: true })).toHaveAttribute("aria-valuenow", "30");
  await expect(usage(page)).toHaveAttribute("aria-valuemax", "100");
  await expect(cpu(page).getByLabel("当前频率", { exact: true })).toHaveText("2900 MHz");
  await expect(cpu(page).locator(".ccc-meter-heading > svg")).toHaveCount(3);
  const writes = await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|restore_|set_)/.test(call.command) && call.command !== "set_shortcut_recording"));
  expect(writes).toEqual([]);
});
test("首次采样和读取失败显示未知，真实零值保留为零并可恢复", async ({ page }) => {
  await publish(page, null); await expect(missing(page)).toBeVisible();
  await expect(missing(page).locator("output")).toHaveText("—");
  await expect(missing(page).locator('svg[data-icon="performance"]')).toBeVisible();
  await publish(page, 0); await expect(usage(page)).toHaveAttribute("aria-valuenow", "0");
  await publish(page, null, 0, "CPU效用读取失败：隔离计数器不可用"); await expect(missing(page)).toBeVisible();
  await expect(page.getByRole("button", { name: "监控诊断", exact: true })).toHaveCount(0);
  await publish(page, 76); await expect(usage(page)).toHaveAttribute("aria-valuenow", "76");
});
test("过期和未来时间戳不显示为当前利用率", async ({ page }) => {
  await publish(page, 90, 2501); await expect(missing(page)).toBeVisible();
  await publish(page, 90, -1); await expect(missing(page)).toBeVisible();
  await publish(page, 90, 2500); await expect(usage(page)).toHaveAttribute("aria-valuenow", "90");
});
test("深浅宽窄布局保留卡片间距，CPU 频率和比例读数不溢出", async ({ page }) => {
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 900 }); await cpu(page).scrollIntoViewIfNeeded();
      const layout = await cpu(page).evaluate(el => {
        const bounds = el.getBoundingClientRect();
        const metrics = el.querySelector(".ccc-sensor-metrics"), grid = metrics.getBoundingClientRect();
        return { overflow: el.scrollWidth > el.clientWidth + 1, right: bounds.right,
          border: parseFloat(getComputedStyle(el).borderTopWidth), gap: parseFloat(getComputedStyle(el.parentElement).gap),
          iconsFit: [...metrics.querySelectorAll(".ccc-meter-heading > svg")].every(icon => {
            const r = icon.getBoundingClientRect(); return r.left >= grid.left - 1 && r.right <= grid.right + 1;
          }) };
      });
      expect(layout.overflow).toBe(false); expect(layout.right).toBeLessThanOrEqual(width);
      expect(layout.border).toBeGreaterThan(0); expect(layout.gap).toBeGreaterThan(0);
      expect(layout.iconsFit).toBe(true);
      if (width !== 760) await cpu(page).screenshot({ path: `target/ui-check/cpu-utility-${theme}-${width}.png` });
    }
  }
});
