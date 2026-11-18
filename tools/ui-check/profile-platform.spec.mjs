import { openStartupParameters, openProfileResults } from "./profile-review.mjs";
import { configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "档案预览", exact: true });
const pd = page => panel(page).getByRole("region", { name: "档案PD档位", exact: true });
const usb = page => panel(page).getByRole("region", { name: "档案USB策略", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|save_|import_|set_startup)/.test(call.command)));
async function select(page) {
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: "性能档案", exact: true }).click();
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const data = window.__uiFixture;
    data.platform = { deepS5: true, warnings: [], powerDelivery: { revision: "pd-0", encoding: "adapterV1", currentLevel: 1, currentValid: true,
      availableLevels: [1, 2, 4], zeroBased: false, adapterWatts: 100, initial: null, restoreBlockedReason: null, blockedReason: null } };
    data.usb = { revision: "usb-0", recommendation: true, supported: true, activePlan: null, plans: [
      { id: "balanced", ac: true, dc: false }, { id: "highPerformance", ac: false, dc: true }, { id: "powerSaver", ac: false, dc: null }],
      initial: [], missing: [], blockedReason: null, restoreBlockedReason: null, warnings: [] };
    data.profile.settings.powerDelivery = { level: 4, encoding: "adapterV1", adapterWatts: 100, availableLevels: [1, 2, 4] };
    data.profile.settings.usbPolicy = { plans: structuredClone(data.usb.plans) };
  });
  await page.goto("/"); await openWorkspace(page, "电池与节能");
});

test("平台档案逐计划只读预览，确认提交完整PD绑定和异构USB目标", async ({ page }) => {
  await select(page);
  await expect(pd(page)).toContainText("100 W");
  await expect(pd(page).getByRole("listitem")).toHaveCount(3);
  await expect(pd(page).getByText("等级 4 · 目标", { exact: true })).toBeVisible();
  await expect(usb(page)).toContainText("3 个标准计划");
  await expect(usb(page)).toContainText("插电：开启；电池：关闭");
  await expect(usb(page)).toContainText("插电：关闭；电池：开启");
  await expect(usb(page)).toContainText("插电：关闭；电池：保留现值");
  await expect(panel(page).getByLabel("档案PD档位").locator("input,button")).toHaveCount(0);
  expect(await writes(page)).toEqual([]);
  await openConfigurationReview(page);
  expect(await writes(page)).toEqual([]);
  await configurationDialog(page).getByRole("button", { name: "应用档案", exact: true }).click();
  const calls = await writes(page); expect(calls).toHaveLength(1);
  expect(calls[0].args.expected.settings.powerDelivery).toEqual({ level: 4, encoding: "adapterV1", adapterWatts: 100, availableLevels: [1, 2, 4] });
  expect(calls[0].args.expected.settings.usbPolicy.plans[1]).toEqual({ id: "highPerformance", ac: false, dc: true });
  const results = await openProfileResults(page);
  await expect(results).toContainText("USB选择性暂停");
});

test("捕获导出使用当前PD和各计划原值，未知USB字段保留null", async ({ page }) => {
  await select(page); await page.getByPlaceholder("游戏 / 静音").fill("平台档案");
  await page.getByRole("button", { name: "保存当前", exact: true }).click();
  await page.getByRole("button", { name: "保存当前设置？", exact: true }).click();
  const download = page.waitForEvent("download");
  await panel(page).getByRole("button", { name: "导出", exact: true }).click();
  const file = JSON.parse(await readFile(await (await download).path(), "utf8"));
  expect(file.settings.powerDelivery.level).toBe(1);
  expect(file.settings.powerDelivery.encoding).toBe("adapterV1");
  expect(file.settings.usbPolicy.plans.map(plan => [plan.ac, plan.dc])).toEqual([[true, false], [false, true], [false, null]]);
  expect((await writes(page)).map(call => call.command)).toEqual(["save_profile"]);
});

test("旧版零起始档案按语义显示且确认保留原始编号", async ({ page }) => {
  await select(page);
  const file = await page.evaluate(() => {
    const file = structuredClone(window.__uiFixture.profile); file.name = "旧版PD";
    file.settings.powerDelivery = { level: 0, encoding: "legacyZeroBased", adapterWatts: null, availableLevels: [0, 1] };
    return file;
  });
  await page.getByLabel("导入性能档案", { exact: true }).setInputFiles({ name: "旧版.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(file)) });
  await expect(pd(page)).toContainText("未报告功率");
  await expect(pd(page).getByText("等级 1 · 目标", { exact: true })).toBeVisible();
  expect(await writes(page)).toEqual([]);
});

test("启动选择共享完整平台预览，保存选择不写平台", async ({ page }) => {
  await openWorkspace(page, "自动策略");
  const startup = page.getByRole("region", { name: "启动档案", exact: true });
  await startup.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  const parameters = await openStartupParameters(page);
  await expect(parameters.getByLabel("档案PD档位")).toContainText("等级 4");
  await expect(parameters.getByLabel("档案USB策略")).toContainText("保留现值");
  await page.keyboard.press("Escape");
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  const calls = await writes(page); expect(calls.map(call => call.command)).toEqual(["set_startup_profile"]);
  expect(calls[0].args.expected.settings.powerDelivery.level).toBe(4);
  expect(calls[0].args.expected.settings.usbPolicy.plans).toHaveLength(3);
});

test("档案变化撤销旧确认，平台失败和恢复失败保留明确步骤", async ({ page }) => {
  await select(page); await openConfigurationReview(page);
  await page.evaluate(() => { window.__uiFixture.profile.settings.powerDelivery.adapterWatts = 65; });
  await configurationDialog(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("档案已变化");
  await page.evaluate(() => { window.__uiFixture.profileSteps = [{ step: "USB选择性暂停", ok: false, message: "隔离字段写入失败" },
    { step: "恢复USB策略", ok: false, message: "初始快照保留，可重试" }, { step: "恢复PD档位", ok: true, message: "已确认" }]; });
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await openConfigurationReview(page);
  await configurationDialog(page).getByRole("button", { name: "应用档案", exact: true }).click();
  const results = await openProfileResults(page);
  await expect(results).toContainText("初始快照保留，可重试");
});

test("无法确认的PD或全部未知USB阻止捕获，不保存虚构目标", async ({ page }) => {
  await select(page); await page.getByPlaceholder("游戏 / 静音").fill("未知平台");
  await page.evaluate(() => { window.__uiFixture.platform.powerDelivery.currentValid = false; });
  await page.getByRole("button", { name: "保存当前", exact: true }).click();
  await page.getByRole("button", { name: "保存当前设置？", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("当前PD档位无法捕获");
  await page.evaluate(() => {
    window.__uiFixture.platform.powerDelivery.currentValid = true;
    window.__uiFixture.usb.plans.forEach(plan => { plan.ac = null; plan.dc = null; });
  });
  await page.getByRole("button", { name: "保存当前", exact: true }).click();
  await page.getByRole("button", { name: "保存当前设置？", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("USB档案没有已知目标字段");
  expect(await writes(page)).toEqual([]);
});

test("平台档案深浅宽窄布局保留模块边框和分区", async ({ page }) => {
  await select(page);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    for (const width of [1440, 360]) {
      await page.setViewportSize({ width, height: 1800 }); await usb(page).scrollIntoViewIfNeeded();
      await expect(usb(page)).toBeVisible();
      const overflow = await page.evaluate(() => document.documentElement.scrollWidth > innerWidth);
      expect(overflow).toBe(false);
      const layout = await usb(page).evaluate(element => ({ columns: getComputedStyle(element).gridTemplateColumns.split(" ").length,
        border: getComputedStyle(element).borderTopWidth }));
      expect(layout.columns).toBe(width === 1440 ? 2 : 1); expect(parseFloat(layout.border)).toBeGreaterThan(0);
      await usb(page).screenshot({ path: `target/ui-check/profile-usb-${theme}-${width}.png` });
      await pd(page).scrollIntoViewIfNeeded(); await pd(page).screenshot({ path: `target/ui-check/profile-pd-${theme}-${width}.png` });
    }
  }
});
