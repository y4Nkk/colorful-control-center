import { configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";

const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "apply_hotkeys"));
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/");
  await openWorkspace(page, "键盘与输入");
});

test("合盖沿用两种供电的真实计划目标，预览确认不请求系统动作", async ({ page }) => {
  const p = panel(page);
  await p.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await p.getByRole("checkbox", { name: "原厂合盖动作", exact: true }).check();
  const preview = p.getByLabel("原厂合盖动作预览");
  await expect(preview).toContainText("插电"); await expect(preview).toContainText("关闭屏幕");
  await expect(preview).toContainText("电池"); await expect(preview).toContainText("休眠");
  await expect(preview.locator('[data-icon="display"]')).toHaveCount(1);
  await expect(preview.getByRole("slider")).toHaveCount(0);
  await openConfigurationReview(page, "原厂热键与屏显");
  expect(await writes(page)).toEqual([]);
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  expect((await writes(page))[0].args.request.settings.actions).toEqual(["lidAction"]);
  const actual = await page.evaluate(() => window.__uiFixture.calls.filter(call => /suspend|shutdown|monitor_power|lid_action/.test(call.command)));
  expect(actual).toEqual([]);
});

test("关机请求不继承合盖或低电量授权，初始恢复只还原策略", async ({ page }) => {
  const p = panel(page);
  await p.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  const selected = p.getByRole("checkbox", { name: "原厂关机请求", exact: true }); await selected.check();
  await expect(p.getByRole("checkbox", { name: "原厂合盖动作", exact: true })).not.toBeChecked();
  await expect(p.getByRole("checkbox", { name: "原厂低电量系统动作", exact: true })).not.toBeChecked();
  await openConfigurationReview(page, "原厂热键与屏显");
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  expect((await writes(page))[0].args.request.settings.actions).toEqual(["systemShutdown"]);
  await openConfigurationReview(page, "原厂热键与屏显", true);
  await configurationDialog(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(selected).not.toBeChecked();
});

test("计划变化撤销旧确认，未知供电与计划错误保留诊断", async ({ page }) => {
  const p = panel(page);
  await p.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await p.getByRole("checkbox", { name: "原厂合盖动作", exact: true }).check();
  await openConfigurationReview(page, "原厂热键与屏显");
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys; h.revision += "-changed-lid";
    h.facts.capabilities.find(cap => cap.scope === "lidAction").identity = null;
    h.facts.lidAction.current = null; h.facts.lidAction.onAc = null;
    h.facts.lidAction.error = "隔离计划读取失败"; h.facts.lidAction.powerError = "隔离供电状态未知";
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(configurationDialog(page).getByRole("button", { name: "确认应用", exact: true })).toHaveCount(0);
  await expect(p.getByLabel("原厂合盖动作预览")).toContainText("隔离计划读取失败");
  await expect(p.getByLabel("原厂合盖动作预览")).toContainText("隔离供电状态未知");
  expect(await writes(page)).toEqual([]);
});
