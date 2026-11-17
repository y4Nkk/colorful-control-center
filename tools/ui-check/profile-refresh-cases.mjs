import { openWorkspace } from "./workspace.mjs";
import { expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const gpu = page => page.getByRole("region", { name: "独显开关", exact: true });
async function ready(page) { await expect(gpu(page)).toContainText("电源控制开启"); }
export const PROFILE_REFRESH_IPC = `${MOCK_IPC};
  window.__uiFixture.gpuPower.devices[0].drivesDisplay = false;
  window.__uiFixture.gpuPower.devices[0].canDisable = true;
`;
export async function prepareProfileRefresh(page) {
  await page.goto("http://127.0.0.1:1450/"); await openWorkspace(page, "显示与显卡");
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
    await expect(gpu(page)).toContainText("已请求关断", { timeout: 2000 });
    await openWorkspace(page, "性能调校");
    await expect(page.getByRole("region", { name: "GPU 频率", exact: true })).toContainText("启动档案已关断独显", { timeout: 2000 });
    await openWorkspace(page, "显示与显卡");
    await expect(gpu(page).getByRole("radio", { name: "关闭独显", exact: true })).toBeChecked();
    expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|restore_)/.test(call.command)))).toEqual([]);
  } },
  { name: "阶段结果刷新当前事实，保留已编辑的独显与灯光草稿", run: async page => {
    await ready(page);
    await gpu(page).getByRole("radio", { name: "关闭独显", exact: true }).check();
    await openWorkspace(page, "灯光效果");
    await page.getByRole("spinbutton", { name: "闲置等待秒数", exact: true }).fill("90");
    await openWorkspace(page, "显示与显卡");
    await page.evaluate(() => {
      const data = window.__uiFixture;
      data.gpuPower.devices[0] = { ...data.gpuPower.devices[0], automaticPowerOff: true };
      data.lightingOptions = structuredClone(data.lightingOptions);
      data.lightingOptions.current.idleTimer.seconds = 45;
      data.profileApplication = { id: 811, revision: 1, origin: "manual", targetsApplied: true, name: "外部档案", pending: null,
        steps: [{ step: "灯光策略与外围灯区", ok: true, message: "已确认" }] };
    });
    await expect(gpu(page)).toContainText("自动关断允许 → 允许", { timeout: 2000 });
    await expect(gpu(page).getByRole("radio", { name: "关闭独显", exact: true })).toBeChecked();
    await openWorkspace(page, "灯光效果");
    await expect(page.getByRole("spinbutton", { name: "闲置等待秒数", exact: true })).toHaveValue("90");
    await expect(page.getByRole("region", { name: "灯光策略与外围灯区", exact: true })).toContainText("45", { timeout: 2000 });
  } },
  { name: "未编辑的灯光、充电及节能草稿采用档案后的新事实", run: async page => {
    await ready(page);
    await openWorkspace(page, "电池与节能");
    await openWorkspace(page, "灯光效果");
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
    await openWorkspace(page, "电池与节能");
    await expect(page.getByRole("switch", { name: "限制充电范围", exact: true })).toBeChecked({ timeout: 2000 });
    await expect(page.getByRole("switch", { name: "启用节能计划", exact: true })).toBeChecked({ timeout: 2000 });
    await openWorkspace(page, "灯光效果");
    await expect(page.getByRole("button", { name: "应用灯光选项", exact: true })).toBeDisabled();
  } },
];
