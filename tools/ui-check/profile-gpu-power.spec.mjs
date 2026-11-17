import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "档案预览", exact: true });
const gpu = page => panel(page).getByLabel("档案独显配置", { exact: true });
async function select(page) {
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: "性能档案", exact: true }).click();
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
}
async function apply(page) {
  await panel(page).getByRole("button", { name: "应用", exact: true }).click();
  await panel(page).getByRole("button", { name: "应用「隔离档案」？", exact: true }).click();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const data = window.__uiFixture, before = data.gpuPower.devices[0];
    before.drivesDisplay = false; before.canDisable = true;
    data.profile.settings.gpuPower = { id: before.id, name: before.name, values: { enabled: false, automaticPowerOff: true } };
    data.profile.settings.pl1W = 77; data.profile.settings.coreOffsetMv = -40;
  });
  await page.goto("/"); await openWorkspace(page, "显示与显卡");
});
test("节点和自动策略分别预览，确认前零写入并绑定完整来源", async ({ page }) => {
  await select(page);
  await expect(gpu(page)).toContainText("开启 → 关闭");
  await expect(gpu(page)).toContainText("关闭 → 开启");
  await expect(gpu(page).getByRole("meter")).toHaveCount(0);
  await expect(panel(page).getByRole("meter")).toHaveCount(0);
  await expect(panel(page).getByLabel("保存设置 · CPU 持续功耗上限", { exact: true })).toHaveText("77 W");
  await panel(page).getByRole("button", { name: "应用", exact: true }).click();
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "apply_profile"))).toEqual([]);
  await panel(page).getByRole("button", { name: "应用「隔离档案」？", exact: true }).click();
  await expect(panel(page).getByLabel("执行结果")).toContainText("独显目标复核");
  const state = await page.evaluate(() => {
    const data = window.__uiFixture;
    return { device: data.gpuPower.devices[0], args: data.calls.find(call => call.command === "apply_profile").args };
  });
  expect(state.device.enabled).toBe(false); expect(state.device.automaticPowerOff).toBe(true);
  expect(state.args.expectedGpuPower.before.enabled).toBe(true);
  expect(state.args.expectedGpuPower.target).toEqual({ enabled: false, automaticPowerOff: true });
});
test("混合策略保持独立，不被压成节点开关的反值", async ({ page }) => {
  await page.evaluate(() => window.__uiFixture.profile.settings.gpuPower.values = { enabled: true, automaticPowerOff: true });
  await select(page);
  await expect(gpu(page)).toContainText("开启 → 开启");
  await expect(gpu(page)).toContainText("关闭 → 开启");
  await apply(page);
  await expect(panel(page).getByLabel("执行结果")).toContainText("独显目标复核");
  expect(await page.evaluate(() => window.__uiFixture.gpuPower.devices[0].automaticPowerOff)).toBe(true);
});
test("独显预览失败保留文件目标并禁止手动执行", async ({ page }) => {
  await page.evaluate(() => window.__uiFixture.profileGpuPreviewFault = "隔离独显预览断开");
  await select(page);
  await expect(gpu(page)).toContainText("目标关闭");
  await expect(panel(page).getByRole("button", { name: "应用", exact: true })).toBeDisabled();
  await expect(panel(page).getByRole("button", { name: "导出", exact: true })).toBeEnabled();
});
test("旧来源在执行时拒绝，不授权或改变独显", async ({ page }) => {
  await select(page);
  await page.evaluate(() => window.__uiFixture.gpuPower.devices[0].name = "外部更新");
  await apply(page);
  await expect(page.getByRole("alert").first()).toContainText("独显预览已变化");
  expect(await page.evaluate(() => window.__uiFixture.profileGpuAuthorizations ?? 0)).toBe(0);
  expect(await page.evaluate(() => window.__uiFixture.gpuPower.devices[0].enabled)).toBe(true);
});
test("多屏确认前不执行独显，明确保留才继续", async ({ page }) => {
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.profile.settings.display = { displays: data.display.displays.map(display => ({ ...display.current, frequency: 144 })) };
  });
  await select(page); await apply(page);
  const host = page.getByRole("region", { name: "档案显示确认", exact: true });
  await expect(host).toBeVisible();
  expect(await page.evaluate(() => window.__uiFixture.profileGpuAuthorizations ?? 0)).toBe(0);
  await host.getByRole("button", { name: "保留显示并继续档案", exact: true }).click();
  await expect(host).toHaveCount(0);
  await expect(panel(page).getByLabel("执行结果")).toContainText("独显目标复核");
  expect(await page.evaluate(() => window.__uiFixture.gpuPower.devices[0].enabled)).toBe(false);
});
test("开启后驱动复核失败显示本次恢复，不能报告全部完成", async ({ page }) => {
  await page.evaluate(() => {
    const data = window.__uiFixture, device = data.gpuPower.devices[0];
    device.enabled = false; device.problem = 22; device.automaticPowerOff = true;
    data.profile.settings.gpuPower.values = { enabled: true, automaticPowerOff: false };
    data.profile.settings.gpu = { coreOffsetMhz: 60, memOffsetMhz: null };
    data.profileGpuDriverReady = false;
  });
  await select(page); await apply(page);
  await expect(panel(page).getByLabel("执行结果")).toContainText("恢复本次独显状态");
  await expect(panel(page).getByLabel("最近应用记录")).toContainText("未完成全部目标");
  expect(await page.evaluate(() => window.__uiFixture.gpuPower.devices[0].enabled)).toBe(false);
});
test("启动选择复用独显预览且保存意图不执行硬件", async ({ page }) => {
  await openWorkspace(page, "自动策略");
  const startup = page.getByRole("region", { name: "启动档案", exact: true });
  await startup.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  await startup.locator("summary").filter({ hasText: "查看启动档案参数" }).click();
  await expect(startup.getByLabel("档案独显配置")).toContainText("开启 → 关闭");
  await startup.getByRole("button", { name: "保存", exact: true }).click();
  await startup.getByRole("button", { name: "启动时应用「隔离档案」？", exact: true }).click();
  const saved = await page.evaluate(() => window.__uiFixture.calls.find(call => call.command === "set_startup_profile"));
  expect(saved.args.expected.settings.gpuPower.values).toEqual({ enabled: false, automaticPowerOff: true });
  expect(await page.evaluate(() => window.__uiFixture.profileGpuAuthorizations ?? 0)).toBe(0);
});
test("保存当前捕获实际节点与混合策略，不复制旧档案目标", async ({ page }) => {
  await page.evaluate(() => window.__uiFixture.gpuPower.devices[0].automaticPowerOff = true);
  await select(page);
  await page.getByPlaceholder("游戏 / 静音").fill("当前独显");
  await page.getByRole("button", { name: "保存当前", exact: true }).click();
  await page.getByRole("button", { name: "保存当前设置？", exact: true }).click();
  await expect(panel(page).getByRole("heading", { name: "当前独显", exact: true })).toBeVisible();
  const saved = await page.evaluate(() => window.__uiFixture.calls.find(call => call.command === "save_profile"));
  expect(saved.args.args.settings.gpuPower.values).toEqual({ enabled: true, automaticPowerOff: true });
  expect(await page.evaluate(() => window.__uiFixture.profileGpuAuthorizations ?? 0)).toBe(0);
});
test("宽窄深浅保持模块边框与数值语义", async ({ page }) => {
  await select(page);
  for (const theme of ["light", "dark"]) for (const width of [1440, 360]) {
    await page.setViewportSize({ width, height: 1500 });
    await page.evaluate(value => document.documentElement.dataset.theme = value, theme);
    await gpu(page).scrollIntoViewIfNeeded();
    const bounds = await gpu(page).boundingBox();
    expect(bounds.x).toBeGreaterThanOrEqual(0); expect(bounds.x + bounds.width).toBeLessThanOrEqual(width + 1);
    await expect(panel(page).getByRole("meter")).toHaveCount(0);
    await page.screenshot({ path: `target/ui-check/profile-gpu-power-${theme}-${width}.png`, fullPage: true });
  }
});
