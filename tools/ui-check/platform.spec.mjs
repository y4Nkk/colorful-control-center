import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panelOf = page => page.getByRole("region", { name: "平台电源", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply|restore)_pd_level$/.test(c.command)));
const select = async (panel, page, label) => {
  await panel.getByRole("combobox", { name: "PD 档位" }).click();
  await page.getByRole("option", { name: label, exact: true }).click();
};
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    window.__uiFixture.platform = { deepS5: true, warnings: [], powerDelivery: { revision: "pd-0", encoding: "adapterV1", currentLevel: 1, currentValid: true,
      availableLevels: [1,2,4], zeroBased: false, adapterWatts: 100, initial: null, restoreBlockedReason: null, blockedReason: null } };
  });
  await page.goto("/"); await openWorkspace(page, "电池与节能");
});

test("PD等级按声明候选预览，确认后写入并独立恢复", async ({ page }) => {
  const panel = panelOf(page);
  await expect(panel).toContainText("100 W"); await expect(panel).toContainText("Deep S5");
  await panel.getByRole("combobox", { name: "PD 档位" }).click();
  await expect(page.getByRole("option", { name: "等级 3", exact: true })).toHaveCount(0);
  await page.getByRole("option", { name: "等级 4", exact: true }).click();
  await expect(panel).toContainText("等级 1 → 等级 4");
  await panel.getByRole("button", { name: "应用 PD 档位", exact: true }).click();
  expect(await writes(page)).toHaveLength(0);
  await panel.getByRole("button", { name: "确认应用 PD 档位？", exact: true }).click();
  expect((await writes(page))[0].args.request.level).toBe(4);
  await expect(panel).toContainText("恢复目标：等级 1");
  await panel.getByRole("button", { name: "恢复 PD 初始", exact: true }).click();
  expect(await writes(page)).toHaveLength(1);
  await panel.getByRole("button", { name: "恢复本会话修改前的 PD 档位？", exact: true }).click();
  await expect(panel.getByRole("combobox", { name: "PD 档位" })).toContainText("等级 1");
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("旧编号显示等级但发送原始编码，未知功率不补零", async ({ page }) => {
  const panel = panelOf(page);
  await page.evaluate(() => Object.assign(window.__uiFixture.platform.powerDelivery,
    { currentLevel: 0, encoding: "legacyZeroBased", availableLevels: [0,1], zeroBased: true, adapterWatts: null, revision: "pd-old" }));
  await panel.getByRole("button", { name: "重新读取平台电源" }).click();
  await expect(panel).toContainText("未报告");
  await select(panel, page, "等级 2");
  await panel.getByRole("button", { name: "应用 PD 档位", exact: true }).click();
  await panel.getByRole("button", { name: "确认应用 PD 档位？", exact: true }).click();
  expect((await writes(page))[0].args.request.level).toBe(1);
});

test("编辑、适配器变化与重读撤销确认，保留草稿并拒绝无效候选", async ({ page }) => {
  const panel = panelOf(page);
  await select(panel, page, "等级 4");
  await panel.getByRole("button", { name: "应用 PD 档位", exact: true }).click();
  await select(panel, page, "等级 2");
  await expect(panel.getByRole("button", { name: "应用 PD 档位", exact: true })).toBeVisible();
  await panel.getByRole("button", { name: "应用 PD 档位", exact: true }).click();
  await page.evaluate(() => Object.assign(window.__uiFixture.platform.powerDelivery, { adapterWatts: 45, availableLevels: [1], revision: "pd-changed" }));
  await panel.getByRole("button", { name: "重新读取平台电源" }).click();
  await expect(panel).toContainText("此适配器不支持当前草稿");
  await expect(panel.getByRole("button", { name: "应用 PD 档位", exact: true })).toBeDisabled();
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await page.getByRole("button", { name: "电池与节能", exact: true }).click();
  expect(await writes(page)).toHaveLength(0);
});

test("部分失败保留草稿与恢复入口，不重复写入", async ({ page }) => {
  const panel = panelOf(page);
  await select(panel, page, "等级 2");
  await page.evaluate(() => { window.__uiFixture.pdFault = "隔离PD写入失败，补偿尚需重试"; });
  await panel.getByRole("button", { name: "应用 PD 档位", exact: true }).click();
  await panel.getByRole("button", { name: "确认应用 PD 档位？", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("补偿尚需重试");
  await expect(panel.getByRole("combobox", { name: "PD 档位" })).toContainText("等级 2");
  await expect(panel.getByRole("button", { name: "恢复 PD 初始", exact: true })).toBeEnabled();
  expect(await writes(page)).toHaveLength(1);
});

test("校准锁和恢复不适配状态分别阻止写入", async ({ page }) => {
  const panel = panelOf(page);
  await select(panel, page, "等级 2");
  await panel.getByRole("button", { name: "应用 PD 档位", exact: true }).click();
  await page.evaluate(() => {
    window.__uiFixture.calibration.recoveryRequired = true;
    window.__uiFixture.platform.powerDelivery.initial = { value: 4, zeroBased: false };
  });
  await panel.getByRole("button", { name: "重新读取平台电源" }).click();
  await expect(panel).toContainText("电池校准期间不能更改PD档位");
  await expect(panel.getByRole("combobox", { name: "PD 档位" })).toBeDisabled();
  await expect(panel.getByRole("button", { name: "恢复 PD 初始", exact: true })).toBeDisabled();
  await page.evaluate(() => {
    window.__uiFixture.calibration.recoveryRequired = false;
    Object.assign(window.__uiFixture.platform.powerDelivery, { availableLevels: [1], restoreBlockedReason: "当前适配器不支持初始PD档位" });
  });
  await panel.getByRole("button", { name: "重新读取平台电源" }).click();
  await expect(panel).toContainText("当前适配器不支持初始PD档位");
  await expect(panel.getByRole("button", { name: "应用 PD 档位", exact: true })).toBeVisible();
  await expect(panel.getByRole("button", { name: "恢复 PD 初始", exact: true })).toBeDisabled();
});

test("PD模块左右布局与窄屏回流不溢出", async ({ page }) => {
  const panel = panelOf(page);
  for (const width of [1440,760,360]) {
    await page.setViewportSize({ width, height: 1000 });
    await expect(panel.getByRole("combobox", { name: "PD 档位" })).toBeVisible();
    expect(await panel.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  }
  await page.setViewportSize({ width: 1000, height: 1000 });
  await panel.screenshot({ path: "target/ui-check/platform-power.png" });
});

test("未知当前等级仍可恢复已有初始快照，读取失败撤销执行入口", async ({ page }) => {
  const panel = panelOf(page);
  await page.evaluate(() => Object.assign(window.__uiFixture.platform.powerDelivery,
    { currentLevel: 9, currentValid: false, initial: { value: 1, zeroBased: false }, revision: "unknown-current" }));
  await panel.getByRole("button", { name: "重新读取平台电源" }).click();
  await expect(panel).toContainText("当前等级未识别");
  await expect(panel.getByRole("button", { name: "应用 PD 档位", exact: true })).toBeDisabled();
  await expect(panel.getByRole("button", { name: "恢复 PD 初始", exact: true })).toBeEnabled();
  await page.evaluate(() => { window.__uiFixture.platformReadFault = "隔离平台读取失败"; });
  await panel.getByRole("button", { name: "重新读取平台电源" }).click();
  await expect(panel.getByRole("button", { name: "恢复 PD 初始", exact: true })).toHaveCount(0);
});

test("仅Deep S5状态不生成控制开关，未声明能力时隐藏面板", async ({ page }) => {
  const panel = panelOf(page);
  await page.evaluate(() => { window.__uiFixture.platform.powerDelivery = null; });
  await panel.getByRole("button", { name: "重新读取平台电源" }).click();
  await expect(panel).toContainText("Deep S5"); await expect(panel.getByRole("switch")).toHaveCount(0);
  await page.setViewportSize({ width: 360, height: 1000 });
  expect(await panel.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true);
  await page.evaluate(() => { window.__uiFixture.platform.deepS5 = null; });
  await panel.getByRole("button", { name: "重新读取平台电源" }).click();
  await expect(panel).toHaveCount(0);
});
