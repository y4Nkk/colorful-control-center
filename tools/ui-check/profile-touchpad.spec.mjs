import { openProfileResults, openProfileSave, openStartupParameters, saveProfileButton, showProfileMenu, showProfileSettings } from "./profile-review.mjs";
import { openWorkspace, openPowerArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { installTouchpadFixture } from "./touchpad-fixture.mjs";
const preview = page => page.getByRole("region", { name: "档案预览", exact: true });
const navigate = (page, title) => page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: title, exact: true }).click();
async function select(page) {
  await openPowerArea(page, "配置档案");
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await showProfileSettings(page);
}
async function apply(page) {
  await preview(page).getByRole("button", { name: "应用档案", exact: true }).click();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.addInitScript(installTouchpadFixture); await page.goto("/"); await openWorkspace(page, "设备配置");
});
test("档案预览分别展示目标与保存偏好，确认后可在控制台直接切回", async ({ page }) => {
  await select(page);
  const card = preview(page).getByRole("region", { name: "档案触摸板", exact: true });
  await expect(card).toContainText("系统目标 · 关闭"); await expect(card).toContainText("保存偏好 · 启用");
  await apply(page);
  const results = await openProfileResults(page);
  await expect(results).toContainText("触摸板开关");
  await page.keyboard.press("Escape");
  await navigate(page, "设备配置");
  const panel = page.getByRole("region", { name: "触摸板", exact: true });
  const current = panel.getByLabel("当前触摸板状态", { exact: true });
  await expect(current).toContainText("已关闭");
  await expect(panel.getByRole("button", { name: "恢复初始设置", exact: true })).toHaveCount(0);
  await panel.getByRole("radio", { name: "启用", exact: true }).click();
  await expect(panel.getByRole("radio", { name: "启用", exact: true })).toBeChecked();
  expect(await page.evaluate(() => window.__uiFixture.touchpad.current.enabled)).toBe(true);
});
test("捕获导出导入保留设备身份、系统值和独立保存值", async ({ page }) => {
  await openPowerArea(page, "配置档案");
  await openProfileSave(page, "完整触摸板", ["设备功能"]);
  await saveProfileButton(page).click();
  const pending = page.waitForEvent("download"); await (await showProfileMenu(page)).getByRole("button", { name: "导出档案", exact: true }).click();
  const file = JSON.parse(await readFile(await (await pending).path(), "utf8"));
  expect(file.settings.touchpad).toEqual({ devices: ["ACPI\\I2C0001\\0"], enabled: true, savedEnabled: false });
  file.name = "导入触摸板"; file.settings.touchpad.enabled = false;
  await page.locator('input[type="file"]').setInputFiles({ name: "触摸板.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(file)) });
  await showProfileSettings(page);
  await expect(preview(page).getByRole("region", { name: "档案触摸板", exact: true })).toContainText("系统目标 · 关闭");
  expect(await page.evaluate(() => window.__uiFixture.touchpad.current.enabled)).toBe(true);
});
test("后续独显失败恢复触摸板本次前态", async ({ page }) => {
  await page.evaluate(() => {
    const data = window.__uiFixture, device = data.gpuPower.devices[0];
    data.profile.settings.gpuPower = { id: device.id, name: device.name, values: { enabled: true, automaticPowerOff: !device.automaticPowerOff } };
    data.gpuPowerFault = "隔离后续独显失败";
  });
  await select(page); await apply(page);
  const results = await openProfileResults(page);
  await expect(results).toContainText("恢复本次触摸板状态");
  expect(await page.evaluate(() => window.__uiFixture.touchpad.current)).toMatchObject({ enabled: true, saved: 0 });
});
test("启动档案预览含触摸板目标，保存选择不立即执行", async ({ page }) => {
  await openPowerArea(page, "电源设置");
  const panel = page.getByRole("region", { name: "启动档案", exact: true });
  await panel.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  const parameters = await openStartupParameters(page);
  await expect(parameters.getByRole("region", { name: "档案触摸板", exact: true })).toContainText("系统目标 · 关闭");
  await page.keyboard.press("Escape");
  await panel.getByRole("button", { name: "授权启动应用档案", exact: true }).click();
  const calls = await page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "set_startup_profile"));
  expect(calls[0].args.expected.settings.touchpad).toMatchObject({ enabled: false, savedEnabled: true });
});

test("显示试用取消不改触摸板，明确保留后才执行", async ({ page }) => {
  await page.evaluate(() => { const d = window.__uiFixture; d.profile.settings.display = { displays: [{ ...d.display.displays[0].current, frequency: 144 }] }; });
  await select(page); await apply(page);
  await expect(page.getByRole("button", { name: "保留显示并继续档案", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "恢复本次显示并停止", exact: true }).click();
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "apply_touchpad"))).toHaveLength(0);
  await apply(page);
  await page.getByRole("button", { name: "保留显示并继续档案", exact: true }).click();
  const results = await openProfileResults(page);
  await expect(results).toContainText("触摸板开关");
  expect(await page.evaluate(() => window.__uiFixture.touchpad.current.enabled)).toBe(false);
});
