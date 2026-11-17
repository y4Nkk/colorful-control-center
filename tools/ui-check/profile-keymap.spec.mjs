import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "档案预览", exact: true });
const card = page => panel(page).getByRole("region", { name: "档案按键映射与宏", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply_|save_|import_|set_startup|retry_)/.test(c.command)));
async function select(page) {
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: "性能档案", exact: true }).click();
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
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
  await page.goto("/"); await openWorkspace(page, "键盘与输入");
});

test("完整宏预览只读，全部动作可展开且确认携带监听意图", async ({ page }) => {
  await select(page);
  await expect(card(page)).toContainText("不启用程序宏监听，保留宏配置");
  await expect(card(page).getByRole("img", { name: "键盘映射范围" })).toBeVisible();
  await card(page).getByText("键盘 · 查看全部 2 项映射", { exact: true }).click();
  await card(page).getByText("鼠标 · 查看全部 2 项映射", { exact: true }).click();
  await expect(card(page)).toContainText("按下 D"); await expect(card(page)).toContainText("释放 D");
  await expect(card(page)).toContainText("隔离宏内容"); await expect(card(page)).toContainText("F:\\isolated\\helper.exe");
  await expect(card(page).locator("input, button")).toHaveCount(0); expect(await writes(page)).toEqual([]);
  await panel(page).getByRole("button", { name: "应用", exact: true }).click(); expect(await writes(page)).toEqual([]);
  await panel(page).getByRole("button", { name: "应用「隔离档案」？", exact: true }).click();
  const calls = await writes(page); expect(calls).toHaveLength(1);
  expect(calls[0].args.expected.settings.keymap.programsEnabled).toBe(false);
  expect(calls[0].args.expected.settings.keymap.configuration.devices).toHaveLength(2);
  await expect(panel(page).getByLabel("执行结果")).toContainText("提交映射与程序监听");
});

test("捕获只保存已接管设备和当前启用位，完整导出不启动宏", async ({ page }) => {
  await select(page); await page.getByPlaceholder("游戏 / 静音").fill("映射档案");
  await page.getByRole("button", { name: "保存当前", exact: true }).click();
  await page.getByRole("button", { name: "保存当前设置？", exact: true }).click();
  const download = page.waitForEvent("download"); await panel(page).getByRole("button", { name: "导出", exact: true }).click();
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
  await page.getByLabel("导入性能档案", { exact: true }).setInputFiles({ name: "程序宏.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(file)) });
  await expect(card(page)).toContainText("全部设置成功后启用程序宏监听，需保持应用运行");
  expect(await writes(page)).toEqual([]);
});

test("启动档案共享映射目标，保存选择不应用或启用监听", async ({ page }) => {
  await openWorkspace(page, "自动策略");
  const startup = page.getByRole("region", { name: "启动档案", exact: true });
  await startup.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  await startup.getByText("查看启动档案参数", { exact: true }).click();
  await expect(startup.getByLabel("档案按键映射与宏")).toContainText("不启用程序宏监听");
  await startup.getByRole("button", { name: "保存", exact: true }).click();
  await startup.getByRole("button", { name: "启动时应用「隔离档案」？", exact: true }).click();
  const calls = await writes(page); expect(calls.map(c => c.command)).toEqual(["set_startup_profile"]);
  expect(calls[0].args.expected.settings.keymap.programsEnabled).toBe(false);
});

test("旧宏目标确认失效，失败记录与暂停原因按步骤保留", async ({ page }) => {
  await select(page); await panel(page).getByRole("button", { name: "应用", exact: true }).click();
  await page.evaluate(() => { window.__uiFixture.profile.settings.keymap.programsEnabled = true; });
  await panel(page).getByRole("button", { name: "应用「隔离档案」？", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("档案已变化");
  await page.evaluate(() => { window.__uiFixture.profileSteps = [{ step: "按键映射与宏", ok: false, message: "隔离部分写入失败" },
    { step: "恢复按键映射与宏", ok: false, message: "恢复记录待处理" }, { step: "恢复程序宏监听", ok: false, message: "前态未知，保持暂停" }]; });
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await panel(page).getByRole("button", { name: "应用", exact: true }).click();
  await panel(page).getByRole("button", { name: "应用「隔离档案」？", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("部分设置恢复失败");
  await expect(panel(page).getByLabel("执行结果")).toContainText("前态未知，保持暂停");
});

test("缺少来源或待恢复记录拒绝捕获，不生成空映射", async ({ page }) => {
  await select(page); await page.evaluate(() => { window.__uiFixture.keymap.configuration = null; });
  await page.getByPlaceholder("游戏 / 静音").fill("不可捕获");
  await page.getByRole("button", { name: "保存当前", exact: true }).click();
  await page.getByRole("button", { name: "保存当前设置？", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("映射来源不可读"); expect(await writes(page)).toEqual([]);
});

test("映射卡片宽窄深浅布局可展开完整长内容且保持模块边框", async ({ page }) => {
  await select(page); await card(page).getByText("鼠标 · 查看全部 2 项映射", { exact: true }).click();
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 1800 }); await card(page).scrollIntoViewIfNeeded();
      expect(await card(page).evaluate(el => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
      for (const trigger of await card(page).locator("summary").all()) expect(await trigger.evaluate(el => el.clientHeight)).toBeLessThan(80);
      await expect(card(page)).toHaveAttribute("data-surface", "group");
      await card(page).screenshot({ path: `target/ui-check/profile-keymap-${theme}-${width}.png` });
    }
  }
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
