import { availableAction, previewConfiguration, submitConfiguration } from "./configuration-review.mjs";
import { openPowerArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "网络驱动策略", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_lan_policy|restore_lan_policy)$/.test(call.command)));
async function select(page, label, option) {
  await panel(page).getByRole("combobox", { name: label, exact: true }).click(); await page.getByRole("option", { name: option, exact: true }).click();
}
const wake = page => panel(page).getByRole("button", { name: "关机唤醒补丁", exact: true });
const power = page => panel(page).getByRole("combobox", { name: "网卡节能", exact: true });
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const string = text => ({ kind: 1, bytes: [...text, "\0"].flatMap(letter => [letter.charCodeAt(0), 0]) });
    const device = number => ({ identity: { instance: `PCI\\ISOLATED\\${number}`, driver: `{4d36e972-e325-11ce-bfc1-08002be10318}\\000${number}`,
      netCfgId: string(`isolated-${number}`), driverVersion: string("1"), matchingId: null }, name: `隔离网卡 ${number}`,
      values: { powerSavingMode: string("0"), pnpCapabilities: null, wakeOnLink: null, s5WakeOnLan: null, shutdownWake: null, wakeOnSlot: string("1") } });
    window.__uiFixture.lan = { revision: "lan-0", frame: { capabilities: { powerSaving: true, wakePatch: true }, recommendation: true,
      devices: [device(1), device(2)] }, initial: [], lastError: null, blockedReason: null };
  });
  await page.goto("/"); await openPowerArea(page, "电源设置");
});

test("网卡节能直接提交，组合唤醒补丁独立预览及恢复", async ({ page }) => {
  const region = panel(page);
  await region.getByRole("button", { name: "使用原厂建议", exact: true }).click();
  await expect(power(page)).toHaveText("开启网卡节能");
  await wake(page).click();
  const review = await previewConfiguration(page, "关机唤醒补丁");
  await expect(review.locator(".ccc-fact-row")).toHaveCount(2);
  expect(await writes(page)).toHaveLength(1);
  await submitConfiguration(review);
  const calls = await writes(page);
  expect(calls.map(call => call.args.request)).toMatchObject([
    { deviceId: "PCI\\ISOLATED\\1", powerSaving: true, applyWakePatch: false },
    { deviceId: "PCI\\ISOLATED\\1", powerSaving: null, applyWakePatch: true },
  ]);
  expect(await page.evaluate(() => window.__uiFixture.lan.frame.devices[1].values.pnpCapabilities)).toBeNull();
  const restore = await previewConfiguration(page, "网络驱动策略", true);
  await submitConfiguration(restore, "确认恢复");
  await expect(power(page)).toHaveText("关闭网卡节能");
  expect(await page.evaluate(() => window.__uiFixture.lan.frame.devices[0].values.pnpCapabilities)).toBeNull();
});

test("切换网卡只更换读取目标并清除旧补丁预览", async ({ page }) => {
  await wake(page).click();
  await previewConfiguration(page, "关机唤醒补丁");
  await select(page, "网络策略设备", "隔离网卡 2");
  await expect(power(page)).toHaveText("关闭网卡节能");
  await expect(availableAction(page, "关机唤醒补丁")).toHaveCount(0);
  expect(await writes(page)).toHaveLength(0);
});

test("读失败撤销快照，过时驱动修订拒绝直接写入", async ({ page }) => {
  const region = panel(page);
  await expect(power(page)).toBeEnabled();
  await page.evaluate(() => { window.__uiFixture.lan.revision = "external"; });
  await select(page, "网卡节能", "开启网卡节能");
  await expect(region.getByRole("alert")).toContainText("驱动或恢复记录已变化");
  await page.evaluate(() => { window.__uiFixture.lanReadFault = "隔离LAN读取失败"; });
  await region.getByRole("button", { name: "重新读取网络策略", exact: true }).click();
  await expect(region.getByRole("alert")).toContainText("隔离LAN读取失败");
  await expect(power(page)).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.lanReadFault = null; });
  await region.getByRole("button", { name: "重新读取网络策略", exact: true }).click();
  await expect(power(page)).toHaveText("关闭网卡节能");
  expect(await writes(page)).toHaveLength(0);
});

test("权威预检失败不请求系统授权，重试成功才提交", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.lanPreviewFault = "隔离驱动字段预览失败"; });
  await select(page, "网卡节能", "开启网卡节能");
  await expect(panel(page).getByRole("alert")).toContainText("隔离驱动字段预览失败");
  expect(await writes(page)).toHaveLength(0);
  await page.evaluate(() => { window.__uiFixture.lanPreviewFault = null; });
  await select(page, "网卡节能", "开启网卡节能");
  await expect(power(page)).toHaveText("开启网卡节能");
  expect(await writes(page)).toHaveLength(1);
});

test("部分失败保留原值，驱动更换后拒绝恢复", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.lanFault = "隔离写失败；补偿已尝试"; });
  await select(page, "网卡节能", "开启网卡节能");
  await expect(panel(page).getByRole("alert")).toContainText("隔离写失败");
  await expect(availableAction(page, "网络驱动策略", true)).toBeEnabled();
  await page.evaluate(() => { const lan = window.__uiFixture.lan; lan.frame.devices[0].identity.driverVersion = { kind: 1, bytes: [50, 0, 0, 0] }; lan.revision = "driver-update"; });
  await panel(page).getByRole("button", { name: "重新读取网络策略", exact: true }).click();
  await expect(panel(page)).toContainText("网卡或驱动已更换");
  await expect(availableAction(page, "网络驱动策略", true)).toHaveCount(0);
  await expect(power(page)).toBeDisabled();
});

test("未知字段禁用节能，不适用补丁和无能力模块隐藏", async ({ page }) => {
  await expect(power(page)).toBeEnabled();
  await page.evaluate(() => { const lan = window.__uiFixture.lan; lan.frame.devices[0].values.powerSavingMode = { kind: 4, bytes: [1, 0, 0, 0] }; lan.frame.devices[0].values.wakeOnSlot = null; lan.revision = "unknown"; });
  await panel(page).getByRole("button", { name: "重新读取网络策略", exact: true }).click();
  await expect(power(page)).toBeDisabled();
  await expect(wake(page)).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.lan = null; });
  await panel(page).getByRole("button", { name: "重新读取网络策略", exact: true }).click();
  await expect(panel(page)).toHaveCount(0);
  expect(await writes(page)).toHaveLength(0);
});

test("软件风扇互斥时禁用可能请求授权的操作", async ({ page }) => {
  const region = panel(page);
  await expect(power(page)).toBeEnabled();
  await page.evaluate(() => { window.__uiFixture.lan.blockedReason = "请先退出软件风扇曲线，再请求网络策略系统授权"; });
  await region.getByRole("button", { name: "重新读取网络策略", exact: true }).click();
  await expect(region).toContainText("请先退出软件风扇曲线");
  await expect(power(page)).toBeDisabled();
  await expect(wake(page)).toBeDisabled();
  expect(await writes(page)).toHaveLength(0);
});

test("宽窄窗口均可读取网卡控件和补丁预览", async ({ page }) => {
  const region = panel(page);
  await wake(page).click();
  await previewConfiguration(page, "关机唤醒补丁");
  for (const width of [360, 760, 1440]) {
    await page.setViewportSize({ width, height: 1800 }); await region.scrollIntoViewIfNeeded();
    expect(await region.evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true);
  }
  await region.screenshot({ path: "target/ui-check/lan-policy.png", animations: "disabled" });
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
