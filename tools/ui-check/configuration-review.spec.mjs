import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";
import { installTouchpadFixture } from "./touchpad-fixture.mjs";
import { installMicrophoneFixture } from "./microphone-fixture.mjs";
import { pageActions } from "./configuration-review.mjs";
import { openDeviceRestore } from "./device-settings.mjs";

const region = (page, name) => page.getByRole("region", { name, exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls
  .filter(call => /^(apply_|restore_|set_)/.test(call.command) && call.command !== "set_shortcut_recording")
  .map(call => call.command));
const touch = page => region(page, "触摸板");
const mic = page => region(page, "麦克风静音");

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(installTouchpadFixture);
  await page.addInitScript(installMicrophoneFixture);
  await page.goto("/");
});

test("规格按需打开，弹窗可用 Escape 关闭并归还焦点", async ({ page }) => {
  const entry = page.getByRole("button", { name: "CPU 规格" });
  await expect(page.getByText("物理核心数", { exact: true })).toHaveCount(0);
  await entry.focus();
  await page.keyboard.press("Enter");
  const dialog = page.getByRole("dialog", { name: "CPU 硬件参数" });
  await expect(dialog.getByRole("heading")).toBeFocused();
  await expect(dialog.getByText("物理核心数", { exact: true })).toBeVisible();
  await expect(dialog.getByText("标称最高频率", { exact: true })).toBeVisible();
  await expect(dialog.locator(".ccc-dialog-header")).toHaveCSS("border-bottom-width", "0px");
  expect(await dialog.locator(".ccc-fact-row").evaluateAll(rows =>
    rows.length > 1 && rows.every(row => getComputedStyle(row).borderTopWidth === "0px")
  )).toBe(true);
  const close = dialog.getByRole("button", { name: "关闭弹窗", exact: true });
  const body = dialog.locator(".ccc-scroll-viewport");
  await page.keyboard.press("Tab");
  await expect(close).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(body).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(close).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(body).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(entry).toBeFocused();

  await entry.click();
  const box = await dialog.boundingBox();
  await page.mouse.move(box.x + 40, box.y + 34);
  await page.mouse.down();
  await page.mouse.move(2, 2);
  await page.mouse.up();
  await expect(dialog).toBeVisible();
  await page.mouse.click(2, 2);
  await expect(dialog).toHaveCount(0);
  await expect(entry).toBeFocused();
  expect(await writes(page)).toEqual([]);
});

test("触摸板与麦克风独立提交，跨页回读并支持单项恢复", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  await openWorkspace(page, "设备配置");
  await expect(touch(page)).toContainText("已启用");
  await expect(mic(page)).toContainText("部分静音");
  await expect(mic(page).getByText("内置麦克风", { exact: true })).toHaveCount(0);
  const pair = await Promise.all([touch(page).boundingBox(), mic(page).boundingBox()]);
  expect(pair[0].y).toBeCloseTo(pair[1].y, 0);
  expect(pair[0].height).toBeCloseTo(pair[1].height, 0);
  await touch(page).getByRole("radio", { name: "关闭", exact: true }).click();
  await expect(touch(page).getByRole("radio", { name: "关闭", exact: true })).toBeChecked();
  expect(await writes(page)).toEqual(["apply_touchpad"]);
  await mic(page).getByRole("radio", { name: "全部静音", exact: true }).click();
  await expect(mic(page).getByRole("radio", { name: "全部静音", exact: true })).toBeChecked();
  expect(await writes(page)).toEqual(["apply_touchpad", "apply_microphones"]);
  await expect(pageActions(page)).toHaveCount(0);
  await openWorkspace(page, "概览");
  await openWorkspace(page, "设备配置");
  await expect(touch(page).getByRole("radio", { name: "关闭", exact: true })).toBeChecked();
  await expect(mic(page).getByRole("radio", { name: "全部静音", exact: true })).toBeChecked();
  const restore = await openDeviceRestore(touch(page));
  await expect(restore).toContainText("已启用");
  await restore.getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(touch(page).getByLabel("当前触摸板状态", { exact: true })).toContainText("已启用");
  await expect(mic(page).getByRole("radio", { name: "全部静音", exact: true })).toBeChecked();
  expect(await writes(page)).toEqual(["apply_touchpad", "apply_microphones", "restore_touchpad"]);
});

test("设备写入期间锁定同一模块，离页后回读失败结果", async ({ page }) => {
  await openWorkspace(page, "设备配置");
  const keyboard = region(page, "键盘功能");
  const fnLock = keyboard.getByRole("switch", { name: "Fn 锁", exact: true });
  const winLock = keyboard.getByRole("switch", { name: "Win 锁", exact: true });
  await expect(fnLock).toBeEnabled();
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__uiFixture.keyboardFault = "隔离故障，已补偿";
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === "apply_keyboard_settings") await new Promise(resolve => { window.__finishKeyboardWrite = resolve; });
      return invoke(command, args);
    };
  });
  await fnLock.click();
  await expect(fnLock).toBeDisabled();
  await expect(winLock).toBeDisabled();
  await winLock.evaluate(input => input.click());
  await openWorkspace(page, "设置");
  await page.evaluate(() => window.__finishKeyboardWrite());
  await openWorkspace(page, "设备配置");
  await expect(keyboard.getByRole("alert")).toContainText("隔离故障");
  await expect(fnLock).not.toBeChecked();
  await expect(winLock).toBeEnabled();
  await expect(winLock).not.toBeChecked();
  expect(await writes(page)).toEqual(["apply_keyboard_settings"]);
});

test("设备修订变化拒绝旧请求，回读后可直接重试", async ({ page }) => {
  await openWorkspace(page, "设备配置");
  const off = touch(page).getByRole("radio", { name: "关闭", exact: true });
  await expect(off).toBeEnabled();
  await page.evaluate(() => Object.assign(window.__uiFixture.touchpad, {
    revision: "touchpad-updated", current: { ...window.__uiFixture.touchpad.current, saved: 1 },
  }));
  await off.click();
  await expect(touch(page).getByRole("alert")).toContainText("触摸板预览已变化");
  await expect(touch(page).getByRole("radio", { name: "启用", exact: true })).toBeChecked();
  await off.click();
  await expect(off).toBeChecked();
  const requests = await page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "apply_touchpad").map(call => call.args.request));
  expect(requests).toEqual([
    { expectedRevision: "touchpad-0", enabled: false },
    { expectedRevision: "touchpad-updated", enabled: false },
  ]);
});

test("显示试用期间锁定方向与刷新率，保留后可立即切换", async ({ page }) => {
  await openWorkspace(page, "设备配置");
  const screen = region(page, "屏幕显示");
  const rate = screen.getByRole("combobox", { name: "目标刷新率" });
  const rotate = screen.getByRole("switch", { name: "翻转主屏" });
  await rate.click();
  await page.getByRole("option", { name: "144 Hz" }).click();
  await expect(screen.getByRole("button", { name: "保留刷新率" })).toBeEnabled();
  await expect(rotate).toBeDisabled();
  await expect(rate).toBeDisabled();
  expect(await writes(page)).toEqual(["apply_display_refresh"]);
  await screen.getByRole("button", { name: "保留刷新率" }).click();
  await rotate.click();
  await expect(screen.getByRole("status")).toContainText("试用 横向（翻转）");
  expect(await writes(page)).toEqual(["apply_display_refresh", "apply_display_rotation"]);
});

test("弹窗内下拉可操作，第一个 Escape 只关闭菜单", async ({ page }) => {
  await page.setViewportSize({ width: 360, height: 720 });
  await openWorkspace(page, "设置");
  await page.getByRole("button", { name: "日志设置" }).click();
  const dialog = page.getByRole("dialog", { name: "运行日志" });
  const select = dialog.getByRole("combobox", { name: "日志级别" });
  await select.click();
  await expect(dialog.getByRole("listbox")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog.getByRole("listbox")).toHaveCount(0);
  await expect(dialog).toBeVisible();
  await select.click();
  await dialog.getByRole("option", { name: "警告" }).click();
  await expect(select).toHaveText("警告");
  await select.click();
  await expect(dialog.getByRole("listbox")).toBeVisible();
  await page.mouse.click(2, 2);
  await expect(dialog.getByRole("listbox")).toHaveCount(0);
  await expect(dialog).toBeVisible();
  await page.mouse.click(2, 2);
  await expect(dialog).toHaveCount(0);
});
