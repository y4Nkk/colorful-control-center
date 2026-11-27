import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { editHotkeys, finishHotkeyEdit, openHotkeyDetails, openWorkspace } from "./workspace.mjs";

const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "apply_hotkeys"));
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/");
  await openWorkspace(page, "键盘与灯光");
});

test("合盖沿用两种供电的真实计划目标，预览确认不请求系统动作", async ({ page }) => {
  const editor = await editHotkeys(page);
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await editor.getByRole("checkbox", { name: "原厂合盖动作", exact: true }).check();
  await finishHotkeyEdit(editor);
  const details = await openHotkeyDetails(page);
  const preview = details.getByLabel("原厂合盖动作预览");
  const actions = preview.locator("dl[aria-label='合盖动作']");
  await expect(actions.locator("dt")).toHaveText(["插电", "电池"]);
  await expect(actions.locator("dd")).toHaveText(["关闭屏幕", "休眠"]);
  await expect(actions.locator('[data-icon="display"]')).toHaveCount(1);
  await expect(preview.getByRole("slider")).toHaveCount(0);
  await details.getByRole("button", { name: "关闭弹窗", exact: true }).click();
  await previewConfiguration(page, "原厂热键与屏显");
  expect(await writes(page)).toEqual([]);
  await configurationView(page).getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  expect((await writes(page))[0].args.request.settings.actions).toEqual(["lidAction"]);
  const actual = await page.evaluate(() => window.__uiFixture.calls.filter(call => /suspend|shutdown|monitor_power|lid_action/.test(call.command)));
  expect(actual).toEqual([]);
});

test("关机请求不继承合盖或低电量授权，初始恢复只还原策略", async ({ page }) => {
  const editor = await editHotkeys(page);
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  const selected = editor.getByRole("checkbox", { name: "原厂关机请求", exact: true });
  await selected.check();
  await expect(editor.getByRole("checkbox", { name: "原厂合盖动作", exact: true })).not.toBeChecked();
  await expect(editor.getByRole("checkbox", { name: "原厂低电量系统动作", exact: true })).not.toBeChecked();
  await finishHotkeyEdit(editor);
  await previewConfiguration(page, "原厂热键与屏显");
  await configurationView(page).getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  expect((await writes(page))[0].args.request.settings.actions).toEqual(["systemShutdown"]);
  await previewConfiguration(page, "原厂热键与屏显", true);
  await configurationView(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await editHotkeys(page);
  await expect(selected).not.toBeChecked();
});

test("计划变化撤销旧确认，未知供电与计划错误保留诊断", async ({ page }) => {
  const editor = await editHotkeys(page);
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await editor.getByRole("checkbox", { name: "原厂合盖动作", exact: true }).check();
  await finishHotkeyEdit(editor);
  await previewConfiguration(page, "原厂热键与屏显");
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys; h.revision += "-changed-lid";
    h.facts.capabilities.find(cap => cap.scope === "lidAction").identity = null;
    h.facts.lidAction.current = null; h.facts.lidAction.onAc = null;
    h.facts.lidAction.error = "隔离计划读取失败"; h.facts.lidAction.powerError = "隔离供电状态未知";
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(configurationView(page).getByRole("button", { name: "保存热键接管范围", exact: true })).toHaveCount(0);
  const details = await openHotkeyDetails(page);
  await expect(details.getByLabel("原厂合盖动作预览")).toContainText("隔离计划读取失败");
  await expect(details.getByLabel("原厂合盖动作预览")).toContainText("隔离供电状态未知");
  expect(await writes(page)).toEqual([]);
});
