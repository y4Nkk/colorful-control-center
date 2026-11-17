import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => c.command === "apply_gpu_power").length);
async function open(page, mixed = false) {
  await page.addInitScript({ content: `${MOCK_IPC};Object.assign(window.__uiFixture.gpuPower.devices[0],{drivesDisplay:false,canDisable:true,automaticPowerOff:${mixed}});` });
  await page.goto("/"); await openWorkspace(page, "显示与显卡");
  return page.getByRole("region", { name: "独显开关", exact: true });
}
test("原厂关断预览绑定策略，确认重置并恢复混合初态", async ({ page }) => {
  const panel = await open(page, true);
  await expect(panel.getByText("允许 → 允许", { exact: true })).toBeVisible();
  await panel.screenshot({ path: "target/ui-check/oem-gpu-power.png" });
  expect(await writes(page)).toBe(0);
  await panel.getByRole("button", { name: "应用独显状态", exact: true }).click();
  expect(await writes(page)).toBe(0);
  await panel.getByRole("radio", { name: "启用独显", exact: true }).check();
  await expect(panel.getByRole("button", { name: "应用独显状态", exact: true })).toBeVisible();
  await panel.getByRole("radio", { name: "关闭独显", exact: true }).check();
  await panel.getByRole("button", { name: "应用独显状态", exact: true }).click();
  await panel.getByRole("button", { name: "确认关闭此独显并请求系统授权？", exact: true }).click();
  await expect(panel).toContainText("已请求关断");
  await openWorkspace(page, "控制台");
  await expect(page.getByText("独显监控暂停", { exact: true })).toBeVisible();
  await openWorkspace(page, "显示与显卡");
  expect(await writes(page)).toBe(1);
  await panel.getByRole("button", { name: "恢复独显初始", exact: true }).click();
  await panel.getByRole("button", { name: "恢复首次操作前的独显状态？", exact: true }).click();
  await expect(panel).toContainText("允许运行");
  const device = await page.evaluate(() => window.__uiFixture.gpuPower.devices[0]);
  expect(device.enabled).toBe(true); expect(device.automaticPowerOff).toBe(true);
  await openWorkspace(page, "控制台");
  await expect(page.getByText("独显监控暂停", { exact: true })).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
test("原厂工作失败保留目标与恢复入口，不重复请求写入", async ({ page }) => {
  const panel = await open(page);
  await panel.getByRole("radio", { name: "关闭独显", exact: true }).check();
  await page.evaluate(() => window.__uiFixture.gpuPowerFault = "原厂固件关闭命令失败；已补偿本次前态");
  await panel.getByRole("button", { name: "应用独显状态", exact: true }).click();
  await panel.getByRole("button", { name: "确认关闭此独显并请求系统授权？", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("固件关闭命令失败");
  await expect(panel.getByRole("radio", { name: "关闭独显", exact: true })).toBeChecked();
  await expect(panel.getByRole("button", { name: "恢复独显初始", exact: true })).toBeEnabled();
  expect(await writes(page)).toBe(1);
  expect(await page.evaluate(() => window.__uiFixture.gpuPower.devices[0].enabled)).toBe(true);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
test("独显正在驱动输出时关断入口禁用", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "显示与显卡");
  const panel = page.getByRole("region", { name: "独显开关", exact: true });
  await expect(panel.getByRole("radio", { name: "关闭独显", exact: true })).toBeDisabled();
  expect(await writes(page)).toBe(0);
});
