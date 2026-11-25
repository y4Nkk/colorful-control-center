import { availableAction, configurationView, previewConfiguration } from "./configuration-review.mjs";
import { editHotkeys, finishHotkeyEdit, openHotkeyDetails, openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });

test("颜色循环独立授权，确认策略不立即改灯光", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与灯光");
  const editor = await editHotkeys(page);
  const color = editor.getByRole("checkbox", { name: "键盘颜色循环", exact: true });
  await expect(color).toBeEnabled(); await expect(color).not.toBeChecked();
  await editor.getByRole("checkbox", { name: "键盘亮度", exact: true }).check();
  await editor.getByRole("checkbox", { name: "键盘灯开关", exact: true }).check();
  await expect(color).not.toBeChecked();
  await color.check();
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await finishHotkeyEdit(editor);
  await previewConfiguration(page, "原厂热键与屏显");
  expect(await page.evaluate(() => window.__uiFixture.calls.some(c => c.command === "apply_hotkeys"))).toBe(false);
  await configurationView(page).getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  const calls = await page.evaluate(() => window.__uiFixture.calls);
  expect(calls.find(c => c.command === "apply_hotkeys").args.request.settings.actions).toEqual(["keyboardBrightness", "keyboardSwitch", "keyboardColor"]);
  expect(calls.some(c => /^(apply|restore)_keyboard/.test(c.command))).toBe(false);
});
test("type2分区能力不隐式开放原厂颜色循环", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => { window.__uiFixture.keyboard.detectedType = 2; });
  await page.goto("/"); await openWorkspace(page, "键盘与灯光");
  const editor = await editHotkeys(page);
  await expect(editor.getByRole("checkbox", { name: "键盘分区开关", exact: true })).toBeEnabled();
  await expect(editor.getByRole("checkbox", { name: "键盘颜色循环", exact: true })).toHaveCount(0);
  await finishHotkeyEdit(editor);
  await expect(await openHotkeyDetails(page)).toContainText("此键盘未提供原厂颜色循环");
});
test("调色板身份变化撤销旧确认并允许取消范围", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与灯光");
  const editor = await editHotkeys(page);
  const color = editor.getByRole("checkbox", { name: "键盘颜色循环", exact: true });
  await color.check();
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await finishHotkeyEdit(editor);
  await previewConfiguration(page, "原厂热键与屏显");
  await page.evaluate(() => {
    const profile = window.__uiFixture.hotkeys;
    const cap = profile.facts.capabilities.find(c => c.scope === "keyboardColor");
    cap.identity = null; cap.reason = "调色板规则已变化"; profile.revision = "palette-changed";
  });
  await panel(page).getByRole("button", { name: "重新读取热键", exact: true }).click();
  await expect(availableAction(page, "原厂热键与屏显")).toHaveCount(0);
  await editHotkeys(page);
  await expect(color).toBeChecked();
  await expect(editor).toContainText("暂不可用，请取消选择。");
  await color.click();
  await expect(color).toHaveCount(0);
  await finishHotkeyEdit(editor);
  await expect(await openHotkeyDetails(page)).toContainText("调色板规则已变化");
  expect(await page.evaluate(() => window.__uiFixture.calls.some(c => c.command === "apply_hotkeys"))).toBe(false);
});
