import { availableAction, configurationView, previewConfiguration } from "./configuration-review.mjs";
import { editHotkeys, finishHotkeyEdit, openHotkeyDetails, openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });

test("原厂分区热键默认独立未授权，普通RGB不能借用type2能力", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与灯光");
  const editor = await editHotkeys(page);
  await expect(editor.getByRole("checkbox", { name: "键盘分区开关", exact: true })).toHaveCount(0);
  await expect(editor.getByRole("checkbox", { name: "键盘灯开关", exact: true })).toBeEnabled();
  await finishHotkeyEdit(editor);
  await expect(await openHotkeyDetails(page)).toContainText("此键盘未提供原厂分区开关");
});
test("type2分区范围不继承总开关，确认只保存授权", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => { window.__uiFixture.keyboard.detectedType = 2; });
  await page.goto("/"); await openWorkspace(page, "键盘与灯光");
  const editor = await editHotkeys(page);
  const zones = editor.getByRole("checkbox", { name: "键盘分区开关", exact: true });
  await expect(zones).toBeEnabled(); await expect(zones).not.toBeChecked();
  await editor.getByRole("checkbox", { name: "键盘灯开关", exact: true }).check();
  await expect(zones).not.toBeChecked();
  await zones.check();
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await finishHotkeyEdit(editor);
  await previewConfiguration(page, "原厂热键与屏显");
  await configurationView(page).getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  const calls = await page.evaluate(() => window.__uiFixture.calls);
  expect(calls.filter(call => call.command === "apply_hotkeys")[0].args.request.settings.actions).toEqual(["keyboardSwitch", "keyboardZones"]);
  expect(calls.some(call => /^(apply|restore)_keyboard/.test(call.command))).toBe(false);
});
test("分区设备能力变化会撤销已预览的授权", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => { window.__uiFixture.keyboard.detectedType = 2; });
  await page.goto("/"); await openWorkspace(page, "键盘与灯光");
  const editor = await editHotkeys(page);
  const zones = editor.getByRole("checkbox", { name: "键盘分区开关", exact: true });
  await zones.check();
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await finishHotkeyEdit(editor);
  await previewConfiguration(page, "原厂热键与屏显");
  await page.evaluate(() => {
    const data = window.__uiFixture;
    const cap = data.hotkeys.facts.capabilities.find(cap => cap.scope === "keyboardZones");
    cap.identity = null; cap.reason = "键盘已替换"; data.hotkeys.revision = "zone-replacement";
  });
  await panel(page).getByRole("button", { name: "重新读取热键", exact: true }).click();
  await expect(availableAction(page, "原厂热键与屏显")).toHaveCount(0);
  await editHotkeys(page);
  await expect(zones).toBeChecked();
  await expect(editor).toContainText("暂不可用，请取消选择。");
  await zones.click();
  await finishHotkeyEdit(editor);
  await expect(await openHotkeyDetails(page)).toContainText("键盘已替换");
  expect(await page.evaluate(() => window.__uiFixture.calls.some(c => c.command === "apply_hotkeys"))).toBe(false);
});
