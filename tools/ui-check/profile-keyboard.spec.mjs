import { openProfileResults, openProfileSave, openStartupParameters, saveProfileButton, showProfileMenu, showProfileSettings } from "./profile-review.mjs";
import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openWorkspace, openSystemArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { readFile } from "node:fs/promises";
const navigate = (page, title) => page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: title, exact: true }).click();
const preview = page => page.getByRole("region", { name: "档案预览", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply_|save_|import_|set_startup)/.test(c.command)));
async function select(page) {
  await openSystemArea(page, "配置档案");
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await showProfileSettings(page);
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const data = window.__uiFixture, color = { r: 20, g: 80, b: 160 };
    data.profile.settings.keyboard = { fnLocked: true, winLocked: false, fnWinSwap: null };
    data.profile.settings.keyboardLighting = { keyboardType: 6, enabled: false, settings: { family: "zoneRgb", brightness: 3, effect: "zones", allColor: color,
      zones: [{ enabled: true, color }, { enabled: false, color: { r: 255, g: 0, b: 0 } }, { enabled: true, color: { r: 0, g: 255, b: 0 } }] } };
    data.profile.settings.lightingOptions = { keyboardType: 6, values: { idleTimer: { enabled: true, seconds: 65 }, bootOverride: true,
      logo: { enabled: true, color: { r: 255, g: 0, b: 0 } }, lightBar: { enabled: false, color: { r: 0, g: 0, b: 255 } } , topCover: null} };
    data.keyboardSettings.current = structuredClone(data.profile.settings.keyboard);
    const light = data.profile.settings.keyboardLighting;
    data.keyboard.current = { brightnessLevel: 3, enabled: light.enabled, rawMode: 1, effect: "zones", allColor: color, zones: structuredClone(light.settings.zones) };
    data.lightingOptions.current = structuredClone(data.profile.settings.lightingOptions.values);
    data.lightingOptions.capabilities = { keyboardType: 6, peripheralSupportKnown: true, bootOverride: true, logo: true, lightBar: true , topCover: false};
  });
  await page.goto("/"); await openWorkspace(page, "设备配置");
});

test("三类键盘卡片只读展示开关与保留亮度，确认携带全部设备目标", async ({ page }) => {
  await select(page);
  const panel = preview(page);
  await expect(panel.getByLabel("档案键盘功能")).toContainText("Fn 锁");
  await expect(panel.getByLabel("档案键盘功能")).not.toContainText("Fn / Win 交换");
  await expect(panel.getByLabel("档案键盘灯效")).toContainText("关闭 · 保留亮度 3 级");
  await expect(panel.getByLabel("档案键盘灯效")).toContainText("分区");
  await expect(panel.getByLabel("档案灯光策略与外围灯区")).toContainText("65 秒后关灯");
  await expect(panel.getByRole("img", { name: "Logo档案颜色", exact: true })).toHaveCSS("background-color", "rgb(255, 0, 0)");
  expect(await writes(page)).toEqual([]);
  await previewConfiguration(page);
  expect(await writes(page)).toEqual([]);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  const calls = await writes(page);
  expect(calls).toHaveLength(1);
  expect(calls[0].args.expected.settings.keyboardLighting.enabled).toBe(false);
  expect(calls[0].args.expected.settings.keyboardLighting.settings.brightness).toBe(3);
  expect(calls[0].args.expected.settings.lightingOptions.values.idleTimer.seconds).toBe(65);
  const results = await openProfileResults(page);
  await expect(results).toContainText("灯光策略与外围灯区");
});

test("保存捕获真实开关和档位，导出完整型号化键盘与外围配置", async ({ page }) => {
  await openSystemArea(page, "配置档案");
  await openProfileSave(page, "完整键盘", ["设备功能", "灯光"]);
  await saveProfileButton(page).click();
  await showProfileSettings(page);
  await expect(preview(page).getByLabel("档案键盘灯效")).toContainText("关闭 · 保留亮度 3 级");
  const download = page.waitForEvent("download");
  await (await showProfileMenu(page)).getByRole("button", { name: "导出档案", exact: true }).click();
  const exported = JSON.parse(await readFile(await (await download).path(), "utf8"));
  expect(exported.settings.keyboard.fnLocked).toBe(true);
  expect(exported.settings.keyboardLighting).toMatchObject({ keyboardType: 6, enabled: false, settings: { brightness: 3 } });
  expect(exported.settings.lightingOptions.values.logo.color).toEqual({ r: 255, g: 0, b: 0 });
});

test("白光导入有独立档位与示意，无RGB色区编辑入口", async ({ page }) => {
  await openSystemArea(page, "配置档案");
  const file = await page.evaluate(() => ({ ...structuredClone(window.__uiFixture.profile), name: "白光档案", settings: {
    ...window.__uiFixture.profile.settings, keyboardLighting: { keyboardType: 1, enabled: true, settings: { family: "white", brightness: 5 } }, lightingOptions: null } }));
  await page.locator('input[type="file"]').setInputFiles({ name: "白光.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(file)) });
  await showProfileSettings(page);
  const card = preview(page).getByLabel("档案键盘灯效");
  await expect(card).toContainText("白光键盘"); await expect(card).toContainText("开启 · 亮度 5 级");
  await expect(card.getByRole("img", { name: "档案白光亮度：5 级" })).toBeVisible();
  await expect(card.locator("button[popovertarget]")).toHaveCount(0);
  expect(await writes(page)).toEqual([]);
});

test("未知保存灯效阻止捕获，不把未识别模式自动补成默认", async ({ page }) => {
  await openSystemArea(page, "配置档案");
  await page.evaluate(() => { window.__uiFixture.keyboard.current.effect = null; });
  await openProfileSave(page, "未知灯效", ["设备功能", "灯光"]);
  await expect(saveProfileButton(page)).toBeDisabled();
  await expect(page.getByRole("alert")).toContainText("保存灯效未识别");
  expect(await writes(page)).toEqual([]);
});

test("同名目标变化拒绝旧确认，失败补偿按设备分别展示", async ({ page }) => {
  await select(page);
  await previewConfiguration(page);
  await page.evaluate(() => { window.__uiFixture.profile.settings.keyboardLighting.settings.brightness = 4; });
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("档案已变化");
  await page.evaluate(() => { window.__uiFixture.profileSteps = [{ step: "键盘灯效", ok: true, message: "已确认" },
    { step: "灯光策略与外围灯区", ok: false, message: "隔离保存失败" }, { step: "恢复键盘灯效", ok: false, message: "型号已变化，基线保留" }]; });
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await showProfileSettings(page);
  await previewConfiguration(page);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("部分设置恢复失败");
  const results = await openProfileResults(page);
  await expect(results).toContainText("型号已变化，基线保留");
});

test("启动档案共用三类卡片，保存选择不执行设备写入", async ({ page }) => {
  await openWorkspace(page, "电源");
  const panel = page.getByRole("region", { name: "启动档案", exact: true });
  await panel.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  const parameters = await openStartupParameters(page);
  await expect(parameters.getByLabel("档案键盘灯效")).toContainText("保留亮度 3 级");
  await page.keyboard.press("Escape");
  await configurationView(page).getByRole("button", { name: "授权启动应用档案", exact: true }).click();
  const calls = await writes(page); expect(calls.map(c => c.command)).toEqual(["set_startup_profile"]);
  expect(calls[0].args.expected.settings.keyboardLighting.enabled).toBe(false);
});

test("键盘与外围卡片左右排列，窄屏回流且边框间隔一致", async ({ page }) => {
  await select(page);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 1600 });
      for (const name of ["档案键盘功能", "档案键盘灯效", "档案灯光策略与外围灯区"]) {
        const card = preview(page).getByRole("region", { name, exact: true });
        await card.scrollIntoViewIfNeeded();
        expect(await card.evaluate(el => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
        await expect(card).toHaveAttribute("data-surface", "group");
        await card.screenshot({ path: `target/ui-check/profile-${name}-${theme}-${width}.png` });
      }
    }
  }
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
