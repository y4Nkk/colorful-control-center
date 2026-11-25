import { expect, test } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { hotkeyRestoreButton, openHotkeyDetails, openWorkspace, previewHotkeyRestore } from "./workspace.mjs";

const calls = page => page.evaluate(() => window.__uiFixture.calls
  .filter(call => /^restore_/.test(call.command)).map(call => [call.command, call.args.expectedRevision]));
const option = (dialog, name) => dialog.getByRole("checkbox", { name, exact: true });
const confirm = dialog => dialog.getByRole("button", { name: "确认恢复", exact: true });

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
});

async function prepareHotkeyRecovery(page) {
  await openWorkspace(page, "键盘与灯光");
  await expect(page.getByRole("button", { name: "配置热键", exact: true })).toBeEnabled();
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, { batteryAlert, screenBrightness } = h.facts;
    h.current = { enabled: true, actions: ["screenBrightness", "batteryAlert"], osd: true };
    h.initial = { enabled: false, actions: [], osd: false };
    h.authorized = true; h.status.running = true; h.restoreRevision = "confirmed-policy";
    screenBrightness.initial = [{ plan: screenBrightness.current.plan, original: [90, 55], owned: [true, true],
      current: [...screenBrightness.current.levels], error: null }];
    screenBrightness.restoreRevision = "confirmed-brightness";
    batteryAlert.codec.initial = structuredClone(batteryAlert.codec.current);
    batteryAlert.codec.current.muted = false;
    batteryAlert.codec.restoreRevision = "confirmed-audio";
    window.__uiEmit("hardware-controls-changed", null);
  });
}

test("一个红色入口，默认不勾选，只执行选中项及已确认的修订", async ({ page }) => {
  await prepareHotkeyRecovery(page);
  const trigger = hotkeyRestoreButton(page);
  await expect(trigger).toHaveCount(1);
  const details = await openHotkeyDetails(page);
  await expect(details.getByRole("button", { name: /^恢复/ })).toHaveCount(0);
  const dialog = await previewHotkeyRestore(page);
  await expect(trigger).toHaveAttribute("data-tone", "danger");
  await expect(confirm(dialog)).toHaveAttribute("data-tone", "danger");
  await expect(confirm(dialog)).toBeDisabled();
  await expect(dialog.getByRole("checkbox")).toHaveCount(3);
  await option(dialog, "初始电池音频").check();
  await option(dialog, "初始热键策略").check();
  await expect(dialog).toContainText("接管前：静音");
  await expect(dialog).toContainText("恢复初始策略");
  await dialog.getByRole("button", { name: "取消", exact: true }).click();
  await expect(trigger).toBeFocused();
  expect(await calls(page)).toEqual([]);
  await previewHotkeyRestore(page);
  await expect(confirm(dialog)).toBeDisabled();
  await option(dialog, "初始热键策略").check();
  await option(dialog, "初始电池音频").check();
  await confirm(dialog).click();
  await expect.poll(() => calls(page)).toEqual([
    ["restore_battery_alert_audio", "confirmed-audio"], ["restore_hotkeys", "confirmed-policy"],
  ]);
  const result = await page.evaluate(() => window.__uiFixture.hotkeys);
  expect(result.current.enabled).toBe(false);
  expect(result.facts.screenBrightness.current.levels).toEqual([80, 60]);
});

test("部分失败仍回读已恢复设备，报告失败并暂缓恢复监听策略", async ({ page }) => {
  await prepareHotkeyRecovery(page);
  await page.evaluate(() => { window.__uiFixture.batteryAlertRestoreFault = "音频恢复失败"; });
  const dialog = await previewHotkeyRestore(page);
  for (const name of ["初始电池音频", "初始屏幕计划亮度", "初始热键策略"]) await option(dialog, name).check();
  await confirm(dialog).click();
  const details = await openHotkeyDetails(page);
  await expect(details).toContainText("已恢复 1 项。");
  await expect(details).toContainText("音频恢复失败");
  await expect(details).toContainText("再恢复监听策略");
  expect(await calls(page)).toEqual([
    ["restore_battery_alert_audio", "confirmed-audio"], ["restore_screen_brightness", "confirmed-brightness"],
  ]);
  await expect(details).toContainText("计划插电亮度90%");
});

test("涉及同一电源计划的两个热键亮度恢复目标互斥", async ({ page }) => {
  await prepareHotkeyRecovery(page);
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, low = h.facts.batteryLowBrightness;
    low.recovery = { record: { levelsOwned: [true, true], baseline: structuredClone(low.current) }, levels: [80, 60] };
    low.restoreRevision = "confirmed-low-brightness";
    window.__uiEmit("hardware-controls-changed", null);
  });
  const dialog = await previewHotkeyRestore(page);
  const screen = option(dialog, "初始屏幕计划亮度"), low = option(dialog, "低电量前亮度");
  await expect(dialog).toContainText("两个亮度目标涉及同一个电源计划");
  await screen.check();
  await expect(low).toBeDisabled();
  await screen.uncheck();
  await low.check();
  await expect(screen).toBeDisabled();
  await expect(option(dialog, "初始电池音频")).toBeEnabled();
  expect(await calls(page)).toEqual([]);
});

test("风扇恢复使用确认前的修订，旧修订被拒绝并回读", async ({ page }) => {
  await openWorkspace(page, "性能与散热");
  const trigger = page.getByRole("button", { name: "恢复设置", exact: true });
  const dialog = page.getByRole("dialog", { name: "恢复设置", exact: true });
  const initial = option(dialog, "初始风扇设置");
  await trigger.click();
  await initial.check();
  await page.evaluate(() => { window.__uiFixture.fanRestoreGeneration = 1; });
  await confirm(dialog).click();
  const error = page.getByRole("alert").filter({ hasText: "风扇恢复目标已变化" });
  await expect(error).toBeVisible();
  await trigger.click();
  await initial.check();
  await confirm(dialog).click();
  await expect(error).toHaveCount(0);
  const [stale, fresh] = await page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "restore_fan_settings"));
  expect(stale.args.request.target).toBe("initial");
  expect(fresh.args.request.target).toBe("initial");
  expect(stale.args.request.expectedRevision).not.toBe(fresh.args.request.expectedRevision);
});
