import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const facts = page => page.getByRole("region", { name: "原厂屏幕按键事实", exact: true });
const scope = page => panel(page).getByRole("checkbox", { name: "原厂屏幕按键", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(c => c.command === command), command);
async function owned(page, kind = "pointerWake") {
  await page.evaluate(kind => {
    const h = window.__uiFixture.hotkeys, p = h.facts.panelHotkey;
    p.session = { id: 1, identity: p.identity, kind, preparing: false, error: null }; p.restoreRevision = "panel-restore-1";
    h.current = { enabled: true, actions: ["panelHotkey"], osd: true }; h.authorized = true; h.status.running = true; h.revision = "with-panel-session";
    window.__uiEmit("hardware-controls-changed", null);
  }, kind);
  await expect(facts(page).getByRole("button", { name: "结束本软件屏幕会话", exact: true })).toBeEnabled();
}
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与输入"); });

test("适用行为在授权前可见，读取和选择范围不会启动会话", async ({ page }) => {
  await expect(scope(page)).not.toBeChecked();
  await expect(facts(page)).toHaveCount(0);
  await scope(page).check();
  await expect(facts(page)).toContainText("一次性输入唤醒");
  await expect(facts(page)).toContainText("首个键盘按下结束等待");
  await expect(scope(page)).toBeChecked();
  await scope(page).check();
  await expect(panel(page).getByLabel("屏幕按键接管预览")).toContainText("鼠标活动发送一次唤醒请求");
  expect(await calls(page, "apply_hotkeys")).toEqual([]); expect(await calls(page, "end_panel_hotkey")).toEqual([]);
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys; h.facts.panelHotkey.current.kind = "blankSaver";
    h.revision = "oled-source"; window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(facts(page)).toContainText("已匹配原厂 OLED 条件");
  await expect(panel(page).getByLabel("屏幕按键接管预览")).toContainText("按键启动空白屏保，正常输入退出");
  await expect(facts(page)).toContainText("不表示面板已断电");
});
test("屏幕按键独立授权并确认，不继承触摸板或亮度范围", async ({ page }) => {
  const p = panel(page); await p.getByRole("checkbox", { name: "启用原厂热键接管", exact: true }).check();
  for (const label of ["触摸板开关", "原厂屏幕亮度策略", "原厂状态通知"]) {
    const checkbox = p.getByRole("checkbox", { name: label, exact: true }); await checkbox.check();
    await expect(scope(page)).not.toBeChecked(); await checkbox.uncheck();
  }
  await scope(page).check(); await p.getByRole("button", { name: "应用热键策略", exact: true }).click();
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
  await p.getByRole("button", { name: "确认此范围的热键接管？", exact: true }).click();
  expect((await calls(page, "apply_hotkeys"))[0].args.request.settings.actions).toEqual(["panelHotkey"]);
  await expect(facts(page)).toContainText("未运行");
});
test("只结束本软件会话，确认后停止监听并保留保存的接管范围", async ({ page }) => {
  await owned(page, "blankSaver");
  await facts(page).getByRole("button", { name: "结束本软件屏幕会话", exact: true }).click();
  expect(await calls(page, "end_panel_hotkey")).toEqual([]);
  await facts(page).getByRole("button", { name: "确认结束会话并停止监听？", exact: true }).click();
  expect((await calls(page, "end_panel_hotkey"))[0].args).toEqual({ expectedRevision: "panel-restore-1" });
  await expect(facts(page)).toContainText("未运行"); await expect(scope(page)).toBeChecked();
  await expect(panel(page)).toContainText("热键监听已停止");
});
test("当前来源读取失败仍能结束已有会话，未选择的未知范围禁止接管", async ({ page }) => {
  await owned(page);
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, p = h.facts.panelHotkey;
    p.current = null; p.identity = null; p.error = "面板身份未完整确认";
    const cap = h.facts.capabilities.find(c => c.scope === "panelHotkey"); cap.identity = null; cap.reason = p.error;
    h.revision = "unknown-panel"; window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(facts(page)).toContainText("本次适用行为不可用");
  await expect(facts(page).getByRole("button", { name: "结束本软件屏幕会话", exact: true })).toBeEnabled();
  await facts(page).getByRole("button", { name: "结束本软件屏幕会话", exact: true }).click();
  await facts(page).getByRole("button", { name: "确认结束会话并停止监听？", exact: true }).click();
  await scope(page).uncheck(); await expect(scope(page)).toBeDisabled();
});
test("新会话撤销旧确认，结束失败保留事实并允许重试", async ({ page }) => {
  await owned(page); await facts(page).getByRole("button", { name: "结束本软件屏幕会话", exact: true }).click();
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, p = h.facts.panelHotkey;
    p.session.id = 2; p.restoreRevision = "panel-restore-2"; h.revision = "new-session";
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(facts(page).getByRole("button", { name: "确认结束会话并停止监听？", exact: true })).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.panelHotkeyEndFault = "结束本软件屏幕会话失败"; });
  for (let attempt = 0; attempt < 2; attempt++) {
    await facts(page).getByRole("button", { name: "结束本软件屏幕会话", exact: true }).click();
    await facts(page).getByRole("button", { name: "确认结束会话并停止监听？", exact: true }).click();
    if (!attempt) await expect(page.getByText("结束本软件屏幕会话失败", { exact: true }).first()).toBeVisible();
  }
  expect((await calls(page, "end_panel_hotkey")).map(c => c.args.expectedRevision)).toEqual(["panel-restore-2", "panel-restore-2"]);
  await expect(facts(page)).toContainText("未运行");
});
test("深浅宽窄布局保留模块边框间隔，宽区域内部左右布局", async ({ page }) => {
  await owned(page);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 900 }); await facts(page).scrollIntoViewIfNeeded();
      const box = await facts(page).evaluate(el => {
        const columns = el.querySelector(".ccc-panel-hotkey-columns"), parts = [...columns.children];
        const [left, right] = parts.map(p => p.getBoundingClientRect());
        return { overflow: el.scrollWidth > el.clientWidth + 1, right: el.getBoundingClientRect().right,
          borders: parts.every(p => parseFloat(getComputedStyle(p).borderTopWidth) > 0), gap: parseFloat(getComputedStyle(columns).gap),
          layout: el.clientWidth >= 380 ? right.left >= left.right - 1 : right.top >= left.bottom - 1 };
      });
      expect(box.overflow).toBe(false); expect(box.right).toBeLessThanOrEqual(width);
      expect(box.borders).toBe(true); expect(box.gap).toBeGreaterThan(0); expect(box.layout).toBe(true);
      if (width !== 760) await facts(page).screenshot({ path: `target/ui-check/panel-hotkey-${theme}-${width}.png` });
    }
  }
});
