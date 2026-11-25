import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const trigger = page => page.getByRole("button", { name: /^通知（/ });
const notices = page => page.getByRole("dialog", { name: "通知", exact: true });

test("启动显示选项和原厂退出开关使用同一软件偏好", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await page.getByRole("button", { name: "设置", exact: true }).click();
  const display = page.getByRole("radiogroup", { name: "启动时显示", exact: true });
  const window = display.getByRole("radio", { name: "显示主界面" });
  const tray = display.getByRole("radio", { name: "最小化到托盘" });
  const close = page.getByRole("switch", { name: "自动关闭原厂软件" });
  await expect(window).toBeChecked();
  await expect(close).toBeChecked();
  await expect(page.getByText(/最小化到托盘时也生效/)).toBeVisible();
  await tray.check();
  await expect(tray).toBeChecked();
  await close.uncheck();
  await expect(close).not.toBeChecked();
  await expect(trigger(page)).toContainText("已保存启动显示方式");
  await window.check();
  await expect(window).toBeChecked();
  await close.check();
  await expect(close).toBeChecked();
  const saved = await page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "set_app_preferences")
    .map(({ args: { settings } }) => [settings.startMinimized, settings.closeOemOnStartup]));
  expect(saved).toEqual([[true, true], [true, false], [false, false], [false, true]]);
  await page.setViewportSize({ width: 360, height: 720 });
  await display.scrollIntoViewIfNeeded();
  const bounds = await display.boundingBox();
  expect(bounds.x + bounds.width).toBeLessThanOrEqual(360);
});

test("标题栏保留原厂启动结果，支持键盘展开、逐条关闭和清空", async ({ page }) => {
  await page.clock.install();
  await page.addInitScript({ content: `${MOCK_IPC}\nwindow.__uiFixture.oemStartupNotice = {
    kind: "failed", detail: "CC40.exe 位于其他 Windows 会话", closed: ["FnKey.exe", "ControlCenter30.exe"]
  };` });
  await page.goto("/");
  await expect(trigger(page)).toContainText("启动时未完全退出原厂软件");
  await page.clock.fastForward(12_000);
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await page.getByRole("radiogroup", { name: "启动时显示", exact: true })
    .getByRole("radio", { name: "最小化到托盘" }).check();
  await expect(trigger(page)).toHaveAccessibleName("通知（2 条）");
  await trigger(page).focus();
  await page.keyboard.press("Enter");
  await expect(notices(page)).toBeVisible();
  await page.clock.fastForward(12_000);
  await expect(notices(page)).toContainText("已保存启动显示方式");
  await expect(notices(page)).toContainText("位于其他 Windows 会话");
  await expect(notices(page)).toContainText("已关闭 FnKey.exe、ControlCenter30.exe");
  await page.keyboard.press("Escape");
  await expect(notices(page)).toBeHidden();
  await expect(trigger(page)).toBeFocused();
  await trigger(page).click();
  const summary = notices(page).getByRole("button", { name: "关闭通知", exact: true });
  await summary.first().click();
  await expect(summary).toHaveCount(1);
  await expect(notices(page)).toContainText("位于其他 Windows 会话");
  await page.setViewportSize({ width: 360, height: 720 });
  const bounds = await notices(page).boundingBox();
  expect(bounds.x).toBeGreaterThanOrEqual(0);
  expect(bounds.x + bounds.width).toBeLessThanOrEqual(360);
  await page.screenshot({ path: "target/ui-check/startup-notifications-360.png" });
  await notices(page).getByRole("button", { name: "清空通知" }).click();
  await expect(trigger(page)).toHaveCount(0);
});
