import { deviceRestoreButton, restoreDevice } from "./device-settings.mjs";
import { openWorkspace, openSystemArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => c.command === "apply_gpu_power").length);
async function open(page, mixed = false) {
  await page.addInitScript({ content: `${MOCK_IPC};Object.assign(window.__uiFixture.gpuPower.devices[0],{drivesDisplay:false,canDisable:true,automaticPowerOff:${mixed}});` });
  await page.goto("/"); await openWorkspace(page, "设备配置");
  return page.getByRole("region", { name: "独显电源", exact: true });
}
test("原厂关断直接提交并绑定策略，卡内可恢复混合初态", async ({ page }) => {
  const panel = await open(page, true);
  await expect(panel.locator('[data-enabled="on"]')).toBeVisible();
  await panel.screenshot({ path: "target/ui-check/oem-gpu-power.png" });
  expect(await writes(page)).toBe(0);
  await panel.getByRole("radio", { name: "关闭独显", exact: true }).click();
  await expect(panel.locator('[data-enabled="off"]')).toBeVisible();
  const request = await page.evaluate(() => window.__uiFixture.calls.find(call => call.command === "apply_gpu_power").args);
  expect(request).toMatchObject({ expected: { enabled: true, automaticPowerOff: true }, enabled: false });
  await openSystemArea(page, "概览");
  await expect(page.getByText("独显监控暂停", { exact: true })).toBeVisible();
  await openWorkspace(page, "设备配置");
  expect(await writes(page)).toBe(1);
  await restoreDevice(panel);
  await expect(panel.locator('[data-enabled="on"]')).toBeVisible();
  const device = await page.evaluate(() => window.__uiFixture.gpuPower.devices[0]);
  expect(device.enabled).toBe(true); expect(device.automaticPowerOff).toBe(true);
  await openSystemArea(page, "概览");
  await expect(page.getByText("独显监控暂停", { exact: true })).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
test("原厂工作失败回读实际状态，保留恢复入口", async ({ page }) => {
  const panel = await open(page);
  await page.evaluate(() => window.__uiFixture.gpuPowerFault = "原厂固件关闭命令失败；已补偿本次前态");
  await panel.getByRole("radio", { name: "关闭独显", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("固件关闭命令失败");
  await expect(panel.getByRole("radio", { name: "启用独显", exact: true })).toBeChecked();
  await expect(deviceRestoreButton(panel)).toBeEnabled();
  expect(await writes(page)).toBe(1);
  expect(await page.evaluate(() => window.__uiFixture.gpuPower.devices[0].enabled)).toBe(true);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
test("独显正在驱动输出时关断入口禁用", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "设备配置");
  const panel = page.getByRole("region", { name: "独显电源", exact: true });
  await expect(panel.getByRole("radio", { name: "关闭独显", exact: true })).toBeDisabled();
  expect(await writes(page)).toBe(0);
});
