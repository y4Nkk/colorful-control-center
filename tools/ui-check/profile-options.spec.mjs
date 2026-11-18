import { openProfileDetails, openStartupParameters, openProfileResults } from "./profile-review.mjs";
import { availableAction, changeButton, configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { readFile } from "node:fs/promises";

const preview = page => page.getByRole("region", { name: "档案预览", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply_|save_|import_|set_startup)/.test(c.command)));
async function selectProfile(page) {
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: "性能档案", exact: true }).click();
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const data = window.__uiFixture;
    data.profile.settings.oemPowerMode = 2;
    data.profile.settings.performanceOptions = { turboFan: true, dtt: true };
  });
  await page.goto("/"); await openWorkspace(page, "性能调校");
});

test("档案组合预览包含目标选项，两次确认携带同一目标", async ({ page }) => {
  await selectProfile(page);
  const review = await openConfigurationReview(page);
  await expect(review.getByLabel("档案性能选项")).toContainText("随性能模式应用");
  await expect(review.getByLabel("模式联动目标")).toContainText("性能增强");
  await expect(review.getByLabel("模式联动目标")).toContainText("动态调优");
  expect(await writes(page)).toEqual([]);
  await configurationDialog(page).getByRole("button", { name: "应用档案", exact: true }).click();
  const results = await openProfileResults(page);
  await expect(results.getByRole("listitem").filter({ hasText: "性能选项" }).getByRole("img", { name: "成功", exact: true })).toBeVisible();
  const [call] = await writes(page);
  expect(call.args.expectedMode).toMatchObject({ mode: 2, turbo: true, dtt: true, fanMode: 0 });
  expect(call.args.expected.settings.performanceOptions).toEqual({ turboFan: true, dtt: true });
});

test("其他模式保存偏好，图形预览和导出保留开关值", async ({ page }) => {
  await page.evaluate(() => window.__uiFixture.profile.settings.oemPowerMode = 3);
  await selectProfile(page);
  const panel = preview(page);
  const details = await openProfileDetails(page);
  await details.getByLabel("档案性能选项").getByRole("button", { name: "应用顺序", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "应用顺序", exact: true })).toContainText("本次不激活它们");
  await page.keyboard.press("Escape");
  await expect(details.getByLabel("模式联动目标")).not.toContainText("动态调优");
  await page.keyboard.press("Escape");
  const download = page.waitForEvent("download");
  await panel.getByRole("button", { name: "导出", exact: true }).click();
  const file = await download;
  const exported = JSON.parse(await readFile(await file.path(), "utf8"));
  expect(exported.settings.performanceOptions).toEqual({ turboFan: true, dtt: true });
  expect(await writes(page)).toEqual([]);
});

test("捕获保留当前保存偏好并省略固件未声明的选项", async ({ page }) => {
  await page.evaluate(() => {
    window.__uiFixture.performanceOptions.current = { turboFan: null, dtt: true };
    window.__uiFixture.performanceOptions.currentMode = 3;
  });
  await selectProfile(page);
  await page.getByPlaceholder("游戏 / 静音").fill("保存偏好");
  await page.getByRole("button", { name: "保存当前", exact: true }).click();
  await page.getByRole("button", { name: "保存当前设置？", exact: true }).click();
  await expect(preview(page).getByRole("heading", { name: "保存偏好" })).toBeVisible();
  const saved = (await writes(page)).find(call => call.command === "save_profile");
  expect(saved.args.args.settings.performanceOptions).toEqual({ turboFan: null, dtt: true });
  await expect((await openProfileDetails(page)).getByLabel("档案性能选项")).not.toContainText("Turbo 风扇");
});

test("驱动在确认后变化时拒绝激活，重新预览显示错误", async ({ page }) => {
  await selectProfile(page);
  await openConfigurationReview(page);
  await page.evaluate(() => window.__uiFixture.performanceOptions.dttDriverAvailable = false);
  await configurationDialog(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("不能激活DTT");
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await expect(changeButton(page)).toHaveCount(0);
  await expect(preview(page).getByRole("button", { name: /^执行明细/ })).toHaveCount(0);
});

test("启动选择展示组合联动，保存完整文件且不发送应用", async ({ page }) => {
  await openWorkspace(page, "自动策略");
  const panel = page.getByRole("region", { name: "启动档案", exact: true });
  await panel.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  const parameters = await openStartupParameters(page);
  await expect(parameters.getByLabel("模式联动目标")).toContainText("动态调优");
  await expect(parameters).toContainText("启动时保存插电偏好并应用模式的固件节能联动");
  await page.keyboard.press("Escape");
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  const calls = await writes(page);
  expect(calls.map(call => call.command)).toEqual(["set_startup_profile"]);
  expect(calls[0].args.expected.settings.performanceOptions).toEqual({ turboFan: true, dtt: true });
});

test("保留开启DTT但驱动未知时，先关闭DTT才能应用其他选项", async ({ page }) => {
  const panel = page.getByRole("region", { name: "性能选项", exact: true });
  await page.evaluate(() => {
    const options = window.__uiFixture.performanceOptions;
    options.current.dtt = true; options.dttDriverAvailable = null;
  });
  await panel.getByRole("button", { name: "重新读取性能选项", exact: true }).click();
  await panel.getByRole("switch", { name: "Turbo 风扇", exact: true }).click();
  await expect(availableAction(page, "性能选项")).toHaveCount(0);
  await expect(panel).toContainText("请确认驱动或先关闭 DTT");
  await panel.getByRole("switch", { name: "动态调校 DTT", exact: true }).click();
  await expect(availableAction(page, "性能选项")).toBeEnabled();
  expect(await writes(page)).toEqual([]);
});

test("性能档案卡片在宽窄及深浅主题下保持模块边框和可读布局", async ({ page }) => {
  await selectProfile(page);
  const panel = (await openProfileDetails(page)).getByLabel("档案性能选项");
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 1440]) {
      await page.setViewportSize({ width, height: 1800 });
      await panel.scrollIntoViewIfNeeded();
      expect(await panel.evaluate(el => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
      await expect(panel).toHaveAttribute("data-surface", "group");
      await panel.screenshot({ path: `target/ui-check/profile-options-${theme}-${width}.png` });
    }
  }
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
