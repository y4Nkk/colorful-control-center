import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { supplyModeFixture } from "./supply-mode-fixture.mjs";

const modes = page => page.getByRole("region", { name: "预设模式", exact: true });
const options = page => page.getByRole("region", { name: "性能选项", exact: true });
const preview = page => page.getByRole("region", { name: "档案预览", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
const energy = page => page.evaluate(() => window.__uiFixture.modeEnergy);
async function setup(page, managed = false) {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(managed => {
    window.__uiFixture.modeEnergy = { hardwarePState: managed, energyStar: managed ? null : 2 };
  }, managed);
  await page.goto("/"); await openWorkspace(page, "性能调校");
  await expect(modes(page).getByRole("button", { name: "确认应用模式", exact: true })).toBeDisabled();
}
async function select(page, name = "省电") {
  await modes(page).getByRole("radio", { name: new RegExp(name) }).click();
}
async function apply(page) {
  const count = (await calls(page, "apply_oem_power_mode")).length;
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(count);
  await modes(page).getByRole("button", { name: "确认应用模式", exact: true }).click();
  await expect(modes(page).getByRole("button", { name: "确认应用模式", exact: true })).toBeDisabled();
}
async function enableTurbo(page) {
  await options(page).getByRole("switch", { name: "Turbo 风扇", exact: true }).click();
  await options(page).getByRole("button", { name: "应用性能选项", exact: true }).click();
  await options(page).getByRole("button", { name: "确认应用性能选项？", exact: true }).click();
  await expect(options(page).getByText("当前开启", { exact: true })).toBeVisible();
}
async function restore(page, panel, label) {
  await panel.getByText("初始模式恢复预览", { exact: true }).click();
  await panel.getByRole("button", { name: label, exact: true }).click();
  expect(await calls(page, "restore_oem_power_mode")).toHaveLength(0);
  await panel.getByRole("button", { name: "恢复以上初始模式、选项及关联设置？", exact: true }).click();
}
async function selectProfile(page) {
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: "性能档案", exact: true }).click();
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
}
async function rereadModes(page) {
  const reads = (await calls(page, "read_oem_power_mode")).length;
  const navigation = page.getByRole("navigation", { name: "主导航" });
  await navigation.getByRole("button", { name: "性能档案", exact: true }).click();
  await navigation.getByRole("button", { name: "性能调校", exact: true }).click();
  await expect.poll(async () => (await calls(page, "read_oem_power_mode")).length).toBeGreaterThan(reads);
  await expect(modes(page).getByRole("radio", { name: /安静/ })).toBeEnabled();
}

test("模式节能目标只在选择卡片后的确认时执行，初始值独立预览并恢复", async ({ page }) => {
  await setup(page); await select(page);
  await expect(modes(page).getByLabel("模式联动目标")).toContainText("固件节能值 1");
  expect(await energy(page)).toEqual({ hardwarePState: false, energyStar: 2 });
  await apply(page);
  expect((await calls(page, "apply_oem_power_mode"))[0].args.expected.energy).toEqual({ hardwarePState: false, energyStar: 1 });
  await expect.poll(() => energy(page)).toEqual({ hardwarePState: false, energyStar: 1 });
  await modes(page).getByText("初始模式恢复预览", { exact: true }).click();
  await expect(modes(page).getByLabel("初始模式恢复目标")).toContainText("固件节能值 1 → 固件节能值 2");
  await modes(page).getByText("初始模式恢复预览", { exact: true }).click();
  await restore(page, modes(page), "恢复初始");
  await expect.poll(() => energy(page)).toEqual({ hardwarePState: false, energyStar: 2 });
});

test("仅选项没有节能所有权，恢复保留后来变化的固件值", async ({ page }) => {
  await setup(page); await enableTurbo(page);
  expect(await page.evaluate(() => window.__uiFixture.modeInitial.energy)).toBeNull();
  await options(page).getByText("初始模式恢复预览", { exact: true }).click();
  await expect(options(page).getByLabel("初始模式恢复目标")).not.toContainText("固件节能");
  await page.evaluate(() => window.__uiFixture.modeEnergy.energyStar = 0);
  await rereadModes(page);
  await options(page).getByRole("button", { name: "恢复模式与选项", exact: true }).click();
  await options(page).getByRole("button", { name: "恢复以上初始模式、选项及关联设置？", exact: true }).click();
  await expect(options(page).getByRole("switch", { name: "Turbo 风扇", exact: true })).not.toBeChecked();
  expect(await energy(page)).toEqual({ hardwarePState: false, energyStar: 0 });
});

test("选项之后才捕获首次节能值，扩充范围作废旧恢复确认", async ({ page }) => {
  await setup(page); await enableTurbo(page);
  await options(page).getByText("初始模式恢复预览", { exact: true }).click();
  await options(page).getByRole("button", { name: "恢复模式与选项", exact: true }).click();
  await page.evaluate(() => window.__uiFixture.modeEnergy.energyStar = 0);
  await rereadModes(page);
  await select(page, "安静"); await apply(page);
  await expect(options(page).getByRole("button", { name: "恢复模式与选项", exact: true })).toBeVisible();
  expect(await calls(page, "restore_oem_power_mode")).toHaveLength(0);
  expect(await page.evaluate(() => window.__uiFixture.modeInitial.energy.energyStar)).toBe(0);
  await expect(options(page).getByLabel("初始模式恢复目标")).toContainText("固件节能值 2 → 固件节能值 0");
  await options(page).getByRole("button", { name: "恢复模式与选项", exact: true }).click();
  await options(page).getByRole("button", { name: "恢复以上初始模式、选项及关联设置？", exact: true }).click();
  await expect.poll(() => energy(page)).toEqual({ hardwarePState: false, energyStar: 0 });
});

test("硬件管理时预览和载荷明确保留硬件管理状态", async ({ page }) => {
  await setup(page, true); await select(page);
  await expect(modes(page).getByLabel("模式联动目标")).toContainText("节能由硬件管理");
  await expect(modes(page).getByLabel("模式联动目标")).not.toContainText("固件节能值");
  await apply(page);
  expect((await calls(page, "apply_oem_power_mode"))[0].args.expected.energy).toEqual({ hardwarePState: true, energyStar: null });
  await restore(page, modes(page), "恢复初始");
  await expect.poll(() => page.evaluate(() => window.__uiFixture.modeInitial)).toBeNull();
});

test("确认期间能力改变拒绝旧节能目标且不创建恢复基线", async ({ page }) => {
  await setup(page); await select(page);
  await page.evaluate(() => window.__uiFixture.modeEnergy = { hardwarePState: true, energyStar: null });
  await modes(page).getByRole("button", { name: "确认应用模式", exact: true }).click();
  await expect(modes(page).getByRole("alert")).toContainText("固件节能目标已变化");
  expect(await page.evaluate(() => window.__uiFixture.performanceOptions.currentMode)).toBe(2);
  expect(await page.evaluate(() => window.__uiFixture.modeInitial)).toBeNull();
});

test("初始恢复能力变化显示原因并禁止再次执行", async ({ page }) => {
  await setup(page); await select(page); await apply(page);
  await modes(page).getByText("初始模式恢复预览", { exact: true }).click();
  await modes(page).getByRole("button", { name: "恢复初始", exact: true }).click();
  await page.evaluate(() => window.__uiFixture.modeEnergy = { hardwarePState: true, energyStar: null });
  await modes(page).getByRole("button", { name: "恢复以上初始模式、选项及关联设置？", exact: true }).click();
  await expect(modes(page).getByLabel("初始模式恢复目标").getByRole("alert")).toContainText("管理能力已变化");
  await expect(modes(page).getByRole("button", { name: "恢复初始", exact: true })).toBeDisabled();
  expect(await page.evaluate(() => window.__uiFixture.performanceOptions.currentMode)).toBe(1);
  expect(await page.evaluate(() => window.__uiFixture.modeInitial.energy.energyStar)).toBe(2);
});

test("档案复用节能预览和首次恢复，后续失败补偿实际前态", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => { window.__uiFixture.profile.settings.oemPowerMode = 1; });
  await selectProfile(page);
  await expect(preview(page).getByLabel("模式联动目标")).toContainText("固件节能值 1");
  await preview(page).getByRole("button", { name: "应用", exact: true }).click();
  expect(await calls(page, "apply_profile")).toHaveLength(0);
  await preview(page).getByRole("button", { name: "应用「隔离档案」？", exact: true }).click();
  await expect.poll(() => energy(page)).toEqual({ hardwarePState: false, energyStar: 1 });
  expect((await calls(page, "apply_profile"))[0].args.expectedMode.energy.energyStar).toBe(1);
  await page.evaluate(() => {
    window.__uiFixture.profileSteps = [{ step: "隔离后续失败", ok: false, message: "已拒绝" }];
    window.__uiFixture.profile.settings.oemPowerMode = 2;
  });
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await preview(page).getByRole("button", { name: "应用", exact: true }).click();
  await preview(page).getByRole("button", { name: "应用「隔离档案」？", exact: true }).click();
  await expect(preview(page).getByLabel("执行结果")).toContainText("恢复本次模式与固件节能");
  expect(await energy(page)).toEqual({ hardwarePState: false, energyStar: 1 });
  expect(await page.evaluate(() => window.__uiFixture.modeInitial.energy.energyStar)).toBe(2);
});

test("启动选择包含节能目标但不执行模式，供电六条件只展示单一目标", async ({ page }) => {
  const supply = supplyModeFixture();
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(supply => {
    window.__uiFixture.supplyMode = supply;
    window.__uiFixture.profile.settings.oemPowerMode = 1;
  }, supply);
  await page.goto("/"); await openWorkspace(page, "性能调校");
  await openWorkspace(page, "自动策略");
  const startup = page.getByRole("region", { name: "启动档案", exact: true });
  await startup.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  await startup.getByText("查看启动档案参数", { exact: true }).click();
  await expect(startup.getByLabel("模式联动目标")).toContainText("固件节能值 1");
  await startup.getByRole("button", { name: "保存", exact: true }).click();
  await startup.getByRole("button", { name: "启动时应用「隔离档案」？", exact: true }).click();
  expect((await calls(page, "set_startup_profile"))[0].args.expected.settings.oemPowerMode).toBe(1);
  expect(await energy(page)).toEqual({ hardwarePState: false, energyStar: 2 });
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(0);
  const card = page.getByRole("region", { name: "原厂供电模式", exact: true });
  for (const label of ["接电时", "使用电池时", "启动 · 插电", "启动 · 电池", "唤醒 · 插电", "唤醒 · 电池"]) {
    const section = card.getByRole("region", { name: label, exact: true });
    await expect(section.getByText(/^固件节能值 [01]$/u)).toHaveCount(1);
  }
});
