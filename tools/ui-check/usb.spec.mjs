import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "USB选择性暂停", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_usb_policy|restore_usb_policy)$/.test(call.command)));
async function select(page, name, option) {
  await panel(page).getByRole("combobox", { name, exact: true }).click();
  await page.getByRole("option", { name: option, exact: true }).click();
}
async function apply(page) {
  await panel(page).getByRole("button", { name: "应用USB策略", exact: true }).click();
  await panel(page).getByRole("button", { name: "确认修改这些计划的USB策略？", exact: true }).click();
}
async function restore(page) {
  await panel(page).getByRole("button", { name: "恢复初始USB策略", exact: true }).click();
  await panel(page).getByRole("button", { name: "确认恢复本次修改的USB字段？", exact: true }).click();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    window.__uiFixture.usb = { revision: "usb-0", recommendation: true, supported: true, activePlan: null,
      plans: ["balanced", "highPerformance", "powerSaver"].map(id => ({ id, ac: false, dc: false })),
      missing: [], initial: [], blockedReason: null, restoreBlockedReason: null, warnings: ["隔离系统计划"] };
  });
  await page.goto("/"); await openWorkspace(page, "电池与节能");
});

test("原厂建议只填草稿，六个字段完整预览后才能确认写入及恢复", async ({ page }) => {
  const region = panel(page);
  await expect(region).toContainText("活动计划 · 自定义计划");
  await region.getByRole("button", { name: "将原厂建议填入草稿" }).click();
  await expect(region.getByLabel("USB变更预览").locator(".ccc-fact-row")).toHaveCount(6);
  expect(await writes(page)).toHaveLength(0);
  await region.getByRole("button", { name: "应用USB策略", exact: true }).click();
  expect(await writes(page)).toHaveLength(0);
  await region.getByRole("button", { name: "确认修改这些计划的USB策略？", exact: true }).click();
  const calls = await writes(page); expect(calls).toHaveLength(1);
  expect(calls[0].args.request.values).toEqual({ ac: true, dc: true });
  await expect(region).toContainText("初始恢复目标"); await restore(page);
  await expect(region.getByRole("combobox", { name: "插电USB策略", exact: true })).toHaveText("保留各计划当前值");
  expect(await page.evaluate(() => window.__uiFixture.usb.plans.every(value => !value.ac && !value.dc))).toBe(true);
});

test("只修改AC时，初始恢复保留后来变化的DC及活动计划", async ({ page }) => {
  await select(page, "插电USB策略", "开启选择性暂停"); await apply(page);
  await expect(panel(page)).toContainText("电池 · 本次未修改");
  await page.evaluate(() => {
    const usb = window.__uiFixture.usb; usb.plans.forEach(value => { value.dc = true; });
    usb.activePlan = "highPerformance"; usb.revision = "external";
  });
  await panel(page).getByRole("button", { name: "重新读取USB策略", exact: true }).click();
  await restore(page);
  expect(await page.evaluate(() => window.__uiFixture.usb.plans.every(value => !value.ac && value.dc))).toBe(true);
  await expect(panel(page)).toContainText("活动计划 · 高性能");
});

test("旧确认被撤销，读取失败时撤销执行快照并保留草稿", async ({ page }) => {
  const region = panel(page);
  await select(page, "插电USB策略", "开启选择性暂停");
  await region.getByRole("button", { name: "应用USB策略", exact: true }).click();
  await page.evaluate(() => { window.__uiFixture.usb.revision = "external"; });
  await region.getByRole("button", { name: "重新读取USB策略", exact: true }).click();
  await expect(region.getByRole("button", { name: "确认修改这些计划的USB策略？", exact: true })).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.usbReadFault = "隔离USB读取失败"; });
  await region.getByRole("button", { name: "重新读取USB策略", exact: true }).click();
  await expect(region.getByRole("alert")).toContainText("隔离USB读取失败");
  await expect(region.getByRole("button", { name: "应用USB策略", exact: true })).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.usbReadFault = null; });
  await region.getByRole("button", { name: "重新读取USB策略", exact: true }).click();
  await expect(region.getByRole("combobox", { name: "插电USB策略", exact: true })).toHaveText("开启选择性暂停");
  expect(await writes(page)).toHaveLength(0);
});

test("缺失计划明确列出，未知字段只能保留，校准期间禁止修改", async ({ page }) => {
  await page.evaluate(() => {
    const usb = window.__uiFixture.usb;
    usb.plans = usb.plans.filter(value => value.id !== "powerSaver"); usb.missing = ["powerSaver"];
    usb.plans[0].ac = null; usb.revision = "unknown";
  });
  await panel(page).getByRole("button", { name: "重新读取USB策略", exact: true }).click();
  await expect(panel(page)).toContainText("未安装：节能");
  await select(page, "插电USB策略", "开启选择性暂停");
  await expect(panel(page).getByRole("button", { name: "应用USB策略", exact: true })).toBeDisabled();
  await select(page, "插电USB策略", "保留各计划当前值");
  await select(page, "电池USB策略", "开启选择性暂停");
  await expect(panel(page).getByLabel("USB变更预览").locator(".ccc-fact-row")).toHaveCount(2);
  await page.evaluate(() => { window.__uiFixture.calibration.recoveryRequired = true; });
  await panel(page).getByRole("button", { name: "重新读取USB策略", exact: true }).click();
  await expect(panel(page)).toContainText("电池校准期间");
  await expect(panel(page).getByRole("button", { name: "应用USB策略", exact: true })).toBeDisabled();
  expect(await writes(page)).toHaveLength(0);
});

test("部分失败保留原值恢复，能力丢失后仍可恢复本次字段", async ({ page }) => {
  await select(page, "插电USB策略", "开启选择性暂停");
  await page.evaluate(() => { window.__uiFixture.usbFault = "隔离部分写入失败；已尝试补偿"; });
  await apply(page);
  await expect(panel(page).getByRole("alert")).toContainText("隔离部分写入失败");
  await page.evaluate(() => {
    const usb = window.__uiFixture.usb; usb.supported = false; usb.blockedReason = "固件未声明USB策略，只能恢复";
    usb.plans.forEach(value => { value.ac = true; }); usb.revision = "lost";
  });
  await panel(page).getByRole("button", { name: "重新读取USB策略", exact: true }).click();
  await expect(panel(page).getByRole("button", { name: "应用USB策略", exact: true })).toBeDisabled();
  await restore(page);
  expect(await page.evaluate(() => window.__uiFixture.usb.plans.every(value => value.ac === false))).toBe(true);
});

test("无能力隐藏且不触发任何系统策略写入", async ({ page }) => {
  await expect(panel(page)).toContainText("活动计划 · 自定义计划");
  await page.evaluate(() => { window.__uiFixture.usb = null; });
  await panel(page).getByRole("button", { name: "重新读取USB策略", exact: true }).click();
  await expect(panel(page)).toHaveCount(0); expect(await writes(page)).toHaveLength(0);
});

test("左右模块有边框间隔，窄屏回流且没有横向溢出", async ({ page }) => {
  const region = panel(page);
  await region.getByRole("button", { name: "将原厂建议填入草稿" }).click();
  for (const width of [360, 760, 1440]) {
    await page.setViewportSize({ width, height: 1800 }); await region.scrollIntoViewIfNeeded();
    const boxes = await region.locator(".ccc-usb-section").evaluateAll(nodes => nodes.map(node => {
      const rect = node.getBoundingClientRect(), style = getComputedStyle(node);
      return { x: rect.x, right: rect.right, y: rect.y, bottom: rect.bottom, border: parseFloat(style.borderLeftWidth) };
    }));
    expect(boxes).toHaveLength(2); expect(boxes.every(value => value.border > 0)).toBe(true);
    if (width === 1440) expect(boxes[1].x - boxes[0].right).toBeGreaterThanOrEqual(12);
    else expect(boxes[1].y - boxes[0].bottom).toBeGreaterThanOrEqual(12);
    expect(await region.evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true);
  }
  await region.screenshot({ path: "target/ui-check/usb-policy.png", animations: "disabled" });
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
