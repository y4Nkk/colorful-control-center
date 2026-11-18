import { availableAction, configurationDialog, confirmConfiguration, openConfigurationReview } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { supplyModeFixture } from "./supply-mode-fixture.mjs";
import { openProfileResults, openStartupParameters } from "./profile-review.mjs";

const modes = page => page.getByRole("region", { name: "预设模式", exact: true });
const options = page => page.getByRole("region", { name: "性能选项", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
const energy = page => page.evaluate(() => window.__uiFixture.modeEnergy);
async function setup(page, managed = false) {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(managed => {
    window.__uiFixture.modeEnergy = { hardwarePState: managed, energyStar: managed ? null : 2 };
  }, managed);
  await page.goto("/"); await openWorkspace(page, "性能调校");
  await expect(modes(page).getByRole("radio", { name: /省电/ })).toBeEnabled();
  await expect(availableAction(page, "预设模式")).toHaveCount(0);
}
async function select(page, name = "省电") {
  await modes(page).getByRole("radio", { name: new RegExp(name) }).click();
}
async function apply(page, review) {
  await confirmConfiguration(review ?? await openConfigurationReview(page, "预设模式"));
  await expect(availableAction(page, "预设模式")).toHaveCount(0);
}
async function enableTurbo(page) {
  await options(page).getByRole("switch", { name: "Turbo 风扇", exact: true }).click();
  await openConfigurationReview(page, "性能选项");
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__uiFixture.performanceOptions.current.turboFan)).toBe(true);
}
async function restore(page, review) {
  const dialog = review ?? await openConfigurationReview(page, "模式与性能选项", true);
  expect(await calls(page, "restore_oem_power_mode")).toHaveLength(0);
  await confirmConfiguration(dialog, "确认恢复");
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
  const review = await openConfigurationReview(page, "预设模式");
  await expect(review.getByLabel("模式联动目标")).toContainText("固件节能值 1");
  expect(await energy(page)).toEqual({ hardwarePState: false, energyStar: 2 });
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(0);
  await apply(page, review);
  expect((await calls(page, "apply_oem_power_mode"))[0].args.expected.energy).toEqual({ hardwarePState: false, energyStar: 1 });
  await expect.poll(() => energy(page)).toEqual({ hardwarePState: false, energyStar: 1 });
  const recovery = await openConfigurationReview(page, "模式与性能选项", true);
  await expect(recovery.getByLabel("初始模式恢复目标")).toContainText("固件节能值 1 → 固件节能值 2");
  await restore(page, recovery);
  await expect.poll(() => energy(page)).toEqual({ hardwarePState: false, energyStar: 2 });
});

test("仅选项没有节能所有权，恢复保留后来变化的固件值", async ({ page }) => {
  await setup(page); await enableTurbo(page);
  expect(await page.evaluate(() => window.__uiFixture.modeInitial.energy)).toBeNull();
  const recovery = await openConfigurationReview(page, "模式与性能选项", true);
  await expect(recovery.getByLabel("初始模式恢复目标")).not.toContainText("固件节能");
  await page.keyboard.press("Escape");
  await page.evaluate(() => window.__uiFixture.modeEnergy.energyStar = 0);
  await rereadModes(page);
  await openConfigurationReview(page, "模式与性能选项", true);
  await configurationDialog(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(options(page).getByRole("switch", { name: "Turbo 风扇", exact: true })).not.toBeChecked();
  await expect(availableAction(page, "性能选项")).toHaveCount(0);
  expect(await energy(page)).toEqual({ hardwarePState: false, energyStar: 0 });
});

test("选项之后才捕获首次节能值，扩充范围作废旧恢复确认", async ({ page }) => {
  await setup(page); await enableTurbo(page);
  const early = await openConfigurationReview(page, "模式与性能选项", true);
  await expect(early.getByLabel("初始模式恢复目标")).not.toContainText("固件节能");
  await page.keyboard.press("Escape");
  await page.evaluate(() => window.__uiFixture.modeEnergy.energyStar = 0);
  await rereadModes(page);
  await select(page, "安静"); await apply(page);
  await expect(availableAction(page, "模式与性能选项", true)).toBeVisible();
  expect(await calls(page, "restore_oem_power_mode")).toHaveLength(0);
  expect(await page.evaluate(() => window.__uiFixture.modeInitial.energy.energyStar)).toBe(0);
  const recovery = await openConfigurationReview(page, "模式与性能选项", true);
  await expect(recovery.getByLabel("初始模式恢复目标")).toContainText("固件节能值 2 → 固件节能值 0");
  await restore(page, recovery);
  await expect.poll(() => energy(page)).toEqual({ hardwarePState: false, energyStar: 0 });
});

test("硬件管理时预览和载荷明确保留硬件管理状态", async ({ page }) => {
  await setup(page, true); await select(page);
  const review = await openConfigurationReview(page, "预设模式");
  await expect(review.getByLabel("模式联动目标")).toContainText("节能由硬件管理");
  await expect(review.getByLabel("模式联动目标")).not.toContainText("固件节能值");
  await apply(page, review);
  expect((await calls(page, "apply_oem_power_mode"))[0].args.expected.energy).toEqual({ hardwarePState: true, energyStar: null });
  await restore(page);
  await expect.poll(() => page.evaluate(() => window.__uiFixture.modeInitial)).toBeNull();
});

test("确认期间能力改变拒绝旧节能目标且不创建恢复基线", async ({ page }) => {
  await setup(page); await select(page);
  await page.evaluate(() => window.__uiFixture.modeEnergy = { hardwarePState: true, energyStar: null });
  await confirmConfiguration(await openConfigurationReview(page, "预设模式"));
  await expect(modes(page).getByRole("alert")).toContainText("固件节能目标已变化");
  expect(await page.evaluate(() => window.__uiFixture.performanceOptions.currentMode)).toBe(2);
  expect(await page.evaluate(() => window.__uiFixture.modeInitial)).toBeNull();
});

test("初始恢复能力变化显示原因并禁止再次执行", async ({ page }) => {
  await setup(page); await select(page); await apply(page);
  await openConfigurationReview(page, "模式与性能选项", true);
  await page.evaluate(() => window.__uiFixture.modeEnergy = { hardwarePState: true, energyStar: null });
  await configurationDialog(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await modes(page).getByRole("button", { name: "模式恢复暂不可用", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "模式恢复暂不可用" })).toContainText("管理能力已变化");
  await expect(availableAction(page, "模式与性能选项", true)).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.performanceOptions.currentMode)).toBe(1);
  expect(await page.evaluate(() => window.__uiFixture.modeInitial.energy.energyStar)).toBe(2);
});

test("档案复用节能预览和首次恢复，后续失败补偿实际前态", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => { window.__uiFixture.profile.settings.oemPowerMode = 1; });
  await selectProfile(page);
  const review = await openConfigurationReview(page);
  await expect(review.getByLabel("模式联动目标")).toContainText("固件节能值 1");
  expect(await calls(page, "apply_profile")).toHaveLength(0);
  await configurationDialog(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect.poll(() => energy(page)).toEqual({ hardwarePState: false, energyStar: 1 });
  expect((await calls(page, "apply_profile"))[0].args.expectedMode.energy.energyStar).toBe(1);
  await page.evaluate(() => {
    window.__uiFixture.profileSteps = [{ step: "隔离后续失败", ok: false, message: "已拒绝" }];
    window.__uiFixture.profile.settings.oemPowerMode = 2;
  });
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await openConfigurationReview(page);
  await configurationDialog(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(await openProfileResults(page)).toContainText("恢复本次模式与固件节能");
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
  const parameters = await openStartupParameters(page);
  await expect(parameters.getByLabel("模式联动目标")).toContainText("固件节能值 1");
  await page.keyboard.press("Escape");
  await confirmConfiguration(configurationDialog(page));
  expect((await calls(page, "set_startup_profile"))[0].args.expected.settings.oemPowerMode).toBe(1);
  expect(await energy(page)).toEqual({ hardwarePState: false, energyStar: 2 });
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(0);
  const card = page.getByRole("region", { name: "原厂供电模式", exact: true });
  await card.getByRole("button", { name: "详情", exact: true }).click();
  const details = page.getByRole("dialog", { name: "供电状态与切换规则", exact: true });
  for (const label of ["接电时", "使用电池时", "启动 · 插电", "启动 · 电池", "唤醒 · 插电", "唤醒 · 电池"]) {
    const section = details.getByRole("region", { name: label, exact: true });
    await section.getByRole("button", { name: "联动目标", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "联动目标", exact: true }).getByText(/^固件节能值 [01]$/u)).toHaveCount(1);
    await page.keyboard.press("Escape");
  }
});
