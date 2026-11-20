import { openProfileResults, openProfileSave, openStartupParameters, saveProfileButton, showProfileMenu, showProfileSettings } from "./profile-review.mjs";
import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openWorkspace, openPowerArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "档案预览", exact: true });
const card = page => panel(page).getByRole("region", { name: "档案逐键灯效", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply_|save_|import_|set_startup)/.test(c.command)));
async function select(page) {
  await openPowerArea(page, "配置档案");
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await showProfileSettings(page);
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const data = window.__uiFixture;
    data.family = "perkey"; data.keyboard.detectedType = 3;
    data.lightingOptions.capabilities.keyboardType = 3;
    data.perkey.current.enabled = false;
    data.perkey.current.pattern.colors = Array.from({ length: 120 }, (_, id) => ({ r: id * 2, g: 80, b: 160 }));
    data.profile.settings.perkeyLighting = { keyboardType: 3, device: structuredClone(data.perkey.device), values: structuredClone(data.perkey.current) };
  });
  await page.goto("/"); await openWorkspace(page, "键盘与灯光");
});

test("档案逐键键帽只读展示关闭与保留档位，确认携带完整布局和矩阵", async ({ page }) => {
  await select(page);
  await expect(card(page)).toContainText("逐键静态");
  await expect(card(page)).toContainText("保留亮度");
  await expect(card(page)).toContainText("3 级");
  await expect(card(page).getByRole("img", { name: "档案逐键键帽配置" })).toBeVisible();
  expect(await card(page).locator("svg title").count()).toBeGreaterThan(80);
  await expect(card(page).locator("button, input")).toHaveCount(0);
  expect(await writes(page)).toEqual([]);
  await previewConfiguration(page);
  expect(await writes(page)).toEqual([]);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  const calls = await writes(page); expect(calls).toHaveLength(1);
  expect(calls[0].args.expected.settings.perkeyLighting).toMatchObject({ keyboardType: 3, device: { compact: false, physicalThreeColumn: false }, values: { enabled: false, brightness: 3 } });
  expect(calls[0].args.expected.settings.perkeyLighting.values.pattern.colors).toHaveLength(120);
  const results = await openProfileResults(page);
  await expect(results).toContainText("逐键灯效");
});

test("捕获和导出保留逐键颜色与型号，不混入普通键盘灯或启动监听", async ({ page }) => {
  await select(page);
  await openProfileSave(page, "逐键完整档案", ["灯光"]);
  await saveProfileButton(page).click();
  await showProfileSettings(page);
  await expect(card(page)).toContainText("保留亮度");
  const download = page.waitForEvent("download");
  await (await showProfileMenu(page)).getByRole("button", { name: "导出档案", exact: true }).click();
  const file = JSON.parse(await readFile(await (await download).path(), "utf8"));
  expect(file.settings.keyboardLighting).toBeNull();
  expect(file.settings.perkeyLighting.values.pattern.colors[119]).toEqual({ r: 238, g: 80, b: 160 });
  expect(file.settings.perkeyLighting.values.enabled).toBe(false);
  expect((await writes(page)).map(c => c.command)).toEqual(["save_profile"]);
});

test("动态导入区分应用后响应意图，未知布局显示矩阵", async ({ page }) => {
  await select(page);
  const file = await page.evaluate(() => {
    const file = structuredClone(window.__uiFixture.profile);
    file.name = "逐键涟漪"; file.settings.perkeyLighting.device.compact = null;
    file.settings.perkeyLighting.values = { enabled: true, brightness: 4, speed: 2, pattern: { kind: "ripple", reactive: true, color: { r: 255, g: 60, b: 10 } } };
    return file;
  });
  await page.locator('input[type="file"]').setInputFiles({ name: "逐键.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(file)) });
  await showProfileSettings(page);
  await expect(card(page)).toContainText("全部设置应用成功后启用按键响应");
  await expect(card(page)).toContainText("布局未识别");
  await expect(card(page).getByRole("img", { name: "档案逐键灯位矩阵" })).toBeVisible();
  await expect(card(page).locator('rect[fill="#ff3c0a"]')).toHaveCount(120);
  await expect(card(page)).not.toContainText("正在运行");
  expect(await writes(page)).toEqual([]);
});

test("启动选择共享逐键卡片，确认只保存选择不应用灯效", async ({ page }) => {
  await openPowerArea(page, "电源设置");
  const startup = page.getByRole("region", { name: "启动档案", exact: true });
  await startup.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  const parameters = await openStartupParameters(page);
  await expect(parameters.getByLabel("档案逐键灯效")).toContainText("保留亮度");
  await page.keyboard.press("Escape");
  await configurationView(page).getByRole("button", { name: "授权启动应用档案", exact: true }).click();
  const calls = await writes(page);
  expect(calls.map(c => c.command)).toEqual(["set_startup_profile"]);
  expect(calls[0].args.expected.settings.perkeyLighting.values.pattern.colors).toHaveLength(120);
});

test("旧逐键预览被拒绝，补偿和会话恢复失败保留具体结果", async ({ page }) => {
  await select(page);
  await previewConfiguration(page);
  await page.evaluate(() => { window.__uiFixture.profile.settings.perkeyLighting.values.speed = 2; });
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("档案已变化");
  await page.evaluate(() => { window.__uiFixture.profileSteps = [{ step: "GPU 偏移", ok: false, message: "隔离后续失败" },
    { step: "恢复逐键灯效", ok: false, message: "设备身份变化，首次快照保留" }, { step: "恢复逐键按键响应", ok: false, message: "前态未恢复，保持暂停" }]; });
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await showProfileSettings(page);
  await previewConfiguration(page);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("部分设置恢复失败");
  const results = await openProfileResults(page);
  await expect(results).toContainText("保持暂停");
});

test("逐键状态未知阻止捕获，不生成默认灯效", async ({ page }) => {
  await select(page);
  await page.evaluate(() => { window.__uiFixture.perkey.current = null; });
  await openProfileSave(page, "未知逐键", ["灯光"]);
  await expect(saveProfileButton(page)).toBeDisabled();
  await expect(page.getByRole("alert")).toContainText("逐键状态未知");
  expect(await writes(page)).toEqual([]);
});

test("键帽卡片在宽窄深浅布局保留模块边框与间隔", async ({ page }) => {
  await select(page);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 1600 });
      await card(page).scrollIntoViewIfNeeded();
      expect(await card(page).evaluate(el => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
      await expect(card(page)).toHaveAttribute("data-surface", "group");
      await card(page).screenshot({ path: `target/ui-check/profile-perkey-${theme}-${width}.png` });
    }
  }
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
