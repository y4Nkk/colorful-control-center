import { openProfileResults, openProfileSave, openStartupParameters, saveProfileButton, showProfileMenu, showProfileSettings } from "./profile-review.mjs";
import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openWorkspace, openSystemArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { installTopCoverFixture } from "./top-cover-fixture.mjs";

const navigate = (page, title) => page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: title, exact: true }).click();
const preview = page => page.getByRole("region", { name: "档案预览", exact: true });
async function select(page) {
  await openSystemArea(page, "配置档案");
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await showProfileSettings(page);
}
async function apply(page) {
  await previewConfiguration(page);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(installTopCoverFixture);
  await page.goto("/"); await openWorkspace(page, "键盘与灯光");
});

test("独立顶盖档案预览只读，确认后更新控制台并复用初始恢复", async ({ page }) => {
  await select(page);
  const card = preview(page).getByLabel("档案灯光策略与外围灯区", { exact: true });
  await expect(card).toContainText("独立顶盖 · 亮度2级 · #ABCDEF");
  await expect(card.getByRole("img", { name: "独立顶盖档案颜色" })).toHaveCSS("background-color", "rgb(171, 205, 239)");
  expect(await page.evaluate(() => window.__uiFixture.lightingOptions.current.topCover.brightness)).toBe(3);
  await apply(page);
  const results = await openProfileResults(page);
  await expect(results).toContainText("灯光策略与外围灯区");
  await page.keyboard.press("Escape");
  await navigate(page, "键盘与灯光");
  const panel = page.getByRole("region", { name: "灯光策略与外围灯区", exact: true });
  await expect(panel.getByRole("slider", { name: "顶盖亮度", exact: true })).toHaveValue("2");
  const restore = await previewConfiguration(page, "灯光策略与外围灯区", true);
  await expect(restore).toContainText(/独立顶盖亮度3级 · #0a141e/i);
  await configurationView(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(panel.getByRole("slider", { name: "顶盖亮度", exact: true })).toHaveValue("3");
});

test("捕获导出与再次导入完整保留顶盖亮度和颜色", async ({ page }) => {
  await openSystemArea(page, "配置档案");
  await openProfileSave(page, "完整顶盖", ["灯光"]);
  await saveProfileButton(page).click();
  await showProfileSettings(page);
  await expect(preview(page)).toContainText("独立顶盖 · 亮度3级 · #0A141E");
  const pending = page.waitForEvent("download");
  await (await showProfileMenu(page)).getByRole("button", { name: "导出档案", exact: true }).click();
  const file = JSON.parse(await readFile(await (await pending).path(), "utf8"));
  expect(file.settings.lightingOptions.values.topCover).toEqual({ brightness: 3, color: { r: 10, g: 20, b: 30 } });
  file.name = "再次导入顶盖"; file.settings.lightingOptions.values.topCover.brightness = 0;
  await page.locator('input[type="file"]').setInputFiles({ name: "顶盖.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(file)) });
  await showProfileSettings(page);
  await expect(preview(page)).toContainText("独立顶盖 · 关闭 · #0A141E");
});

test("后续设备失败恢复本次顶盖前态并保留首次基线", async ({ page }) => {
  await page.evaluate(() => {
    const data = window.__uiFixture, device = data.gpuPower.devices[0];
    data.profile.settings.gpuPower = { id: device.id, name: device.name, values: { enabled: true, automaticPowerOff: !device.automaticPowerOff } };
    data.gpuPowerFault = "隔离后续独显失败";
  });
  await select(page);
  await apply(page);
  const results = await openProfileResults(page);
  await expect(results).toContainText("恢复灯光策略与外围灯区");
  const state = await page.evaluate(() => window.__uiFixture.lightingOptions);
  expect(state.current.topCover.brightness).toBe(3);
  expect(state.initial.topCover.brightness).toBe(3);
  expect(state.canRestore).toBe(true);
});

test("显示试用取消不执行顶盖，其余目标需保留后执行", async ({ page }) => {
  await page.evaluate(() => {
    const data = window.__uiFixture, display = data.display.displays[0].current;
    data.profile.settings.display = { displays: [{ ...display, frequency: 144 }] };
  });
  await select(page);
  await apply(page);
  await expect(page.getByRole("button", { name: "保留显示并继续档案", exact: true })).toBeVisible();
  expect(await page.evaluate(() => window.__uiFixture.lightingOptions.current.topCover.brightness)).toBe(3);
  await page.getByRole("button", { name: "恢复本次显示并停止", exact: true }).click();
  expect(await page.evaluate(() => window.__uiFixture.lightingOptions.current.topCover.brightness)).toBe(3);
  expect(await page.evaluate(() => window.__uiFixture.lightingOptions.initial)).toBeNull();
});

test("启动档案预览和确认绑定完整顶盖目标，保存选择不执行", async ({ page }) => {
  await openWorkspace(page, "电源");
  const panel = page.getByRole("region", { name: "启动档案", exact: true });
  await panel.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  const parameters = await openStartupParameters(page);
  await expect(parameters).toContainText("独立顶盖 · 亮度2级 · #ABCDEF");
  await page.keyboard.press("Escape");
  await configurationView(page).getByRole("button", { name: "授权启动应用档案", exact: true }).click();
  const calls = await page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "set_startup_profile"));
  expect(calls).toHaveLength(1);
  expect(calls[0].args.expected.settings.lightingOptions.values.topCover.brightness).toBe(2);
  expect(await page.evaluate(() => window.__uiFixture.lightingOptions.current.topCover.brightness)).toBe(3);
});
