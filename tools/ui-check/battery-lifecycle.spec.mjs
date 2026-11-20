import { availableAction } from "./configuration-review.mjs";
import { openPowerArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const battery = page => page.getByRole("region", { name: "电池与充电", exact: true });
const energy = page => page.getByRole("region", { name: "电池节能计划", exact: true });
const calibration = page => page.getByRole("region", { name: "电池校准", exact: true });

test("严格模式及快速切页重新读取，旧响应不覆盖新电池事实或锁住子域", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    window.__uiFixture.calibrationSupported = true;
    const invoke = window.__TAURI_INTERNALS__.invoke, counts = {}, releases = [];
    window.__releaseBatteryReads = () => releases.forEach(release => release());
    window.__batteryReadCounts = counts;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      const result = await invoke(command, args);
      if (!["read_battery_profile", "read_energy_policy", "read_battery_calibration"].includes(command)) return result;
      counts[command] = (counts[command] ?? 0) + 1;
      if (counts[command] > 2) return result;
      if (command === "read_battery_profile") result.power.percent = 31;
      if (command === "read_energy_policy") result.saved.enabled = true;
      if (command === "read_battery_calibration") result.canStart = false;
      return new Promise(resolve => releases.push(() => resolve(result)));
    };
  });
  await page.goto("/"); await openPowerArea(page, "电源设置");
  await expect.poll(() => page.evaluate(() => Object.values(window.__batteryReadCounts).every(count => count === 2) && Object.keys(window.__batteryReadCounts).length === 3)).toBe(true);
  const nav = page.getByRole("navigation", { name: "主导航" });
  await openPowerArea(page, "配置档案");
  await nav.getByRole("button", { name: "电源与档案", exact: true }).click();
  await expect(battery(page).getByRole("meter", { name: "当前电量", exact: true })).toHaveAttribute("aria-valuenow", "62");
  await expect(energy(page).getByRole("switch", { name: "启用节能计划", exact: true })).not.toBeChecked();
  await expect(calibration(page).getByRole("button", { name: "开始电池校准", exact: true })).toBeEnabled();
  await page.evaluate(async () => { window.__releaseBatteryReads(); await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame); });
  await expect(battery(page).getByRole("meter", { name: "当前电量", exact: true })).toHaveAttribute("aria-valuenow", "62");
  await expect(energy(page).getByRole("switch", { name: "启用节能计划", exact: true })).not.toBeChecked();
  await expect(calibration(page).getByRole("button", { name: "开始电池校准", exact: true })).toBeEnabled();
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|start_|restore_)/.test(call.command)))).toEqual([]);
});

test("原厂节能已启用时手动充电入口保留状态并禁止写入", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => { window.__uiFixture.energy.saved.enabled = true; });
  await page.goto("/"); await openPowerArea(page, "电源设置");
  await expect(battery(page)).toContainText("节能计划管理充电阈值");
  await expect(battery(page).getByRole("switch", { name: "限制充电范围", exact: true })).toHaveAttribute("aria-disabled", "true");
  await expect(availableAction(page, "充电保护")).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "apply_battery_charge"))).toEqual([]);
});
