import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { installMultiZoneFixture } from "./multizone-fixture.mjs";

const panel = page => page.getByRole("region", { name: "档案预览", exact: true });
const card = page => panel(page).getByLabel("档案多区键盘", { exact: true });
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
  await page.addInitScript(installMultiZoneFixture);
  await page.addInitScript(() => {
    const data = window.__uiFixture;
    data.profile.settings.multiZoneLighting = structuredClone({ capabilities: data.multiZone.capabilities, values: data.multiZone.current });
    data.profile.settings.multiZoneLighting.values.mode = 4;
  });
  await page.goto("/"); await openWorkspace(page, "灯光效果");
});
test("档案完整预览、确认及后续失败恢复", async ({ page }) => {
  await select(page);
  await expect(card(page)).toContainText("波浪");
  await expect(card(page).getByRole("meter")).toHaveCount(0);
  await page.evaluate(() => window.__uiFixture.profileSteps = [{ step: "GPU 偏移", ok: false, message: "隔离后续故障" }]);
  await apply(page);
  await expect(panel(page).getByLabel("执行结果")).toContainText("恢复多区键盘灯效");
  expect(await page.evaluate(() => window.__uiFixture.multiZone.current.mode)).toBe(1);
});
test("捕获导出保留四区及能力，普通和逐键目标为空", async ({ page }) => {
  await select(page);
  await page.getByPlaceholder("游戏 / 静音").fill("当前多区");
  await page.getByRole("button", { name: "保存当前", exact: true }).click();
  await page.getByRole("button", { name: "保存当前设置？", exact: true }).click();
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
  await openWorkspace(page, "自动策略");
  const startup = page.getByRole("region", { name: "启动档案", exact: true });
  await startup.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  await startup.locator("summary").filter({ hasText: "查看启动档案参数" }).click();
  await expect(startup.getByLabel("档案多区键盘")).toContainText("波浪");
  await startup.getByRole("button", { name: "保存", exact: true }).click();
  await startup.getByRole("button", { name: "启动时应用「隔离档案」？", exact: true }).click();
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
