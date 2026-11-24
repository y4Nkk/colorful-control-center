import { openPowerArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const title = "USB 选择性暂停";
const ac = `插电时的 ${title}`;
const dc = `电池供电时的 ${title}`;
const panel = page => page.getByRole("region", { name: title, exact: true });
const field = (page, name) => panel(page).getByRole("combobox", { name, exact: true });
const reload = page => panel(page).getByRole("button", { name: `重新读取 ${title}`, exact: true }).click();
const recommended = page => panel(page).getByRole("button", { name: "使用原厂建议 · 开启", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_usb_policy|restore_usb_policy)$/.test(call.command)));
async function select(page, name, option) {
  await field(page, name).click();
  await page.getByRole("option", { name: option, exact: true }).click();
}
async function openRestore(page) {
  const name = `恢复${title}`;
  await panel(page).getByRole("button", { name, exact: true }).click();
  return page.getByRole("dialog", { name, exact: true });
}
async function restore(page) {
  const dialog = await openRestore(page);
  await dialog.getByRole("button", { name: "确认恢复", exact: true }).click();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    window.__uiFixture.usb = { revision: "usb-0", recommendation: true, supported: true, activePlan: null,
      plans: ["balanced", "highPerformance", "powerSaver"].map(id => ({ id, ac: false, dc: false })),
      missing: [], initial: [], blockedReason: null, restoreBlockedReason: null, warnings: ["隔离系统计划"] };
  });
  await page.goto("/"); await openPowerArea(page, "电源设置");
});

test("原厂建议应用六个字段，恢复前预览初始值", async ({ page }) => {
  const region = panel(page);
  await expect(region).toContainText("活动计划 · 自定义计划");
  expect(await writes(page)).toHaveLength(0);
  await recommended(page).click();
  await expect(field(page, ac)).toHaveText("开启选择性暂停");
  const calls = await writes(page);
  expect(calls).toHaveLength(1);
  expect(calls[0].args.request.values).toEqual({ ac: true, dc: true });
  const baseline = await openRestore(page);
  await expect(baseline.locator(".ccc-fact-row")).toHaveCount(3);
  expect(await writes(page)).toHaveLength(1);
  await baseline.getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(field(page, ac)).toHaveText("关闭选择性暂停");
  expect(await page.evaluate(() => window.__uiFixture.usb.plans.every(value => !value.ac && !value.dc))).toBe(true);
});

test("只修改AC时，初始恢复保留后来变化的DC及活动计划", async ({ page }) => {
  await select(page, ac, "开启选择性暂停");
  await expect(field(page, ac)).toHaveText("开启选择性暂停");
  await page.evaluate(() => {
    const usb = window.__uiFixture.usb; usb.plans.forEach(value => { value.dc = true; });
    usb.activePlan = "highPerformance"; usb.revision = "external";
  });
  await reload(page);
  await expect(panel(page)).toContainText("活动计划 · 高性能");
  const recovery = await openRestore(page);
  await expect(recovery).toContainText("电池 · 本次未修改");
  await recovery.getByRole("button", { name: "确认恢复", exact: true }).click();
  expect(await page.evaluate(() => window.__uiFixture.usb.plans.every(value => !value.ac && value.dc))).toBe(true);
  await expect(panel(page)).toContainText("活动计划 · 高性能");
});

test("旧修订拒绝写入，读取失败期间收起控制", async ({ page }) => {
  const region = panel(page);
  await expect(field(page, ac)).toHaveText("关闭选择性暂停");
  await page.evaluate(() => { window.__uiFixture.usb.revision = "external"; });
  await select(page, ac, "开启选择性暂停");
  await expect(region.getByRole("alert")).toContainText("请重新读取后重试");
  expect(await page.evaluate(() => window.__uiFixture.usb.plans.every(plan => !plan.ac && !plan.dc))).toBe(true);
  await page.evaluate(() => { window.__uiFixture.usbReadFault = "隔离USB读取失败"; });
  await reload(page);
  await expect(region.getByRole("alert")).toContainText("隔离USB读取失败");
  await expect(field(page, ac)).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.usbReadFault = null; });
  await reload(page);
  await expect(field(page, ac)).toHaveText("关闭选择性暂停");
  expect(await writes(page)).toHaveLength(1);
});

test("未知字段只能保留，校准期间禁止修改", async ({ page }) => {
  await page.evaluate(() => {
    const usb = window.__uiFixture.usb;
    usb.plans = usb.plans.filter(value => value.id !== "powerSaver");
    usb.plans[0].ac = null; usb.revision = "unknown";
  });
  await reload(page);
  await expect(field(page, ac)).toBeDisabled();
  await select(page, dc, "开启选择性暂停");
  await expect(field(page, dc)).toHaveText("开启选择性暂停");
  expect((await writes(page))[0].args.request.values).toEqual({ ac: null, dc: true });
  await page.evaluate(() => { window.__uiFixture.calibration.recoveryRequired = true; });
  await reload(page);
  await expect(panel(page)).toContainText("电池校准期间");
  await expect(field(page, dc)).toBeDisabled();
  expect(await writes(page)).toHaveLength(1);
});

test("部分失败保留原值恢复，能力丢失后仍可恢复本次字段", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.usbFault = "隔离部分写入失败；已尝试补偿"; });
  await select(page, ac, "开启选择性暂停");
  await expect(panel(page).getByRole("alert")).toContainText("隔离部分写入失败");
  await page.evaluate(() => {
    const usb = window.__uiFixture.usb; usb.supported = false; usb.blockedReason = "固件未声明 USB 选择性暂停，只能恢复";
    usb.plans.forEach(value => { value.ac = true; }); usb.revision = "lost";
  });
  await reload(page);
  await expect(field(page, ac)).toBeDisabled();
  await restore(page);
  expect(await page.evaluate(() => window.__uiFixture.usb.plans.every(value => value.ac === false))).toBe(true);
});

test("无能力隐藏且不触发任何系统策略写入", async ({ page }) => {
  await expect(panel(page)).toContainText("活动计划 · 自定义计划");
  await page.evaluate(() => { window.__uiFixture.usb = null; });
  await reload(page);
  await expect(panel(page)).toHaveCount(0); expect(await writes(page)).toHaveLength(0);
});

test("恢复被阻止的原因显示在卡内", async ({ page }) => {
  const region = panel(page);
  await page.evaluate(() => { window.__uiFixture.usb.restoreBlockedReason = "隔离初始计划已不存在，不能恢复"; });
  await reload(page);
  await expect(region).toContainText("隔离初始计划已不存在，不能恢复");
  await expect(region.getByRole("button", { name: `恢复${title}`, exact: true })).toHaveCount(0);
  expect(await writes(page)).toHaveLength(0);
});

test("供电控件在宽窄窗口均可读", async ({ page }) => {
  const region = panel(page);
  for (const width of [360, 760, 1440]) {
    await page.setViewportSize({ width, height: 1800 }); await region.scrollIntoViewIfNeeded();
    await expect(field(page, ac)).toBeVisible();
    await expect(field(page, dc)).toBeVisible();
    expect(await region.evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true);
  }
  await region.screenshot({ path: "target/ui-check/usb-policy.png", animations: "disabled" });
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
