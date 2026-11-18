import { availableAction, configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "网络驱动策略", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_lan_policy|restore_lan_policy)$/.test(call.command)));
async function select(page, label, option) {
  await panel(page).getByRole("combobox", { name: label, exact: true }).click(); await page.getByRole("option", { name: option, exact: true }).click();
}
async function apply(page) {
  await openConfigurationReview(page, "网络驱动策略");
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
}
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
  await page.goto("/"); await openWorkspace(page, "电池与节能");
});

test("后端预览三个字段，建议只填草稿，确认后修改所选网卡并恢复缺失值", async ({ page }) => {
  const region = panel(page);
  await region.getByRole("button", { name: "将原厂节能建议填入草稿", exact: true }).click();
  await region.getByRole("checkbox", { name: "应用原厂关机唤醒驱动补丁", exact: true }).check();
  await expect(region.getByLabel("网络策略变更预览").locator(".ccc-fact-row")).toHaveCount(3);
  expect(await writes(page)).toHaveLength(0);
  await openConfigurationReview(page, "网络驱动策略"); expect(await writes(page)).toHaveLength(0);
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  const calls = await writes(page); expect(calls).toHaveLength(1); expect(calls[0].args.request.deviceId).toBe("PCI\\ISOLATED\\1");
  expect(await page.evaluate(() => window.__uiFixture.lan.frame.devices[1].values.pnpCapabilities)).toBeNull();
  await openConfigurationReview(page, "网络驱动策略", true); expect(await writes(page)).toHaveLength(1);
  await configurationDialog(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  expect(await page.evaluate(() => window.__uiFixture.lan.frame.devices[0].values.pnpCapabilities)).toBeNull();
});

test("切换网卡清除草稿和确认，不触碰其他设备", async ({ page }) => {
  await select(page, "网卡节能目标", "开启网卡节能");
  await openConfigurationReview(page, "网络驱动策略");
  await select(page, "网络策略设备", "隔离网卡 2");
  await expect(panel(page).getByRole("combobox", { name: "网卡节能目标", exact: true })).toHaveText("保留当前节能设置");
  await expect(configurationDialog(page).getByRole("button", { name: "确认应用", exact: true })).toHaveCount(0);
  expect(await writes(page)).toHaveLength(0);
});

test("读取失败撤销快照并保留草稿，旧驱动预览不能沿用确认", async ({ page }) => {
  const region = panel(page);
  await select(page, "网卡节能目标", "开启网卡节能");
  await openConfigurationReview(page, "网络驱动策略");
  await page.evaluate(() => { window.__uiFixture.lan.revision = "external"; });
  await region.getByRole("button", { name: "重新读取网络策略", exact: true }).click();
  await expect(configurationDialog(page).getByRole("button", { name: "确认应用", exact: true })).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.lanReadFault = "隔离LAN读取失败"; });
  await region.getByRole("button", { name: "重新读取网络策略", exact: true }).click();
  await expect(region.getByRole("alert")).toContainText("隔离LAN读取失败");
  await expect(availableAction(page, "网络驱动策略")).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.lanReadFault = null; });
  await region.getByRole("button", { name: "重新读取网络策略", exact: true }).click();
  await expect(region.getByRole("combobox", { name: "网卡节能目标", exact: true })).toHaveText("开启网卡节能");
  expect(await writes(page)).toHaveLength(0);
});

test("权威预览失败禁止请求系统授权，恢复后才能继续", async ({ page }) => {
  await expect(panel(page)).toContainText("当前驱动设置");
  await page.evaluate(() => { window.__uiFixture.lanPreviewFault = "隔离驱动字段预览失败"; });
  await select(page, "网卡节能目标", "开启网卡节能");
  await expect(panel(page).getByRole("alert")).toContainText("隔离驱动字段预览失败");
  await expect(availableAction(page, "网络驱动策略")).toHaveCount(0);
  expect(await writes(page)).toHaveLength(0);
  await page.evaluate(() => { window.__uiFixture.lanPreviewFault = null; });
  await panel(page).getByRole("button", { name: "重新读取网络策略", exact: true }).click();
  await expect(availableAction(page, "网络驱动策略")).toBeEnabled();
});

test("部分失败保留逐字段初态，驱动更换阻止恢复且记录可见", async ({ page }) => {
  await select(page, "网卡节能目标", "开启网卡节能");
  await page.evaluate(() => { window.__uiFixture.lanFault = "隔离写失败；补偿已尝试"; }); await apply(page);
  await expect(panel(page).getByRole("alert")).toContainText("隔离写失败");
  await expect(panel(page)).toContainText("初始恢复目标");
  await page.evaluate(() => { const lan = window.__uiFixture.lan; lan.frame.devices[0].identity.driverVersion = { kind: 1, bytes: [50, 0, 0, 0] }; lan.revision = "driver-update"; });
  await panel(page).getByRole("button", { name: "重新读取网络策略", exact: true }).click();
  await expect(panel(page)).toContainText("设备或驱动身份已变化");
  await expect(availableAction(page, "网络驱动策略", true)).toHaveCount(0);
  await expect(availableAction(page, "网络驱动策略")).toHaveCount(0);
});

test("未知字段与不适用补丁不开放编辑，无能力隐藏模块", async ({ page }) => {
  await expect(panel(page)).toContainText("当前驱动设置");
  await page.evaluate(() => { const lan = window.__uiFixture.lan; lan.frame.devices[0].values.powerSavingMode = { kind: 4, bytes: [1, 0, 0, 0] }; lan.frame.devices[0].values.wakeOnSlot = null; lan.revision = "unknown"; });
  await panel(page).getByRole("button", { name: "重新读取网络策略", exact: true }).click();
  await expect(panel(page).getByRole("combobox", { name: "网卡节能目标", exact: true })).toBeDisabled();
  await expect(panel(page).getByRole("checkbox", { name: "应用原厂关机唤醒驱动补丁", exact: true })).toBeDisabled();
  await page.evaluate(() => { window.__uiFixture.lan = null; });
  await panel(page).getByRole("button", { name: "重新读取网络策略", exact: true }).click();
  await expect(panel(page)).toHaveCount(0); expect(await writes(page)).toHaveLength(0);
});

test("运行互斥明确禁止授权，避免等待系统对话框干扰软件风扇控制", async ({ page }) => {
  const region = panel(page);
  await expect(region).toContainText("当前驱动设置");
  await page.evaluate(() => { window.__uiFixture.lan.blockedReason = "请先退出软件风扇曲线，再请求网络策略系统授权"; });
  await region.getByRole("button", { name: "重新读取网络策略", exact: true }).click();
  await select(page, "网卡节能目标", "开启网卡节能");
  await expect(region).toContainText("请先退出软件风扇曲线");
  await expect(availableAction(page, "网络驱动策略")).toHaveCount(0);
  expect(await writes(page)).toHaveLength(0);
});

test("左右卡片有间隔和模块边框，宽窄布局无横向溢出", async ({ page }) => {
  const region = panel(page);
  await region.getByRole("checkbox", { name: "应用原厂关机唤醒驱动补丁", exact: true }).check();
  for (const width of [360, 760, 1440]) {
    await page.setViewportSize({ width, height: 1800 }); await region.scrollIntoViewIfNeeded();
    const boxes = await region.locator(".ccc-lan-section").evaluateAll(nodes => nodes.map(node => { const rect = node.getBoundingClientRect(); return { x: rect.x, right: rect.right, y: rect.y, bottom: rect.bottom, border: parseFloat(getComputedStyle(node).borderLeftWidth) }; }));
    expect(boxes.every(value => value.border > 0)).toBe(true);
    if (width === 1440) expect(boxes[1].x - boxes[0].right).toBeGreaterThanOrEqual(12);
    else expect(boxes[1].y - boxes[0].bottom).toBeGreaterThanOrEqual(12);
    expect(await region.evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true);
  }
  await region.screenshot({ path: "target/ui-check/lan-policy.png", animations: "disabled" });
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
