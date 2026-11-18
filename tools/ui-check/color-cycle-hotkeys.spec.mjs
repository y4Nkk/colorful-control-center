import { availableAction, configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });

test("颜色循环独立授权，确认策略不立即改灯光", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与输入");
  const color = panel(page).getByRole("checkbox", { name: "键盘颜色循环", exact: true });
  await expect(color).toBeEnabled(); await expect(color).not.toBeChecked();
  await panel(page).getByRole("checkbox", { name: "键盘亮度", exact: true }).check();
  await panel(page).getByRole("checkbox", { name: "键盘灯开关", exact: true }).check();
  await expect(color).not.toBeChecked();
  await color.check();
  await panel(page).getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await openConfigurationReview(page, "原厂热键与屏显");
  expect(await page.evaluate(() => window.__uiFixture.calls.some(c => c.command === "apply_hotkeys"))).toBe(false);
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  const calls = await page.evaluate(() => window.__uiFixture.calls);
  expect(calls.find(c => c.command === "apply_hotkeys").args.request.settings.actions).toEqual(["keyboardBrightness", "keyboardSwitch", "keyboardColor"]);
  expect(calls.some(c => /^(apply|restore)_keyboard/.test(c.command))).toBe(false);
});
test("type2分区能力不隐式开放原厂颜色循环", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => { window.__uiFixture.keyboard.detectedType = 2; });
  await page.goto("/"); await openWorkspace(page, "键盘与输入");
  await expect(panel(page).getByRole("checkbox", { name: "键盘分区开关", exact: true })).toBeEnabled();
  await expect(panel(page).getByRole("checkbox", { name: "键盘颜色循环", exact: true })).toBeDisabled();
  await expect(panel(page)).toContainText("此键盘未提供原厂颜色循环");
});
test("调色板身份变化撤销旧确认并允许取消范围", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与输入");
  const color = panel(page).getByRole("checkbox", { name: "键盘颜色循环", exact: true });
  await color.check();
  await panel(page).getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await openConfigurationReview(page, "原厂热键与屏显");
  await page.evaluate(() => {
    const profile = window.__uiFixture.hotkeys;
    const cap = profile.facts.capabilities.find(c => c.scope === "keyboardColor");
    cap.identity = null; cap.reason = "调色板规则已变化"; profile.revision = "palette-changed";
  });
  await panel(page).getByRole("button", { name: "重新读取热键", exact: true }).click();
  await expect(panel(page)).toContainText("调色板规则已变化");
  await expect(availableAction(page, "原厂热键与屏显")).toHaveCount(0);
  await color.uncheck(); await expect(color).not.toBeChecked();
  expect(await page.evaluate(() => window.__uiFixture.calls.some(c => c.command === "apply_hotkeys"))).toBe(false);
});
