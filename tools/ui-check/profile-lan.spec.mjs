import { openStartupParameters, openProfileResults } from "./profile-review.mjs";
import { configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { readFile } from "node:fs/promises";
const panel = page => page.getByRole("region", { name: "档案预览", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|save_|import_|set_startup)/.test(call.command)));
async function selectProfile(page) {
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: "性能档案", exact: true }).click();
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
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
  await page.goto("/"); await openWorkspace(page, "电池与节能");
});

test("逐设备预览使用语义与缺失状态，两次确认提交完整目标", async ({ page }) => {
  await selectProfile(page);
  const card = panel(page).getByRole("region", { name: "档案网络策略 · 隔离网卡 1", exact: true });
  await expect(card).toContainText("需系统授权");
  await expect(card).toContainText("恢复为未设置");
  await expect(card).toContainText("开启");
  expect(await writes(page)).toEqual([]);
  await openConfigurationReview(page);
  expect(await writes(page)).toEqual([]);
  await configurationDialog(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await panel(page).getByText(/^执行明细/).click();
  const results = await openProfileResults(page);
  await expect(results.getByRole("listitem").filter({ hasText: "网络驱动策略" }).getByRole("img", { name: "成功", exact: true })).toBeVisible();
  const calls = await writes(page);
  expect(calls.map(call => call.command)).toEqual(["apply_profile"]);
  expect(calls[0].args.expected.settings.lanPolicy.devices).toHaveLength(2);
  expect(calls[0].args.expected.settings.lanPolicy.devices[0].values.wakeOnLink).toBeNull();
});

test("捕获与导出保留当前驱动身份、原始类型及缺失字段", async ({ page }) => {
  await selectProfile(page);
  await page.getByPlaceholder("游戏 / 静音").fill("网络快照");
  await page.getByRole("button", { name: "保存当前", exact: true }).click();
  await page.getByRole("button", { name: "保存当前设置？", exact: true }).click();
  await expect(panel(page).getByRole("heading", { name: "网络快照" })).toBeVisible();
  const saved = (await writes(page)).find(call => call.command === "save_profile");
  expect(saved.args.args.settings.lanPolicy.devices[1].values.pnpCapabilities).toEqual({ kind: 4, bytes: [9, 0, 0, 0] });
  expect(saved.args.args.settings.lanPolicy.devices[0].values.wakeOnLink).toBeNull();
  const download = page.waitForEvent("download");
  await panel(page).getByRole("button", { name: "导出", exact: true }).click();
  const file = await download;
  const exported = JSON.parse(await readFile(await file.path(), "utf8"));
  expect(exported.settings.lanPolicy).toEqual(saved.args.args.settings.lanPolicy);
  expect((await writes(page)).map(call => call.command)).toEqual(["save_profile"]);
});

test("导入只保存档案，字段删除含义与原始配置保持可查", async ({ page }) => {
  await selectProfile(page);
  const file = await page.evaluate(() => ({ ...structuredClone(window.__uiFixture.profile), name: "导入网络" }));
  await page.locator('input[type="file"]').setInputFiles({ name: "网络.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(file)) });
  await expect(panel(page)).toContainText("未设置项会清除现有配置");
  await panel(page).getByRole("button", { name: "确认导入", exact: true }).click();
  await panel(page).getByRole("button", { name: "导入并覆盖同名档案？", exact: true }).click();
  const card = panel(page).getByRole("region", { name: "档案网络策略 · 隔离网卡 2", exact: true });
  await card.getByText("网卡身份与原始配置", { exact: true }).click();
  await expect(card).toContainText("保存的驱动配置");
  await expect(card).toContainText("PCI\\ISOLATED\\2");
  expect((await writes(page)).map(call => call.command)).toEqual(["import_profile"]);
});

test("驱动变化拒绝旧目标，同名文件变化仍需重新确认", async ({ page }) => {
  await selectProfile(page);
  await openConfigurationReview(page);
  await page.evaluate(() => window.__uiFixture.lan.frame.devices[0].identity.driverVersion.bytes = [50, 0, 0, 0]);
  await configurationDialog(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("LAN档案驱动身份已变化");
  expect(await writes(page)).toHaveLength(1);
  await page.evaluate(() => window.__uiFixture.profile.settings.lanPolicy.devices[0].name = "文件已更改");
  await openConfigurationReview(page);
  await configurationDialog(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("档案已变化");
});

test("后续失败和网络补偿失败逐项展示，不报告全部成功", async ({ page }) => {
  await page.evaluate(() => window.__uiFixture.profileSteps = [
    { step: "网络驱动策略", ok: true, message: "已确认" }, { step: "GPU 偏移", ok: false, message: "隔离GPU故障" },
    { step: "恢复网络驱动策略", ok: false, message: "同一授权补偿失败，初始记录保留" }]);
  await selectProfile(page);
  await openConfigurationReview(page);
  await configurationDialog(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("部分设置恢复失败");
  const results = await openProfileResults(page);
  await expect(results).toContainText("初始记录保留");
});

test("启动选择共享网络预览，保存选择不申请网络写入", async ({ page }) => {
  await openWorkspace(page, "自动策略");
  const startup = page.getByRole("region", { name: "启动档案", exact: true });
  await startup.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  const parameters = await openStartupParameters(page);
  await expect(parameters.getByRole("region", { name: "档案网络策略 · 隔离网卡 1", exact: true })).toContainText("需系统授权");
  await page.keyboard.press("Escape");
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  const calls = await writes(page);
  expect(calls.map(call => call.command)).toEqual(["set_startup_profile"]);
  expect(calls[0].args.expected.settings.lanPolicy.devices).toHaveLength(2);
});

test("未知节能值拒绝捕获，无能力时省略网络目标", async ({ page }) => {
  await selectProfile(page);
  await page.evaluate(() => window.__uiFixture.lan.frame.devices[0].values.powerSavingMode = { kind: 4, bytes: [1, 0, 0, 0] });
  await page.getByPlaceholder("游戏 / 静音").fill("未知网络");
  await page.getByRole("button", { name: "保存当前", exact: true }).click();
  await page.getByRole("button", { name: "保存当前设置？", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("LAN节能字段未知");
  expect(await writes(page)).toEqual([]);
  await page.evaluate(() => window.__uiFixture.lan.frame.capabilities = { powerSaving: false, wakePatch: false });
  await page.getByRole("button", { name: "保存当前", exact: true }).click();
  await page.getByRole("button", { name: "保存当前设置？", exact: true }).click();
  await expect(panel(page).getByRole("heading", { name: "未知网络" })).toBeVisible();
  expect((await writes(page))[0].args.args.settings.lanPolicy).toBeNull();
});

test("逐设备卡片深浅宽窄布局有边框间隔，原始配置展开也不溢出", async ({ page }) => {
  await selectProfile(page);
  const card = panel(page).getByRole("region", { name: "档案网络策略 · 隔离网卡 1", exact: true });
  await card.getByText("网卡身份与原始配置", { exact: true }).click();
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
