import { openProfileResults, openProfileSave, openStartupParameters, saveProfileButton, showProfileMenu, showProfileSettings } from "./profile-review.mjs";
import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openDeviceRestore } from "./device-settings.mjs";
import { openWorkspace, openPowerArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { installMicrophoneFixture } from "./microphone-fixture.mjs";
const preview = page => page.getByRole("region", { name: "档案预览", exact: true });
const navigate = (page, title) => page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: title, exact: true }).click();
async function select(page) {
  await openPowerArea(page, "配置档案");
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await showProfileSettings(page);
}
async function apply(page) {
  await previewConfiguration(page);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.addInitScript(installMicrophoneFixture); await page.goto("/"); await openWorkspace(page, "设备配置");
});

test("档案逐端点预览，确认后刷新控制台并共用混合状态恢复", async ({ page }) => {
  await select(page);
  const card = preview(page).getByRole("region", { name: "档案麦克风", exact: true });
  await expect(card).toContainText("内置麦克风已静音");
  await expect(card).toContainText("USB麦克风未静音");
  expect(await page.evaluate(() => window.__uiFixture.microphones.initial)).toBeNull();
  await apply(page);
  const results = await openProfileResults(page);
  await expect(results).toContainText("麦克风静音");
  await page.keyboard.press("Escape");
  await navigate(page, "设备配置");
  const panel = page.getByRole("region", { name: "麦克风静音", exact: true });
  await panel.getByRole("button", { name: "详情", exact: true }).click();
  const details = panel.getByRole("dialog", { name: "麦克风详情", exact: true });
  await expect(details.locator(".ccc-fact-row").filter({ hasText: "USB麦克风" }).locator("dd")).toHaveText("未静音");
  await page.keyboard.press("Escape");
  const restore = await openDeviceRestore(panel);
  await expect(restore.locator(".ccc-fact-row").filter({ hasText: "USB麦克风" }).locator("dd")).toHaveText("已静音");
  await restore.getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__uiFixture.microphones.current.endpoints.map(endpoint => endpoint.muted)))
    .toEqual([false, true]);
});

test("捕获导出及导入保留混合状态和独立保存偏好", async ({ page }) => {
  await openPowerArea(page, "配置档案");
  await openProfileSave(page, "完整麦克风", ["设备功能"]);
  await saveProfileButton(page).click();
  const pending = page.waitForEvent("download");
  await (await showProfileMenu(page)).getByRole("button", { name: "导出档案", exact: true }).click();
  const file = JSON.parse(await readFile(await (await pending).path(), "utf8"));
  expect(file.settings.microphones.savedMuted).toBe(false);
  expect(file.settings.microphones.endpoints.map(endpoint => endpoint.muted)).toEqual([false, true]);
  file.name = "导入麦克风"; file.settings.microphones.endpoints.forEach(endpoint => endpoint.muted = true); file.settings.microphones.savedMuted = true;
  await page.locator('input[type="file"]').setInputFiles({ name: "麦克风.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(file)) });
  await showProfileSettings(page);
  const card = preview(page).getByRole("region", { name: "档案麦克风", exact: true });
  await expect(card.getByText("已静音", { exact: true })).toHaveCount(2);
  expect(await page.evaluate(() => window.__uiFixture.microphones.current.endpoints.map(endpoint => endpoint.muted))).toEqual([false, true]);
});

test("后续独显失败恢复麦克风本次前态并保留首次恢复", async ({ page }) => {
  await page.evaluate(() => {
    const data = window.__uiFixture, device = data.gpuPower.devices[0];
    data.profile.settings.gpuPower = { id: device.id, name: device.name, values: { enabled: true, automaticPowerOff: !device.automaticPowerOff } };
    data.gpuPowerFault = "隔离后续独显失败";
  });
  await select(page); await apply(page);
  const results = await openProfileResults(page);
  await expect(results).toContainText("恢复本次麦克风状态");
  const state = await page.evaluate(() => window.__uiFixture.microphones);
  expect(state.current.endpoints.map(endpoint => endpoint.muted)).toEqual([false, true]);
  expect(state.initial.endpoints.map(endpoint => endpoint.muted)).toEqual([false, true]);
});

test("显示试用取消不修改麦克风，保留后才执行完整目标", async ({ page }) => {
  await page.evaluate(() => { const data = window.__uiFixture; data.profile.settings.display = { displays: [{ ...data.display.displays[0].current, frequency: 144 }] }; });
  await select(page); await apply(page);
  await expect(page.getByRole("button", { name: "保留显示并继续档案", exact: true })).toBeVisible();
  expect(await page.evaluate(() => window.__uiFixture.microphones.initial)).toBeNull();
  await page.getByRole("button", { name: "恢复本次显示并停止", exact: true }).click();
  expect(await page.evaluate(() => window.__uiFixture.microphones.initial)).toBeNull();
  await apply(page);
  await page.getByRole("button", { name: "保留显示并继续档案", exact: true }).click();
  const results = await openProfileResults(page);
  await expect(results).toContainText("麦克风静音");
  expect(await page.evaluate(() => window.__uiFixture.microphones.current.endpoints.map(endpoint => endpoint.muted))).toEqual([true, false]);
});

test("启动选择共用完整麦克风预览，确认只保存选择", async ({ page }) => {
  await openPowerArea(page, "电源设置");
  const panel = page.getByRole("region", { name: "启动档案", exact: true });
  await panel.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  const parameters = await openStartupParameters(page);
  await expect(parameters.getByRole("region", { name: "档案麦克风", exact: true })).toContainText("USB麦克风未静音");
  await page.keyboard.press("Escape");
  await configurationView(page).getByRole("button", { name: "授权启动应用档案", exact: true }).click();
  const calls = await page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "set_startup_profile"));
  expect(calls[0].args.expected.settings.microphones.endpoints.map(endpoint => endpoint.muted)).toEqual([true, false]);
  expect(await page.evaluate(() => window.__uiFixture.microphones.initial)).toBeNull();
});
