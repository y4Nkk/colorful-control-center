import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "触摸板", exact: true });
const current = page => panel(page).getByLabel("当前触摸板状态", { exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply|restore)_touchpad$/.test(call.command)));
async function details(page) {
  await panel(page).getByRole("button", { name: "详情", exact: true }).click();
  return page.getByRole("dialog", { name: "触摸板详情", exact: true });
}
async function expectPreference(page, value) {
  const dialog = await details(page);
  await expect(dialog.locator(".ccc-fact-row").filter({ hasText: "保存偏好" }).locator("dd")).toHaveText(value);
  await page.keyboard.press("Escape");
}
async function expectDiagnostic(page, message) {
  expect(await details(page)).toContainText(message);
  await page.keyboard.press("Escape");
}
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "设备配置"); });

test("触摸板页内直接切换，系统开关与保存偏好同步提交", async ({ page }) => {
  await expect(current(page)).toContainText("已启用");
  await expectPreference(page, "已关闭");
  expect(await writes(page)).toHaveLength(0);
  await panel(page).getByRole("radio", { name: "关闭", exact: true }).click();
  await expect(panel(page).getByRole("radio", { name: "关闭", exact: true })).toBeChecked();
  await expect(current(page)).not.toContainText("已关闭");
  await panel(page).getByRole("radio", { name: "启用", exact: true }).click();
  await expect(panel(page).getByRole("radio", { name: "启用", exact: true })).toBeChecked();
  await expect(panel(page).getByRole("button", { name: "恢复初始设置", exact: true })).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
test("读取外部变化后，切换提交新的设备修订", async ({ page }) => {
  await expect(current(page)).toContainText("已启用");
  await page.evaluate(() => { const p = window.__uiFixture.touchpad; p.current.saved = 1; p.revision = "external-touchpad"; });
  await panel(page).getByRole("button", { name: "重新读取触摸板", exact: true }).click();
  await expect(panel(page).getByRole("radio", { name: "启用", exact: true })).toBeChecked();
  expect(await writes(page)).toHaveLength(0);
  await panel(page).getByRole("radio", { name: "关闭", exact: true }).click();
  await expect(panel(page).getByRole("radio", { name: "关闭", exact: true })).toBeChecked();
  expect((await writes(page))[0].args.request).toEqual({ expectedRevision: "external-touchpad", enabled: false });
});
test("读取失败撤销执行快照，重新读取恢复操作", async ({ page }) => {
  await page.evaluate(() => window.__uiFixture.touchpadReadFault = "隔离读取失败");
  await panel(page).getByRole("button", { name: "重新读取触摸板", exact: true }).click();
  await expect(panel(page).getByRole("alert")).toBeVisible();
  await expectDiagnostic(page, "隔离读取失败");
  await expect(panel(page).getByRole("radio", { name: "关闭", exact: true })).toBeDisabled();
  await page.evaluate(() => window.__uiFixture.touchpadReadFault = null);
  await panel(page).getByRole("button", { name: "重新读取触摸板", exact: true }).click();
  await expect(panel(page).getByRole("radio", { name: "关闭", exact: true })).toBeEnabled();
  await expect(current(page)).toContainText("已启用");
  expect(await writes(page)).toHaveLength(0);
});
test("写入与补偿均失败时显示实际状态", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.touchpadWriteFault = "隔离写入失败"; window.__uiFixture.touchpadRecoveryFault = true; });
  await panel(page).getByRole("radio", { name: "关闭", exact: true }).click();
  await expect(panel(page).getByRole("alert")).toBeVisible();
  await expectDiagnostic(page, "恢复失败");
  expect(await page.evaluate(() => window.__uiFixture.touchpad.current.enabled)).toBe(false);
});
test("设备变化后仍可直接切换", async ({ page }) => {
  await panel(page).getByRole("radio", { name: "关闭", exact: true }).click();
  await expect(panel(page).getByRole("radio", { name: "关闭", exact: true })).toBeChecked();
  await page.evaluate(() => { const p = window.__uiFixture.touchpad; p.current.devices = ["replacement"]; p.revision = "replacement"; });
  await panel(page).getByRole("button", { name: "重新读取触摸板", exact: true }).click();
  await expect(panel(page).getByRole("radio", { name: "启用", exact: true })).toBeEnabled();
  await panel(page).getByRole("radio", { name: "启用", exact: true }).click();
  await expect(panel(page).getByRole("radio", { name: "启用", exact: true })).toBeChecked();
  expect(await writes(page)).toHaveLength(2);
});
test("未知系统值不使用保存偏好冒充状态", async ({ page }) => {
  await expect(current(page)).toContainText("已启用");
  await page.evaluate(() => { const p = window.__uiFixture.touchpad; p.current.enabled = null; p.current.saved = 1; p.revision = "unknown"; });
  await panel(page).getByRole("button", { name: "重新读取触摸板", exact: true }).click();
  await expect(current(page)).toContainText("未知");
  await expectPreference(page, "已启用");
  await expect(panel(page).getByRole("radio", { name: "关闭", exact: true })).toBeDisabled();
  expect(await writes(page)).toHaveLength(0);
});
test("触摸板热键有独立范围", async ({ page }) => {
  await openWorkspace(page, "键盘与灯光");
  const hotkeys = page.getByRole("region", { name: "原厂热键与屏显", exact: true });
  await hotkeys.getByRole("button", { name: "配置热键", exact: true }).click();
  const editor = page.getByRole("dialog", { name: "配置热键与屏显", exact: true });
  const touchpad = editor.getByRole("checkbox", { name: /^触摸板开关/ });
  await expect(touchpad).not.toBeChecked(); await touchpad.check();
  await expect(editor.getByRole("checkbox", { name: /^麦克风静音/ })).not.toBeChecked();
  expect(await writes(page)).toHaveLength(0);
});
test("深浅宽窄设备模块边框与并列布局", async ({ page }) => {
  const microphone = page.getByRole("region", { name: "麦克风静音", exact: true });
  for (const theme of ["light", "dark"]) for (const width of [1440, 760, 360]) {
    await page.setViewportSize({ width, height: 1000 });
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    await panel(page).scrollIntoViewIfNeeded();
    await expect(panel(page)).toHaveCSS("border-top-width", "1px");
    await expect(microphone).toHaveCSS("border-top-width", "1px");
    const [a, b] = await Promise.all([panel(page).boundingBox(), microphone.boundingBox()]);
    expect(a.x + a.width).toBeLessThanOrEqual(width + 1);
    expect(b.x + b.width).toBeLessThanOrEqual(width + 1);
    if (width === 1440) { expect(b.x).toBeGreaterThan(a.x + a.width); expect(b.y).toBeCloseTo(a.y, 0); }
    if (width === 360) expect(b.y).toBeGreaterThan(a.y);
    await panel(page).screenshot({ path: `target/ui-check/touchpad-${theme}-${width}.png` });
  }
});
