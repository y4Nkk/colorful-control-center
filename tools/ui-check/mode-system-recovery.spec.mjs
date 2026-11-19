import { availableAction, configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openSystemArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "预设模式", exact: true });
const plans = dialog => dialog.getByRole("region", { name: /链路节能恢复$/ });
const calls = page => page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "restore_oem_power_mode"));
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const data = window.__uiFixture;
    data.modeInitial = { mode: 3, command: 3, acMode: null, energy: null, gpu: null, options: structuredClone(data.performanceOptions.current),
      fan: { mode: 0, offset: 0, software: null, package: null, rpmPairs: [],
        fields: [{ offset: 5, data: [0] }, { offset: 7, data: [0] }] },
      systemPlans: [{ scheme: "381b4222f69441f09685ff5bb260df2e", initial: [0, 1] }, { scheme: "8c5e7fdae8bf4a969a85a6e23a8c635c", initial: [1, 2] }] };
    data.modeSystemValues = { "381b4222f69441f09685ff5bb260df2e": [2, 2], "8c5e7fdae8bf4a969a85a6e23a8c635c": [0, 0] };
  });
});

test("恢复预览显示所有计划的当前与初始字段，两次确认后整组恢复", async ({ page }) => {
  await page.goto("/"); await openSystemArea(page, "性能调校");
  const review = await previewConfiguration(page, "模式与性能选项", true);
  await expect(plans(review)).toHaveCount(2);
  await expect(plans(review).nth(0)).toContainText("最大节能 → 关闭");
  await expect(plans(review).nth(1)).toContainText("关闭 → 最大节能");
  expect(await calls(page)).toHaveLength(0);
  await configurationView(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(availableAction(page, "模式与性能选项", true)).toHaveCount(0);
  expect(await page.evaluate(() => Object.values(window.__uiFixture.modeSystemValues))).toEqual([[0, 1], [1, 2]]);
});

test("确认前另一计划变化拒绝旧恢复修订", async ({ page }) => {
  await page.goto("/"); await openSystemArea(page, "性能调校");
  await previewConfiguration(page, "模式与性能选项", true);
  await page.evaluate(() => { window.__uiFixture.modeSystemValues["8c5e7fdae8bf4a969a85a6e23a8c635c"][1] = 1; });
  await configurationView(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(page.getByRole("alert").filter({ hasText: "恢复目标已变化" })).toBeVisible();
  expect(await page.evaluate(() => window.__uiFixture.modeSystemValues["381b4222f69441f09685ff5bb260df2e"])).toEqual([2, 2]);
});

test("计划不可读显示未知并禁用恢复，深浅主题保留分区", async ({ page }, testInfo) => {
  await page.addInitScript(() => { delete window.__uiFixture.modeSystemValues["8c5e7fdae8bf4a969a85a6e23a8c635c"]; });
  await page.goto("/"); await openSystemArea(page, "性能调校");
  await expect(availableAction(page, "模式与性能选项", true)).toHaveCount(0);
  await panel(page).getByRole("button", { name: "模式恢复暂不可用", exact: true }).click();
  const details = page.getByRole("dialog", { name: "模式恢复暂不可用", exact: true });
  await expect(plans(details).nth(1)).toContainText("未知 →");
  for (const [width, theme] of [[1440, "light"], [700, "dark"]]) {
    await page.setViewportSize({ width, height: 1100 });
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    await details.screenshot({ path: testInfo.outputPath(`mode-system-${theme}-${width}.png`) });
  }
  expect(await calls(page)).toHaveLength(0);
});
