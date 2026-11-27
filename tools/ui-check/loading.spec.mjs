import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openPowerArea, openWorkspace } from "./workspace.mjs";

// 挂起指定命令的所有调用直到测试调用 window.__release()；开发模式 StrictMode 会让首读执行两次。
function hold(...commands) {
  return `
    const held = new Set(${JSON.stringify(commands)}), waiters = [], invoke = window.__TAURI_INTERNALS__.invoke;
    window.__release = () => { held.clear(); for (const resolve of waiters.splice(0)) resolve(); };
    window.__TAURI_INTERNALS__.invoke = async (name, args) => {
      if (held.has(name)) await new Promise(resolve => { waiters.push(resolve); });
      return invoke(name, args);
    };`;
}

test("卡片首读显示旋转图标和说明，系统或应用减少动效时图标静止", async ({ page }) => {
  await page.addInitScript({ content: `${MOCK_IPC}\nwindow.__uiFixture.preferences.motion = "system";${hold("read_graphics_mode")}` });
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.goto("/");
  await openWorkspace(page, "设备配置");
  const panel = page.getByRole("region", { name: "显卡模式", exact: true });
  await expect(panel).toBeVisible({ timeout: 30_000 });
  const loading = panel.getByRole("status");
  await expect(loading).toHaveText("正在读取显卡模式…");
  const icon = loading.locator('[data-icon="loading"]');
  await expect(icon).toHaveCount(1);
  await expect(icon).toHaveCSS("animation-name", "ccc-icon-spin");
  await expect(icon).toHaveCSS("animation-duration", "0.8s");
  await panel.screenshot({ path: "target/ui-check/loading-light.png" });
  await page.evaluate(() => { document.documentElement.dataset.theme = "dark"; });
  await panel.screenshot({ path: "target/ui-check/loading-dark.png" });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await expect(icon).toHaveCSS("animation-name", "none");
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.evaluate(() => { document.documentElement.dataset.motion = "reduce"; });
  await expect(icon).toHaveCSS("animation-name", "none");
  await page.evaluate(() => { document.documentElement.dataset.motion = "system"; window.__release(); });
  await expect(panel.getByRole("status")).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "混合显卡", exact: true })).toBeVisible();
});

test("按钮提交中锁定入口、标记忙碌并显示进度图标", async ({ page }) => {
  await page.addInitScript({ content: `${MOCK_IPC}
    window.__uiFixture.dust = { revision: "dust-0", running: false, current: null, initial: null, softwareActive: false,
      lastStart: null, startBlockedReason: null, scheduleBlockedReason: null };
    ${hold("start_fan_dust")}` });
  await page.goto("/");
  await openWorkspace(page, "性能与散热");
  const panel = page.getByRole("region", { name: "风扇除尘", exact: true });
  const trigger = panel.getByRole("button", { name: "立即除尘", exact: true });
  await expect(trigger).toBeEnabled();
  await expect(trigger).toHaveCSS("cursor", "pointer");
  await trigger.click();
  await page.getByRole("dialog").getByRole("button", { name: "确认开始一次除尘？", exact: true }).click();
  await expect(trigger).toBeDisabled();
  await expect(trigger).toHaveAttribute("aria-busy", "true");
  await expect(trigger.locator('[data-icon="loading"]')).toHaveCount(1);
  await expect(trigger).toHaveCSS("cursor", "progress");
  await page.evaluate(() => window.__release());
  await expect(trigger).not.toHaveAttribute("aria-busy", "true");
});

test("电池与电源计划首读使用统一等待组件，失败结束等待并可重试", async ({ page }) => {
  await page.addInitScript({ content: `${MOCK_IPC}\n${hold("read_battery_profile", "read_windows_power_plans")}` });
  await page.goto("/");
  await openPowerArea(page, "电源设置");
  const battery = page.getByRole("region", { name: "电池与充电", exact: true });
  const plans = page.getByRole("region", { name: "Windows 电源计划", exact: true });
  await expect(battery.getByRole("status")).toHaveText("正在读取电池状态…");
  await expect(battery.getByText("暂无电量读数")).toHaveCount(0);
  await expect(plans.getByRole("status")).toHaveText("正在读取电源计划…");
  await page.evaluate(() => {
    window.__uiFixture.batteryReadFault = "隔离电池读取失败";
    window.__release();
  });
  await expect(battery.getByRole("alert")).toContainText("隔离电池读取失败");
  await expect(battery.getByRole("status")).toHaveCount(0);
  await expect(plans.getByRole("combobox", { name: "选择计划", exact: true })).toBeVisible();
  await expect(plans.getByRole("status")).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.batteryReadFault = null; });
  await battery.getByRole("button", { name: "重新读取电池", exact: true }).click();
  await expect(battery.getByRole("meter")).toBeVisible();
  await expect(battery.getByRole("alert")).toHaveCount(0);
});
