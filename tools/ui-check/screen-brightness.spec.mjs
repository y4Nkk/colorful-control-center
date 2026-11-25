import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { editHotkeys, finishHotkeyEdit, hotkeyRestoreButton, hotkeyRestoreDialog, openHotkeyDetails, openWorkspace, previewHotkeyRestore } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const facts = page => page.getByRole("region", { name: "屏幕亮度策略事实", exact: true });
const scope = editor => editor.getByRole("checkbox", { name: "原厂屏幕亮度策略", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(c => c.command === command), command);
async function initial(page) {
  await expect(panel(page).getByRole("button", { name: "配置热键", exact: true })).toBeEnabled();
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, s = h.facts.screenBrightness;
    s.initial = [{ plan: s.current.plan, original: [90, 55], owned: [true, true], current: [...s.current.levels], error: null }];
    s.restoreRevision = "screen-restore-0";
    h.current = { enabled: true, actions: ["screenBrightness"], osd: true };
    h.authorized = true; h.status.running = true; h.revision = "with-screen-baseline";
    window.__uiEmit("hardware-controls-changed", null);
  });
  const details = await openHotkeyDetails(page);
  await expect(facts(page)).toContainText("恢复接管前的值");
  return details;
}
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与灯光"); });

test("实际亮度与插电电池保存值分开，完整目标及零档在确认前可见", async ({ page }) => {
  const details = await openHotkeyDetails(page);
  await expect(facts(page)).toHaveCount(0);
  await details.getByRole("button", { name: "关闭弹窗", exact: true }).click();
  const editor = await editHotkeys(page);
  await expect(scope(editor)).not.toBeChecked();
  await scope(editor).check();
  await expect(scope(editor)).toBeChecked();
  await finishHotkeyEdit(editor);
  await openHotkeyDetails(page);
  await expect(facts(page).getByRole("meter", { name: "实际亮度", exact: true })).toHaveAttribute("aria-valuenow", "70");
  await expect(facts(page)).toContainText("计划插电亮度80%");
  await expect(facts(page)).toContainText("计划电池亮度60%");
  await expect(facts(page).getByLabel("原厂屏幕亮度触发目标")).toContainText("第11项 0%");
  await expect(facts(page)).toContainText("目标里有 0%");
  const changing = await page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply_|restore_|set_)/.test(c.command) && c.command !== "set_shortcut_recording"));
  expect(changing).toEqual([]);
});

test("屏幕策略独立授权并确认，键盘亮度和原厂反馈不会自动包含", async ({ page }) => {
  const editor = await editHotkeys(page);
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  for (const title of ["键盘亮度", "原厂亮度循环与反馈", "原厂状态通知"]) {
    const checkbox = editor.getByRole("checkbox", { name: title, exact: true });
    if (await checkbox.count()) { await checkbox.check(); await expect(scope(editor)).not.toBeChecked(); await checkbox.uncheck(); }
  }
  await scope(editor).check();
  await finishHotkeyEdit(editor);
  const preview = await previewConfiguration(page, "原厂热键与屏显");
  await expect(preview).toContainText("同步修改活动计划的插电和电池亮度保存值");
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
  await configurationView(page).getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  expect((await calls(page, "apply_hotkeys"))[0].args.request.settings.actions).toEqual(["screenBrightness"]);
  expect(await calls(page, "restore_screen_brightness")).toEqual([]);
});

test("初始亮度逐字段预览并独立恢复，停止监听但保留已保存的软件范围", async ({ page }) => {
  await initial(page);
  await expect(facts(page)).toContainText("80% → 90%");
  await expect(facts(page)).toContainText("60% → 55%");
  const restore = await previewHotkeyRestore(page, "初始屏幕计划亮度");
  await expect(restore).toContainText("60% → 55%");
  expect(await calls(page, "restore_screen_brightness")).toEqual([]);
  await restore.getByRole("button", { name: "确认恢复", exact: true }).click();
  expect((await calls(page, "restore_screen_brightness"))[0].args).toEqual({ expectedRevision: "screen-restore-0" });
  const details = await openHotkeyDetails(page);
  await expect(details).toContainText("热键监听已停止");
  await expect(facts(page)).toContainText("计划插电亮度90%");
  await expect(facts(page).getByRole("meter", { name: "实际亮度", exact: true })).toHaveAttribute("aria-valuenow", "70");
  await details.getByRole("button", { name: "关闭弹窗", exact: true }).click();
  await expect(scope(await editHotkeys(page))).toBeChecked();
});

test("目标报告失败不冒充零亮度，已知初始值仍能独立恢复", async ({ page }) => {
  await initial(page);
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, s = h.facts.screenBrightness;
    s.current = null; s.identity = null; s.error = "原厂目标报告读取失败";
    const cap = h.facts.capabilities.find(c => c.scope === "screenBrightness");
    cap.identity = null; cap.reason = s.error;
    h.revision = "screen-report-failed";
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(facts(page)).toContainText("本次屏幕亮度报告不可用");
  await expect(facts(page).getByRole("meter", { name: "实际亮度", exact: true })).toHaveCount(0);
  const restore = await previewHotkeyRestore(page, "初始屏幕计划亮度");
  await restore.getByRole("button", { name: "确认恢复", exact: true }).click();
  expect(await calls(page, "restore_screen_brightness")).toHaveLength(1);
});

test("来源或初始恢复预览变化撤销旧确认，失败后保留记录并可重试", async ({ page }) => {
  await initial(page);
  await previewHotkeyRestore(page, "初始屏幕计划亮度");
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys;
    h.facts.screenBrightness.restoreRevision = "screen-restore-new"; h.revision = "changed-screen";
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(hotkeyRestoreDialog(page)).toHaveCount(0);
  await expect(hotkeyRestoreButton(page)).toBeFocused();
  await page.evaluate(() => { window.__uiFixture.screenBrightnessRestoreFault = "初始亮度电池字段恢复失败"; });
  for (let attempt = 0; attempt < 2; attempt++) {
    const restore = await previewHotkeyRestore(page, "初始屏幕计划亮度");
    await restore.getByRole("button", { name: "确认恢复", exact: true }).click();
    if (attempt === 0) await expect(await openHotkeyDetails(page)).toContainText("初始亮度电池字段恢复失败");
  }
  await openHotkeyDetails(page);
  await expect(facts(page)).toContainText("90% → 90%");
});

test("屏幕亮度事实在深浅宽窄布局保持卡片间隔且不溢出", async ({ page }) => {
  await initial(page);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 900 }); await facts(page).scrollIntoViewIfNeeded();
      const box = await facts(page).evaluate(el => {
        const ring = el.querySelector(".ccc-meter-ring"), meter = el.querySelector(".ccc-meter"), values = el.querySelector(".ccc-facts");
        const probe = document.createElement("span"); probe.style.color = "var(--ccc-action)"; el.append(probe);
        const correctColor = getComputedStyle(ring).color === getComputedStyle(probe).color; probe.remove();
        const m = meter.getBoundingClientRect(), v = values.getBoundingClientRect();
        return { overflow: el.scrollWidth > el.clientWidth + 1, right: el.getBoundingClientRect().right,
          border: parseFloat(getComputedStyle(el.closest(".ccc-hotkey-section")).borderTopWidth),
          gap: parseFloat(getComputedStyle(el.querySelector(".ccc-screen-brightness-current")).gap), correctColor,
          layout: el.clientWidth >= 320 ? v.left >= m.right - 1 : v.top >= m.bottom - 1 };
      });
      expect(box.overflow).toBe(false); expect(box.right).toBeLessThanOrEqual(width);
      expect(box.border).toBeGreaterThan(0); expect(box.gap).toBeGreaterThan(0);
      expect(box.correctColor).toBe(true); expect(box.layout).toBe(true);
      if (width !== 760) await page.screenshot({ path: `target/ui-check/screen-brightness-${theme}-${width}.png` });
    }
  }
});
