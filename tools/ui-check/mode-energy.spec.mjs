import { availableAction, configurationView, submitConfiguration, previewConfiguration } from "./configuration-review.mjs";
import { openWorkspace, openPowerArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openProfileResults, openStartupParameters, showProfileSettings } from "./profile-review.mjs";

const modes = page => page.getByRole("region", { name: "预设模式", exact: true });
const options = page => page.getByRole("region", { name: "性能选项", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
const energy = page => page.evaluate(() => window.__uiFixture.modeEnergy);
async function setup(page, managed = false) {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(managed => {
    window.__uiFixture.modeEnergy = { hardwarePState: managed, energyStar: managed ? null : 2 };
  }, managed);
  await page.goto("/"); await openWorkspace(page, "性能与散热");
  await expect(modes(page).getByRole("radio", { name: /省电/ })).toBeEnabled();
  await expect(availableAction(page, "预设模式")).toHaveCount(0);
}
async function select(page, name = "省电") {
  const radio = modes(page).getByRole("radio", { name: new RegExp(`^${name}`) });
  await radio.click();
  await expect(radio).toBeChecked();
  await expect(radio).toBeEnabled();
  await expect(availableAction(page, "预设模式")).toHaveCount(0);
}
async function enableTurbo(page) {
  await options(page).getByRole("switch", { name: "Turbo 风扇", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__uiFixture.performanceOptions.current.turboFan)).toBe(true);
}
async function selectProfile(page) {
  await openPowerArea(page, "配置档案");
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
}

test("激活卡片同步执行模式及节能目标", async ({ page }) => {
  await setup(page);
  expect(await energy(page)).toEqual({ hardwarePState: false, energyStar: 2 });
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(0);
  await select(page);
  const requests = await calls(page, "apply_oem_power_mode");
  expect(requests).toHaveLength(1);
  expect(requests[0].args.expected.energy).toEqual({ hardwarePState: false, energyStar: 1 });
  await expect.poll(() => energy(page)).toEqual({ hardwarePState: false, energyStar: 1 });
});

test("仅选项没有节能所有权", async ({ page }) => {
  await setup(page); await enableTurbo(page);
  expect(await page.evaluate(() => window.__uiFixture.modeInitial.energy)).toBeNull();
});

test("切换载荷保留固件的硬件管理状态", async ({ page }) => {
  await setup(page, true); await select(page);
  expect((await calls(page, "apply_oem_power_mode"))[0].args.expected.energy).toEqual({ hardwarePState: true, energyStar: null });
});

test("首次读取后能力改变会拒绝卡片发出的旧目标", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => window.__uiFixture.modeEnergy = { hardwarePState: true, energyStar: null });
  await modes(page).getByRole("radio", { name: "省电", exact: true }).click();
  await expect(modes(page).getByRole("alert")).toContainText("固件节能目标已变化");
  expect(await page.evaluate(() => window.__uiFixture.performanceOptions.currentMode)).toBe(2);
  expect(await page.evaluate(() => window.__uiFixture.modeInitial)).toBeNull();
});

test("档案复用节能预览和首次恢复，后续失败补偿实际前态", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => { window.__uiFixture.profile.settings.oemPowerMode = 1; });
  await selectProfile(page);
  const review = await showProfileSettings(page);
  await expect(review.getByLabel("模式联动目标")).toContainText("电脑节能等级 1");
  await previewConfiguration(page);
  expect(await calls(page, "apply_profile")).toHaveLength(0);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect.poll(() => energy(page)).toEqual({ hardwarePState: false, energyStar: 1 });
  expect((await calls(page, "apply_profile"))[0].args.expectedMode.energy.energyStar).toBe(1);
  await page.evaluate(() => {
    window.__uiFixture.profileSteps = [{ step: "隔离后续失败", ok: false, message: "已拒绝" }];
    window.__uiFixture.profile.settings.oemPowerMode = 2;
  });
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await previewConfiguration(page);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(await openProfileResults(page)).toContainText("恢复本次模式与固件节能");
  expect(await energy(page)).toEqual({ hardwarePState: false, energyStar: 1 });
  expect(await page.evaluate(() => window.__uiFixture.modeInitial.energy.energyStar)).toBe(2);
});

test("启动选择包含节能目标但不执行模式", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => { window.__uiFixture.profile.settings.oemPowerMode = 1; });
  await page.goto("/"); await openWorkspace(page, "概览");
  await openPowerArea(page, "电源设置");
  const startup = page.getByRole("region", { name: "启动档案", exact: true });
  await startup.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  const parameters = await openStartupParameters(page);
  await expect(parameters.getByLabel("模式联动目标")).toContainText("电脑节能等级 1");
  await page.keyboard.press("Escape");
  await submitConfiguration(configurationView(page));
  expect((await calls(page, "set_startup_profile"))[0].args.expected.settings.oemPowerMode).toBe(1);
  expect(await energy(page)).toEqual({ hardwarePState: false, energyStar: 2 });
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(0);
});
