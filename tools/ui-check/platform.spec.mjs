import { availableAction, configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openPowerArea } from "./workspace.mjs";
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
  await page.goto("/"); await openPowerArea(page, "电源设置");
});

test("PD 选择按固件候选直接写入并独立恢复", async ({ page }) => {
  const panel = panelOf(page);
  await expect(panel).toContainText("100 W");
  await expect(panel).toContainText("Deep S5");
  await panel.getByRole("combobox", { name: "PD 档位" }).click();
  await expect(page.getByRole("option", { name: "等级 3", exact: true })).toHaveCount(0);
  await page.getByRole("option", { name: "等级 4", exact: true }).click();
  await expect(panel.getByRole("combobox", { name: "PD 档位" })).toContainText("等级 4");
  expect((await writes(page))[0].args.request.level).toBe(4);
  const restore = await previewConfiguration(page, "PD 档位", true);
  await expect(restore).toContainText("等级 1");
  expect(await writes(page)).toHaveLength(1);
  await restore.getByRole("button", { name: "确认恢复", exact: true }).click();
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
  await expect(panel.getByRole("combobox", { name: "PD 档位" })).toContainText("等级 2");
  expect((await writes(page))[0].args.request.level).toBe(1);
});

test("适配器更换使旧选择失效并回读可用档位", async ({ page }) => {
  const panel = panelOf(page);
  await expect(panel.getByRole("combobox", { name: "PD 档位" })).toContainText("等级 1");
  await page.evaluate(() => Object.assign(window.__uiFixture.platform.powerDelivery, { adapterWatts: 45, availableLevels: [1], revision: "pd-changed" }));
  await select(panel, page, "等级 4");
  await expect(panel.getByRole("alert")).toBeVisible();
  await expect(panel).toContainText("45 W");
  await expect(panel.getByRole("combobox", { name: "PD 档位" })).toContainText("等级 1");
  expect(await writes(page)).toHaveLength(1);
});

test("部分失败回读实际档位并保留恢复入口", async ({ page }) => {
  const panel = panelOf(page);
  await page.evaluate(() => { window.__uiFixture.pdFault = "隔离PD写入失败，补偿尚需重试"; });
  await select(panel, page, "等级 2");
  await expect(panel.getByRole("alert")).toContainText("补偿尚需重试");
  await expect(panel.getByRole("combobox", { name: "PD 档位" })).toContainText("等级 2");
  await expect(availableAction(page, "PD 档位", true)).toBeEnabled();
  expect(await writes(page)).toHaveLength(1);
});

test("校准锁和恢复不适配状态分别阻止写入", async ({ page }) => {
  const panel = panelOf(page);
  await expect(panel.getByRole("combobox", { name: "PD 档位" })).toBeEnabled();
  await page.evaluate(() => {
    window.__uiFixture.calibration.recoveryRequired = true;
    window.__uiFixture.platform.powerDelivery.initial = { value: 4, zeroBased: false };
  });
  await panel.getByRole("button", { name: "重新读取平台电源" }).click();
  await expect(panel).toContainText("电池校准期间不能更改PD档位");
  await expect(panel.getByRole("combobox", { name: "PD 档位" })).toBeDisabled();
  await expect(availableAction(page, "PD 档位", true)).toHaveCount(0);
  await page.evaluate(() => {
    window.__uiFixture.calibration.recoveryRequired = false;
    Object.assign(window.__uiFixture.platform.powerDelivery, { availableLevels: [1], restoreBlockedReason: "当前适配器不支持初始PD档位" });
  });
  await panel.getByRole("button", { name: "重新读取平台电源" }).click();
  await expect(panel.getByRole("combobox", { name: "PD 档位" })).toBeEnabled();
  await expect(panel).toContainText("当前适配器不支持初始PD档位");
  await expect(availableAction(page, "PD 档位", true)).toHaveCount(0);
});

test("部分读取失败的提示直接显示在卡内", async ({ page }) => {
  const panel = panelOf(page);
  await page.evaluate(() => { window.__uiFixture.platform.warnings = ["PD档位：隔离读取失败"]; });
  await panel.getByRole("button", { name: "重新读取平台电源" }).click();
  await expect(panel).toContainText("PD档位：隔离读取失败");
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
  await expect(panel).toContainText("档位未知");
  await expect(availableAction(page, "PD 档位")).toHaveCount(0);
  await expect(availableAction(page, "PD 档位", true)).toBeEnabled();
  await page.evaluate(() => { window.__uiFixture.platformReadFault = "隔离平台读取失败"; });
  await panel.getByRole("button", { name: "重新读取平台电源" }).click();
  await expect(availableAction(page, "PD 档位", true)).toHaveCount(0);
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
