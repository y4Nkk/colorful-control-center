import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { editHotkeys, finishHotkeyEdit, openHotkeyDetails, openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const facts = page => page.getByRole("region", { name: "原厂电池音频事实与预览", exact: true });
const scope = editor => editor.getByRole("checkbox", { name: "原厂电池音频事件", exact: true });
const lowBrightness = editor => editor.getByRole("checkbox", { name: "原厂低电量亮度", exact: true });
const brightness = page => page.getByRole("region", { name: "原厂低电量亮度事实与预览", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与灯光"); });
async function applied(page) {
  await expect(panel(page).getByRole("button", { name: "配置热键", exact: true })).toBeEnabled();
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, alert = h.facts.batteryAlert;
    alert.codec.initial = structuredClone(alert.codec.current); alert.codec.current.muted = false;
    alert.codec.restoreRevision = "audio-restore-0"; alert.playing = true;
    h.current = { enabled: true, actions: ["batteryAlert"], osd: true }; h.authorized = true;
    h.status.running = true; h.revision = "alert-active"; window.__uiEmit("hardware-controls-changed", null);
  });
  const details = await openHotkeyDetails(page);
  await expect(facts(page).getByRole("button", { name: "恢复初始电池音频", exact: true })).toBeEnabled();
  return details;
}
test("独立范围预览与应用，绝对亮度保留真实目标", async ({ page }) => {
  const editor = await editHotkeys(page);
  await scope(editor).check();
  await expect(lowBrightness(editor)).not.toBeChecked();
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await finishHotkeyEdit(editor);
  const details = await openHotkeyDetails(page);
  await expect(facts(page)).toBeVisible();
  await expect(brightness(page)).toContainText("事件指定亮度");
  await expect(brightness(page).getByRole("meter")).toHaveCount(2);
  await details.getByRole("button", { name: "关闭弹窗", exact: true }).click();
  await previewConfiguration(page, "原厂热键与屏显");
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
  await configurationView(page).getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  expect((await calls(page, "apply_hotkeys"))[0].args.request.settings.actions).toEqual(["batteryAlert"]);
});
test("初始codec恢复保留二次确认，失败后记录仍可重试", async ({ page }) => {
  await applied(page);
  await expect(facts(page)).toContainText("未静音");
  await expect(facts(page)).toContainText("接管前：静音");
  await page.evaluate(() => { window.__uiFixture.batteryAlertRestoreFault = "隔离音频恢复失败"; });
  for (let attempt = 0; attempt < 2; attempt++) {
    await facts(page).getByRole("button", { name: "恢复初始电池音频", exact: true }).click();
    if (!attempt) expect(await calls(page, "restore_battery_alert_audio")).toEqual([]);
    await facts(page).getByRole("button", { name: "停止监听并恢复初始电池音频？", exact: true }).click();
    if (!attempt) await expect(page.getByText("隔离音频恢复失败", { exact: true }).first()).toBeVisible();
  }
  expect(await calls(page, "restore_battery_alert_audio")).toHaveLength(2);
  await expect(facts(page)).toContainText("已停止");
  await expect(facts(page).getByRole("button", { name: "恢复初始电池音频", exact: true })).toHaveCount(0);
});
test("设备变化保留原控件恢复信息并禁止覆盖", async ({ page }) => {
  await applied(page);
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, alert = h.facts.batteryAlert;
    alert.identity = null; alert.error = "PC Beep设备已变化";
    alert.codec.restoreRevision = null; alert.codec.restoreError = "保留原设备恢复值";
    const cap = h.facts.capabilities.find(c => c.scope === "batteryAlert"); cap.identity = null; cap.reason = alert.error;
    h.revision = "alert-replaced"; window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(facts(page)).toContainText("保留原设备恢复值");
  await expect(facts(page).getByRole("button", { name: "恢复初始电池音频", exact: true })).toBeDisabled();
  expect(await calls(page, "restore_battery_alert_audio")).toEqual([]);
});
test("两个范围共用亮度事实与恢复，音频离散状态不画比例", async ({ page }) => {
  const editor = await editHotkeys(page);
  await scope(editor).check();
  await lowBrightness(editor).check();
  await finishHotkeyEdit(editor);
  await openHotkeyDetails(page);
  await expect(brightness(page)).toHaveCount(1);
  expect(await facts(page).getByRole("meter").count()).toBe(0);
  expect(await facts(page).locator("svg[data-icon]").count()).toBeGreaterThanOrEqual(2);
  expect(await calls(page, "restore_low_battery_brightness")).toEqual([]);
});
test("深浅宽窄布局保留左右模块边框和间隔", async ({ page }) => {
  await applied(page);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 900 }); await facts(page).scrollIntoViewIfNeeded();
      const result = await facts(page).evaluate(el => {
        const columns = el.querySelector(".ccc-battery-low-columns"), parts = [...columns.children];
        const [a, b] = parts.map(part => part.getBoundingClientRect());
        return { overflow: el.scrollWidth > el.clientWidth + 1, borders: parts.every(part => parseFloat(getComputedStyle(part).borderTopWidth) > 0),
          gap: parseFloat(getComputedStyle(columns).gap), fits: el.getBoundingClientRect().right <= innerWidth + 1,
          layout: el.clientWidth >= 380 ? b.left >= a.right - 1 : b.top >= a.bottom - 1 };
      });
      expect(result.overflow).toBe(false); expect(result.borders).toBe(true); expect(result.fits).toBe(true);
      expect(result.gap).toBeGreaterThan(0); expect(result.layout).toBe(true);
      if (width !== 760) await facts(page).screenshot({ path: `target/ui-check/battery-alert-${theme}-${width}.png` });
    }
  }
});

test("无低电量调暗能力时仍可单独预览绝对亮度事件", async ({ page }) => {
  const editor = await editHotkeys(page);
  await scope(editor).check();
  await finishHotkeyEdit(editor);
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, alert = h.facts.batteryAlert;
    alert.binding.lowControl = false; alert.brightness.current.rule.feature = false;
    const low = h.facts.capabilities.find(c => c.scope === "batteryLowBrightness");
    low.identity = null; low.reason = "固件未声明低电量亮度控制";
    h.revision = "absolute-without-low"; window.__uiEmit("hardware-controls-changed", null);
  });
  const details = await openHotkeyDetails(page);
  await expect(details.getByLabel("不可用的热键动作")).toContainText("固件未声明低电量亮度控制");
  await expect(brightness(page)).toContainText("事件指定亮度");
  await expect(brightness(page)).not.toContainText("还会再调暗");
  await details.getByRole("button", { name: "关闭弹窗", exact: true }).click();
  await editHotkeys(page);
  await expect(scope(editor)).toBeEnabled();
  await expect(lowBrightness(editor)).toHaveCount(0);
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
});

test("监听已结束但循环停止失败时仍能重试清理", async ({ page }) => {
  const details = await applied(page);
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys; h.status.running = false;
    window.__uiFixture.batteryAlertStopFault = "隔离播放器停止失败";
    h.revision = "listener-ended-player-owned"; window.__uiEmit("hardware-controls-changed", null);
  });
  const stop = details.getByRole("button", { name: "临时停止监听", exact: true });
  await expect(stop).toBeEnabled(); await stop.click();
  await expect(page.getByText("隔离播放器停止失败", { exact: true }).first()).toBeVisible();
  await expect(facts(page)).toContainText("运行中");
  await stop.click();
  await expect(facts(page)).toContainText("已停止");
  await expect(facts(page).getByRole("button", { name: "恢复初始电池音频", exact: true })).toHaveCount(0);
  expect(await calls(page, "stop_hotkeys")).toHaveLength(2);
});
