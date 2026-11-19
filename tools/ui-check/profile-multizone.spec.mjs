import { openProfileResults, openProfileSave, openStartupParameters, saveProfileButton, showProfileSettings } from "./profile-review.mjs";
import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openWorkspace, openSystemArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { installMultiZoneFixture } from "./multizone-fixture.mjs";

const panel = page => page.getByRole("region", { name: "档案预览", exact: true });
const card = page => panel(page).getByLabel("档案多区键盘", { exact: true });
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
  await page.addInitScript(installMultiZoneFixture);
  await page.addInitScript(() => {
    const data = window.__uiFixture;
    data.profile.settings.multiZoneLighting = structuredClone({ capabilities: data.multiZone.capabilities, values: data.multiZone.current });
    data.profile.settings.multiZoneLighting.values.mode = 4;
  });
  await page.goto("/"); await openWorkspace(page, "键盘与灯光");
});
test("档案完整预览、确认及后续失败恢复", async ({ page }) => {
  await select(page);
  await expect(card(page)).toContainText("波浪");
  await expect(card(page).getByRole("meter")).toHaveCount(0);
  await page.evaluate(() => window.__uiFixture.profileSteps = [{ step: "GPU 偏移", ok: false, message: "隔离后续故障" }]);
  await apply(page);
  const results = await openProfileResults(page);
  await expect(results).toContainText("恢复多区键盘灯效");
  expect(await page.evaluate(() => window.__uiFixture.multiZone.current.mode)).toBe(1);
});
test("捕获导出保留四区及能力，普通和逐键目标为空", async ({ page }) => {
  await select(page);
  await openProfileSave(page, "当前多区", ["灯光"]);
  await saveProfileButton(page).click();
  await expect(panel(page).getByRole("heading", { name: "当前多区", exact: true })).toBeVisible();
  const settings = await page.evaluate(() => window.__uiFixture.calls.find(c => c.command === "save_profile").args.args.settings);
  expect(settings.multiZoneLighting.values.mode).toBe(1);
  expect(settings.multiZoneLighting.values.zones).toHaveLength(4);
  expect(settings.multiZoneLighting.capabilities.keyboardType).toBe(23);
  expect(settings.keyboardLighting).toBeNull(); expect(settings.perkeyLighting).toBeNull();
  const exported = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke("export_profile", { name: "当前多区" }));
  expect(JSON.parse(exported).settings.multiZoneLighting).toEqual(settings.multiZoneLighting);
});
test("启动选择共享多区预览，保存选择不执行设置", async ({ page }) => {
  await openWorkspace(page, "电源");
  const startup = page.getByRole("region", { name: "启动档案", exact: true });
  await startup.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  const parameters = await openStartupParameters(page);
  await expect(parameters.getByLabel("档案多区键盘")).toContainText("波浪");
  await page.keyboard.press("Escape");
  await configurationView(page).getByRole("button", { name: "授权启动应用档案", exact: true }).click();
  expect(await page.evaluate(() => window.__uiFixture.multiZone.current.mode)).toBe(1);
});
test("多屏试用保留后才执行多区目标", async ({ page }) => {
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.profile.settings.display = { displays: data.display.displays.map(display => ({ ...display.current, frequency: 144 })) };
  });
  await select(page); await apply(page);
  const host = page.getByRole("region", { name: "档案显示确认", exact: true });
  await expect(host).toBeVisible();
  expect(await page.evaluate(() => window.__uiFixture.multiZone.current.mode)).toBe(1);
  await host.getByRole("button", { name: "保留显示并继续档案", exact: true }).click();
  await expect(host).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.multiZone.current.mode)).toBe(4);
});
