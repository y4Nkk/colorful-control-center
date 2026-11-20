import { openWorkspace, openPowerArea } from "./workspace.mjs";
import { expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { availableAction, previewConfiguration } from "./configuration-review.mjs";

const gpu = page => page.getByRole("region", { name: "独显电源", exact: true });
const gpuPower = page => gpu(page).locator(".ccc-gpu-power-visual");
async function ready(page) { await expect(gpuPower(page)).toHaveAttribute("data-enabled", "on"); }
export const PROFILE_REFRESH_IPC = `${MOCK_IPC};
  window.__uiFixture.gpuPower.devices[0].drivesDisplay = false;
  window.__uiFixture.gpuPower.devices[0].canDisable = true;
`;
export async function prepareProfileRefresh(page, url = "/") {
  await page.goto(url); await openWorkspace(page, "设备配置");
}
export const refreshCases = [
  { name: "启动档案结果跨页刷新独显节点和GPU调校事实", run: async page => {
    await ready(page);
    await page.evaluate(() => {
      const data = window.__uiFixture;
      data.gpuPower.devices[0] = { ...data.gpuPower.devices[0], enabled: false, automaticPowerOff: true, problem: 22 };
      data.gpu = { ...data.gpu, available: false, unavailableReason: "启动档案已关断独显" };
      data.profileApplication = { id: 810, revision: 1, origin: "startup", targetsApplied: true, name: "启动独显", pending: null,
        steps: [{ step: "独显目标复核", ok: true, message: "已确认" }] };
    });
    await expect(gpuPower(page)).toHaveAttribute("data-enabled", "off", { timeout: 2000 });
    await openWorkspace(page, "性能与散热");
    const tuning = page.getByRole("region", { name: "GPU 频率", exact: true });
    await expect(tuning).toContainText("不可用");
    await tuning.getByRole("button", { name: "详情", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "GPU 参数与诊断", exact: true })).toContainText("启动档案已关断独显");
    await page.keyboard.press("Escape");
    await openWorkspace(page, "设备配置");
    await expect(gpu(page).getByRole("radio", { name: "关闭独显", exact: true })).toBeChecked();
    expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|restore_)/.test(call.command)))).toEqual([]);
  } },
  { name: "阶段结果刷新当前事实，保留已编辑的独显与灯光草稿", run: async page => {
    await ready(page);
    await gpu(page).getByRole("radio", { name: "关闭独显", exact: true }).check();
    await openWorkspace(page, "键盘与灯光");
    await page.getByRole("spinbutton", { name: "闲置等待分钟", exact: true }).fill("1");
    await openWorkspace(page, "设备配置");
    await page.evaluate(() => {
      const data = window.__uiFixture;
      data.gpuPower.devices[0] = { ...data.gpuPower.devices[0], automaticPowerOff: true };
      data.lightingOptions = structuredClone(data.lightingOptions);
      data.lightingOptions.current.idleTimer.seconds = 45;
      data.profileApplication = { id: 811, revision: 1, origin: "manual", targetsApplied: true, name: "外部档案", pending: null,
        steps: [{ step: "灯光策略与外围灯区", ok: true, message: "已确认" }] };
    });
    await expect(gpuPower(page)).toContainText("自动关断 · 允许", { timeout: 2000 });
    await expect(gpu(page).getByRole("radio", { name: "关闭独显", exact: true })).toBeChecked();
    await openWorkspace(page, "键盘与灯光");
    await expect(page.getByRole("spinbutton", { name: "闲置等待分钟", exact: true })).toHaveValue("1");
    await expect(page.getByRole("spinbutton", { name: "闲置等待秒数", exact: true })).toHaveValue("45");
    await expect(await previewConfiguration(page, "灯光策略与外围灯区")).toContainText("45 秒后关灯 → 105 秒后关灯");
    await page.keyboard.press("Escape");
  } },
  { name: "未编辑的灯光、充电及节能草稿采用档案后的新事实", run: async page => {
    await ready(page);
    await openPowerArea(page, "电源设置");
    await openWorkspace(page, "键盘与灯光");
    await expect(page.getByRole("spinbutton", { name: "闲置等待秒数", exact: true })).toHaveValue("30");
    await page.evaluate(() => {
      const data = window.__uiFixture;
      data.lightingOptions = structuredClone(data.lightingOptions);
      data.lightingOptions.current.idleTimer.seconds = 45;
      data.battery = structuredClone(data.battery);
      data.battery.charge.current = { enabled: true, start: 70, stop: 90 };
      data.battery.chargeLockReason = "节能计划正在管理充电";
      data.energy = structuredClone(data.energy); data.energy.saved.enabled = true;
      data.profileApplication = { id: 812, revision: 1, origin: "startup", targetsApplied: true, name: "启动配置", pending: null,
        steps: [{ step: "电池配置", ok: true, message: "已确认" }] };
    });
    await expect(page.getByRole("spinbutton", { name: "闲置等待秒数", exact: true })).toHaveValue("45", { timeout: 2000 });
    await openPowerArea(page, "电源设置");
    await expect(page.getByRole("switch", { name: "限制充电范围", exact: true })).toBeChecked({ timeout: 2000 });
    await expect(page.getByRole("switch", { name: "启用节能计划", exact: true })).toBeChecked({ timeout: 2000 });
    await openWorkspace(page, "键盘与灯光");
    await expect(availableAction(page, "灯光策略与外围灯区")).toHaveCount(0);
  } },
];
