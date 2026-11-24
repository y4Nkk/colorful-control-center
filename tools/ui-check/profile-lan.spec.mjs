import { openProfileResults, openProfileSave, openStartupParameters, saveProfileButton, showProfileMenu, showProfileSettings } from "./profile-review.mjs";
import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openPowerArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { readFile } from "node:fs/promises";
const panel = page => page.getByRole("region", { name: "档案预览", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|save_|import_|set_startup)/.test(call.command)));
async function selectProfile(page) {
  await openPowerArea(page, "配置档案");
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await showProfileSettings(page);
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const string = text => ({ kind: 1, bytes: [...text, "\0"].flatMap(letter => [letter.charCodeAt(0), 0]) });
    const device = number => ({ identity: { instance: `PCI\\ISOLATED\\${number}`, driver: `{4d36e972-e325-11ce-bfc1-08002be10318}\\000${number}`,
      netCfgId: string(`isolated-${number}`), driverVersion: string("1"), matchingId: null }, name: `隔离网卡 ${number}`,
      values: { powerSavingMode: string("0"), pnpCapabilities: null, wakeOnLink: null, s5WakeOnLan: null, shutdownWake: null, wakeOnSlot: string("1") } });
    const data = window.__uiFixture;
    data.lan = { revision: "lan-0", frame: { capabilities: { powerSaving: true, wakePatch: true }, recommendation: true,
      devices: [device(1), device(2)] }, initial: [], lastError: null, blockedReason: null };
    data.lan.frame.devices[1].values.pnpCapabilities = { kind: 4, bytes: [9, 0, 0, 0] };
    data.profile.settings.lanPolicy = { capabilities: structuredClone(data.lan.frame.capabilities), devices: data.lan.frame.devices.map(device => ({
      identity: structuredClone(device.identity), name: device.name, values: { powerSavingMode: string("1"),
        pnpCapabilities: structuredClone(device.values.pnpCapabilities), wakeOnLink: null } })) };
  });
  await page.goto("/"); await openPowerArea(page, "电源设置");
});

test("逐设备预览使用语义与缺失状态，应用携带完整目标", async ({ page }) => {
  await selectProfile(page);
  const card = panel(page).getByRole("region", { name: "档案网络策略 · 隔离网卡 1", exact: true });
  await expect(card).toContainText("需系统授权");
  await expect(card).toContainText("恢复为未设置");
  await expect(card).toContainText("开启");
  expect(await writes(page)).toEqual([]);
  await previewConfiguration(page);
  expect(await writes(page)).toEqual([]);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  const results = await openProfileResults(page);
  await expect(results.getByRole("listitem").filter({ hasText: "网络驱动策略" }).getByRole("img", { name: "成功", exact: true })).toBeVisible();
  const calls = await writes(page);
  expect(calls.map(call => call.command)).toEqual(["apply_profile"]);
  expect(calls[0].args.expected.settings.lanPolicy.devices).toHaveLength(2);
  expect(calls[0].args.expected.settings.lanPolicy.devices[0].values.wakeOnLink).toBeNull();
});

test("捕获与导出保留当前驱动身份、原始类型及缺失字段", async ({ page }) => {
  await selectProfile(page);
  await openProfileSave(page, "网络快照", ["充电与供电"]);
  await saveProfileButton(page).click();
  await expect(panel(page).getByRole("heading", { name: "网络快照" })).toBeVisible();
  const saved = (await writes(page)).find(call => call.command === "save_profile");
  expect(saved.args.args.settings.lanPolicy.devices[1].values.pnpCapabilities).toEqual({ kind: 4, bytes: [9, 0, 0, 0] });
  expect(saved.args.args.settings.lanPolicy.devices[0].values.wakeOnLink).toBeNull();
  const download = page.waitForEvent("download");
  await (await showProfileMenu(page)).getByRole("button", { name: "导出档案", exact: true }).click();
  const file = await download;
  const exported = JSON.parse(await readFile(await file.path(), "utf8"));
  expect(exported.settings.lanPolicy).toEqual(saved.args.args.settings.lanPolicy);
  expect((await writes(page)).map(call => call.command)).toEqual(["save_profile"]);
});

test("导入只保存档案，字段删除含义保持明确", async ({ page }) => {
  await selectProfile(page);
  const file = await page.evaluate(() => ({ ...structuredClone(window.__uiFixture.profile), name: "导入网络" }));
  await page.locator('input[type="file"]').setInputFiles({ name: "网络.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(file)) });
  await showProfileSettings(page);
  await expect(panel(page)).toContainText("未设置项会清除现有配置");
  await panel(page).getByRole("button", { name: "导入档案", exact: true }).click();
  await expect(panel(page).getByRole("button", { name: "应用档案", exact: true })).toBeVisible();
  await showProfileSettings(page);
  const card = panel(page).getByRole("region", { name: "档案网络策略 · 隔离网卡 2", exact: true });
  await expect(card).toContainText("保存的驱动配置");
  expect((await writes(page)).map(call => call.command)).toEqual(["save_profile"]);
});

test("驱动变化拒绝旧目标，同名文件变化仍需重新确认", async ({ page }) => {
  await selectProfile(page);
  await previewConfiguration(page);
  await page.evaluate(() => window.__uiFixture.lan.frame.devices[0].identity.driverVersion.bytes = [50, 0, 0, 0]);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("LAN档案驱动身份已变化");
  expect(await writes(page)).toHaveLength(1);
  await expect(panel(page).getByRole("button", { name: "应用档案", exact: true })).toHaveCount(0);
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.lan.frame.devices[0].identity.driverVersion = structuredClone(data.profile.settings.lanPolicy.devices[0].identity.driverVersion);
  });
  await panel(page).getByRole("button", { name: "重新预览", exact: true }).click();
  await previewConfiguration(page);
  await page.evaluate(() => window.__uiFixture.profile.settings.lanPolicy.devices[0].name = "文件已更改");
  await previewConfiguration(page);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("档案已变化");
});

test("后续失败和网络补偿失败逐项展示，不报告全部成功", async ({ page }) => {
  await page.evaluate(() => window.__uiFixture.profileSteps = [
    { step: "网络驱动策略", ok: true, message: "已确认" }, { step: "GPU 偏移", ok: false, message: "隔离GPU故障" },
    { step: "恢复网络驱动策略", ok: false, message: "同一授权补偿失败，初始记录保留" }]);
  await selectProfile(page);
  await previewConfiguration(page);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("部分设置恢复失败");
  const results = await openProfileResults(page);
  await expect(results).toContainText("初始记录保留");
});

test("启动选择共享网络预览，保存选择不申请网络写入", async ({ page }) => {
  await openPowerArea(page, "电源设置");
  const startup = page.getByRole("region", { name: "启动档案", exact: true });
  await startup.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  const parameters = await openStartupParameters(page);
  await expect(parameters.getByRole("region", { name: "档案网络策略 · 隔离网卡 1", exact: true })).toContainText("需系统授权");
  await page.keyboard.press("Escape");
  await configurationView(page).getByRole("button", { name: "授权启动应用档案", exact: true }).click();
  const calls = await writes(page);
  expect(calls.map(call => call.command)).toEqual(["set_startup_profile"]);
  expect(calls[0].args.expected.settings.lanPolicy.devices).toHaveLength(2);
});

test("未知节能值拒绝捕获，无能力时省略网络目标", async ({ page }) => {
  await selectProfile(page);
  await page.evaluate(() => window.__uiFixture.lan.frame.devices[0].values.powerSavingMode = { kind: 4, bytes: [1, 0, 0, 0] });
  await openProfileSave(page, "未知网络", ["充电与供电"]);
  await expect(saveProfileButton(page)).toBeDisabled();
  await expect(page.getByRole("dialog").getByRole("alert")).toContainText("LAN节能字段未知");
  expect(await writes(page)).toEqual([]);
  await page.evaluate(() => window.__uiFixture.lan.frame.capabilities = { powerSaving: false, wakePatch: false });
  await page.getByRole("dialog").getByRole("button", { name: "重试读取" }).click();
  await saveProfileButton(page).click();
  await expect(panel(page).getByRole("heading", { name: "未知网络" })).toBeVisible();
  expect((await writes(page))[0].args.args.settings.lanPolicy).toBeNull();
});

test("逐设备卡片深浅宽窄布局有边框间隔", async ({ page }) => {
  await selectProfile(page);
  const card = panel(page).getByRole("region", { name: "档案网络策略 · 隔离网卡 1", exact: true });
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 1440]) {
      await page.setViewportSize({ width, height: 1800 });
      await card.scrollIntoViewIfNeeded();
      await expect(card).toHaveAttribute("data-surface", "group");
      expect(await card.evaluate(el => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
      await card.screenshot({ path: `target/ui-check/profile-lan-${theme}-${width}.png` });
    }
  }
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
