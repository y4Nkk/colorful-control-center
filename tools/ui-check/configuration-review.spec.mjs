import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";
import { installTouchpadFixture } from "./touchpad-fixture.mjs";
import { installMicrophoneFixture } from "./microphone-fixture.mjs";
import { changeButton, restoreButton, openConfigurationReview, confirmConfiguration } from "./configuration-review.mjs";

const region = (page, name) => page.getByRole("region", { name, exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls
  .filter(call => /^(apply_|restore_|set_)/.test(call.command) && call.command !== "set_shortcut_recording")
  .map(call => call.command));
const touch = page => region(page, "触摸板设置");
const mic = page => region(page, "麦克风静音");

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(installTouchpadFixture);
  await page.addInitScript(installMicrophoneFixture);
  await page.goto("/");
});

test("规格按需打开，弹窗可用 Escape 关闭并归还焦点", async ({ page }) => {
  const entry = page.getByRole("button", { name: "CPU 详细信息" });
  await expect(page.getByText("物理核心数", { exact: true })).toHaveCount(0);
  await entry.focus();
  await page.keyboard.press("Enter");
  const dialog = page.getByRole("dialog", { name: "CPU 硬件参数" });
  await expect(dialog.getByRole("heading")).toBeFocused();
  await expect(dialog.getByText("物理核心数", { exact: true })).toBeVisible();
  await expect(dialog.getByText("标称最高频率", { exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(entry).toBeFocused();
  expect(await writes(page)).toEqual([]);
});

test("触摸板和麦克风同排对齐，一次确认后分别写入并单独恢复", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  await openWorkspace(page, "键盘与输入");
  await expect(touch(page)).toContainText("已启用");
  await expect(mic(page)).toContainText("部分静音");
  await expect(mic(page).getByText("内置麦克风", { exact: true })).toHaveCount(0);
  const pair = await Promise.all([touch(page).boundingBox(), mic(page).boundingBox()]);
  expect(pair[0].y).toBeCloseTo(pair[1].y, 0);
  expect(pair[0].height).toBeCloseTo(pair[1].height, 0);
  await touch(page).getByRole("radio", { name: "关闭", exact: true }).check();
  await mic(page).getByRole("radio", { name: "全部静音", exact: true }).check();
  await expect(changeButton(page)).toHaveCount(1);
  let review = await openConfigurationReview(page, "触摸板");
  await expect(review.getByRole("heading", { name: "麦克风", exact: true })).toBeVisible();
  await expect(review).toContainText("内置麦克风");
  await expect(review).toContainText("已启用 → 已关闭");
  expect(await writes(page)).toEqual([]);
  await review.getByRole("button", { name: "取消", exact: true }).click();
  await openWorkspace(page, "概览");
  await openWorkspace(page, "键盘与输入");
  await expect(touch(page).getByRole("radio", { name: "关闭", exact: true })).toBeChecked();
  await expect(mic(page).getByRole("radio", { name: "全部静音", exact: true })).toBeChecked();
  review = await openConfigurationReview(page, "触摸板");
  await confirmConfiguration(review);
  expect(await writes(page)).toEqual(["apply_touchpad", "apply_microphones"]);
  await expect(mic(page)).toContainText("全部静音");
  await expect(changeButton(page)).toHaveCount(0);
  const restore = await openConfigurationReview(page, "触摸板", true);
  await expect(restore).toContainText("已启用");
  await confirmConfiguration(restore, "确认恢复");
  await expect(touch(page)).toContainText("已启用");
  expect(await writes(page)).toEqual(["apply_touchpad", "apply_microphones", "restore_touchpad"]);
});

test("某项提交失败即停止，后续草稿仍可重新预览", async ({ page }) => {
  await openWorkspace(page, "键盘与输入");
  await region(page, "键盘功能").getByRole("switch", { name: "Fn 锁", exact: true }).check();
  await touch(page).getByRole("radio", { name: "关闭", exact: true }).check();
  await page.evaluate(() => { window.__uiFixture.keyboardFault = "隔离故障，已补偿"; });
  const review = await openConfigurationReview(page, "键盘功能");
  await confirmConfiguration(review);
  await expect(region(page, "键盘功能").getByRole("alert")).toContainText("隔离故障");
  expect(await writes(page)).toEqual(["apply_keyboard_settings"]);
  await expect(region(page, "键盘功能").getByRole("switch", { name: "Fn 锁", exact: true })).toBeChecked();
  await expect(touch(page).getByRole("radio", { name: "关闭", exact: true })).toBeChecked();
  await expect(restoreButton(page)).toBeVisible();
});

test("底层修订变化后拒绝提交已打开的预览", async ({ page }) => {
  await openWorkspace(page, "键盘与输入");
  await touch(page).getByRole("radio", { name: "关闭", exact: true }).check();
  const dialog = await openConfigurationReview(page, "触摸板");
  await page.evaluate(() => { window.__uiFixture.touchpad.revision = "touchpad-updated"; });
  await expect(dialog.getByRole("alert")).toContainText("设置已变化", { timeout: 10_000 });
  await expect(dialog.getByRole("button", { name: "确认应用", exact: true })).toBeDisabled();
  expect(await writes(page)).toEqual([]);
  await page.keyboard.press("Escape");
  await expect(touch(page).getByRole("radio", { name: "关闭", exact: true })).toBeChecked();
  await confirmConfiguration(await openConfigurationReview(page, "触摸板"));
  expect(await writes(page)).toEqual(["apply_touchpad"]);
});

test("显示刷新率与方向各自试用且保留另一项草稿", async ({ page }) => {
  await openWorkspace(page, "显示与显卡");
  const screen = region(page, "屏幕显示");
  await screen.getByRole("combobox", { name: "目标刷新率" }).click();
  await page.getByRole("option", { name: "144 Hz" }).click();
  await screen.getByRole("switch", { name: "翻转主屏" }).check();
  const review = await openConfigurationReview(page, "屏幕刷新率");
  await expect(review.getByRole("button", { name: "屏幕方向", exact: true })).toBeVisible();
  expect(await writes(page)).toEqual([]);
  await confirmConfiguration(review, "试用 144 Hz");
  await expect(screen.getByRole("button", { name: "保留刷新率" })).toBeEnabled();
  await screen.getByRole("button", { name: "保留刷新率" }).click();
  const rotation = await openConfigurationReview(page, "屏幕方向");
  await expect(rotation.getByLabel("显示旋转预览")).toContainText("横向（翻转）");
  expect(await writes(page)).toEqual(["apply_display_refresh"]);
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
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
});
