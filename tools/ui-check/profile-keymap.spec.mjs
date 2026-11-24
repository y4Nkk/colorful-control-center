import { openProfileResults, openProfileSave, openStartupParameters, profilePanel, saveProfileButton, showProfileMenu, showProfileSettings } from "./profile-review.mjs";
import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openWorkspace, openPowerArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "档案预览", exact: true });
const card = dialog => dialog.getByRole("region", { name: "档案按键映射与宏", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply_|save_|import_|set_startup|retry_)/.test(c.command)));
async function select(page) {
  await openPowerArea(page, "配置档案");
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await showProfileSettings(page);
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const data = window.__uiFixture;
    const configuration = { devices: [
      { device: "keyboard", enabled: true, bindings: [{ trigger: 30, action: { kind: "disable" } }, { trigger: 31, action: { kind: "keys", steps: [
        { kind: "key", scanCode: 32, pressed: true, delayAfterMs: 20 }, { kind: "key", scanCode: 32, pressed: false, delayAfterMs: 40 }] } }] },
      { device: "mouse", enabled: false, bindings: [{ trigger: 1, action: { kind: "text", text: "隔离宏内容", before: [], after: [] } },
        { trigger: 64, action: { kind: "launch", path: "F:\\isolated\\helper.exe", eventId: 256 } }] },
    ] };
    data.profile.settings.keymap = { configuration: structuredClone(configuration), programsEnabled: false };
    data.keymap = { revision: "keymap-0", configuration, enabledFlags: [0, 1], sourceName: "隔离原配置", baselineName: "首次基线",
      canRestore: true, recoveryRequired: false, blockedReason: null, warnings: [], programsRequired: false, programs: { running: false, error: null } };
    data.keymapOwned = ["keyboard"];
  });
  await page.goto("/"); await openWorkspace(page, "键盘与灯光");
});

test("宏预览只读，确认携带监听意图", async ({ page }) => {
  await select(page);
  const review = await previewConfiguration(page);
  const keymap = card(panel(page));
  await expect(keymap).toContainText("不启用程序宏监听，保留宏配置");
  await expect(keymap.getByRole("img", { name: "键盘映射范围" })).toBeVisible();
  await expect(keymap.getByRole("button")).toHaveCount(0);
  await expect(keymap.locator("input")).toHaveCount(0);
  expect(await writes(page)).toEqual([]);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  const calls = await writes(page); expect(calls).toHaveLength(1);
  expect(calls[0].args.expected.settings.keymap.programsEnabled).toBe(false);
  expect(calls[0].args.expected.settings.keymap.configuration.devices).toHaveLength(2);
  const results = await openProfileResults(page);
  await expect(results).toContainText("提交映射与程序监听");
});

test("捕获只保存已接管设备和当前启用位，完整导出不启动宏", async ({ page }) => {
  await select(page); await openProfileSave(page, "映射档案", ["设备功能"]);
  await saveProfileButton(page).click();
  const download = page.waitForEvent("download"); await (await showProfileMenu(page)).getByRole("button", { name: "导出档案", exact: true }).click();
  const file = JSON.parse(await readFile(await (await download).path(), "utf8"));
  expect(file.settings.keymap.configuration.devices).toHaveLength(1);
  expect(file.settings.keymap.configuration.devices[0]).toMatchObject({ device: "keyboard", enabled: false });
  expect(file.settings.keymap.programsEnabled).toBe(false);
  expect((await writes(page)).map(c => c.command)).toEqual(["save_profile"]);
});

test("程序监听导入明确说明应用后启用，预览不执行程序", async ({ page }) => {
  await select(page);
  const file = await page.evaluate(() => {
    const file = structuredClone(window.__uiFixture.profile); file.name = "程序宏";
    file.settings.keymap.programsEnabled = true; file.settings.keymap.configuration.devices[1].enabled = true; return file;
  });
  await page.getByLabel("导入配置档案", { exact: true }).setInputFiles({ name: "程序宏.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(file)) });
  await showProfileSettings(page);
  await expect(card(profilePanel(page))).toContainText("全部设置成功后启用程序宏监听，需保持应用运行");
  expect(await writes(page)).toEqual([]);
});

test("启动档案共享映射目标，保存选择不应用或启用监听", async ({ page }) => {
  await openPowerArea(page, "电源设置");
  const startup = page.getByRole("region", { name: "启动档案", exact: true });
  await startup.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  const parameters = await openStartupParameters(page);
  await expect(parameters.getByLabel("档案按键映射与宏")).toContainText("不启用程序宏监听");
  await page.keyboard.press("Escape");
  await configurationView(page).getByRole("button", { name: "授权启动应用档案", exact: true }).click();
  const calls = await writes(page); expect(calls.map(c => c.command)).toEqual(["set_startup_profile"]);
  expect(calls[0].args.expected.settings.keymap.programsEnabled).toBe(false);
});

test("旧宏目标确认失效，失败记录与暂停原因按步骤保留", async ({ page }) => {
  await select(page); await previewConfiguration(page);
  await page.evaluate(() => { window.__uiFixture.profile.settings.keymap.programsEnabled = true; });
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("档案已变化");
  await page.evaluate(() => { window.__uiFixture.profileSteps = [{ step: "按键映射与宏", ok: false, message: "隔离部分写入失败" },
    { step: "恢复按键映射与宏", ok: false, message: "恢复记录待处理" }, { step: "恢复程序宏监听", ok: false, message: "前态未知，保持暂停" }]; });
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await showProfileSettings(page);
  await previewConfiguration(page);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("部分设置恢复失败");
  const results = await openProfileResults(page);
  await expect(results).toContainText("前态未知，保持暂停");
});

test("缺少来源或待恢复记录拒绝捕获，不生成空映射", async ({ page }) => {
  await select(page); await page.evaluate(() => { window.__uiFixture.keymap.configuration = null; });
  await openProfileSave(page, "不可捕获", ["设备功能"]);
  await expect(saveProfileButton(page)).toBeDisabled();
  await expect(page.getByRole("alert")).toContainText("映射来源不可读"); expect(await writes(page)).toEqual([]);
});

test("映射卡片宽窄深浅布局保持模块边框且不溢出", async ({ page }) => {
  await select(page);
  const keymap = card(profilePanel(page));
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 960 });
      await keymap.scrollIntoViewIfNeeded();
      expect(await keymap.evaluate(el => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
      await expect(keymap).toHaveAttribute("data-surface", "group");
      await keymap.screenshot({ path: `target/ui-check/profile-keymap-${theme}-${width}.png` });
    }
  }
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
