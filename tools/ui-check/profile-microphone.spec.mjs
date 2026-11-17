import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { installMicrophoneFixture } from "./microphone-fixture.mjs";
const preview = page => page.getByRole("region", { name: "档案预览", exact: true });
const navigate = (page, title) => page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: title, exact: true }).click();
async function select(page) {
  await navigate(page, "性能档案");
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
}
async function apply(page) {
  await preview(page).getByRole("button", { name: "应用", exact: true }).click();
  await preview(page).getByRole("button", { name: "应用「隔离档案」？", exact: true }).click();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.addInitScript(installMicrophoneFixture); await page.goto("/"); await openWorkspace(page, "键盘与输入");
});

test("档案逐端点预览，确认后刷新控制台并共用混合状态恢复", async ({ page }) => {
  await select(page);
  const card = preview(page).getByRole("region", { name: "档案麦克风", exact: true });
  await expect(card).toContainText("内置麦克风已静音");
  await expect(card).toContainText("USB麦克风未静音");
  expect(await page.evaluate(() => window.__uiFixture.microphones.initial)).toBeNull();
  await apply(page);
  await expect(preview(page).getByLabel("执行结果")).toContainText("麦克风静音");
  await navigate(page, "键盘与输入");
  const panel = page.getByRole("region", { name: "麦克风静音", exact: true });
  await expect(panel.getByRole("region", { name: "当前麦克风状态", exact: true })).toContainText("USB麦克风未静音");
  await panel.getByRole("button", { name: "恢复麦克风初始", exact: true }).click();
  await panel.getByRole("button", { name: "恢复各麦克风修改前的状态？", exact: true }).click();
  await expect(panel.getByRole("region", { name: "当前麦克风状态", exact: true })).toContainText("USB麦克风已静音");
});

test("捕获导出及导入保留混合状态和独立保存偏好", async ({ page }) => {
  await navigate(page, "性能档案");
  await page.getByPlaceholder("游戏 / 静音").fill("完整麦克风");
  await page.getByRole("button", { name: "保存当前", exact: true }).click();
  await page.getByRole("button", { name: "保存当前设置？", exact: true }).click();
  const pending = page.waitForEvent("download");
  await preview(page).getByRole("button", { name: "导出", exact: true }).click();
  const file = JSON.parse(await readFile(await (await pending).path(), "utf8"));
  expect(file.settings.microphones.savedMuted).toBe(false);
  expect(file.settings.microphones.endpoints.map(endpoint => endpoint.muted)).toEqual([false, true]);
  file.name = "导入麦克风"; file.settings.microphones.endpoints.forEach(endpoint => endpoint.muted = true); file.settings.microphones.savedMuted = true;
  await page.locator('input[type="file"]').setInputFiles({ name: "麦克风.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(file)) });
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
  await expect(preview(page).getByLabel("执行结果")).toContainText("恢复本次麦克风状态");
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
  await expect(preview(page).getByLabel("执行结果")).toContainText("麦克风静音");
  expect(await page.evaluate(() => window.__uiFixture.microphones.current.endpoints.map(endpoint => endpoint.muted))).toEqual([true, false]);
});

test("启动选择共用完整麦克风预览，确认只保存选择", async ({ page }) => {
  await openWorkspace(page, "自动策略");
  const panel = page.getByRole("region", { name: "启动档案", exact: true });
  await panel.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  await panel.getByText("查看启动档案参数", { exact: true }).click();
  await expect(panel.getByRole("region", { name: "档案麦克风", exact: true })).toContainText("USB麦克风未静音");
  await panel.getByRole("button", { name: "保存", exact: true }).click();
  await panel.getByRole("button", { name: "启动时应用「隔离档案」？", exact: true }).click();
  const calls = await page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "set_startup_profile"));
  expect(calls[0].args.expected.settings.microphones.endpoints.map(endpoint => endpoint.muted)).toEqual([true, false]);
  expect(await page.evaluate(() => window.__uiFixture.microphones.initial)).toBeNull();
});
