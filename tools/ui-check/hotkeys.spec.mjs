import { availableAction, previewConfiguration, submitConfiguration } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply_hotkeys|restore_hotkeys)$/.test(c.command)));
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与灯光"); });

test("低电量亮度与动作共用相同读数，独立采样不一致时各自显示", async ({ page }) => {
  const region = panel(page);
  await region.getByRole("button", { name: "配置热键", exact: true }).click();
  const editor = page.getByRole("dialog", { name: "配置热键与屏显", exact: true });
  for (const name of ["原厂低电量亮度", "原厂低电量系统动作"])
    await editor.getByRole("checkbox", { name, exact: true }).check();
  await editor.getByRole("button", { name: "完成编辑", exact: true }).click();
  const read = async (actionPercent) => {
    await page.evaluate(actionPercent => {
      const { batteryLowBrightness, batteryLowAction } = window.__uiFixture.hotkeys.facts;
      Object.assign(batteryLowBrightness, { onAc: false, percent: 21 });
      Object.assign(batteryLowAction, { onAc: false, percent: actionPercent });
    }, actionPercent);
    await region.getByRole("button", { name: "重新读取热键", exact: true }).click();
    await region.getByRole("button", { name: "详情", exact: true }).click();
    return page.getByRole("dialog", { name: "热键状态与诊断", exact: true });
  };
  const details = await read(21);
  await expect(details.getByText("当前电池供电 · 电量 21%", { exact: true })).toHaveCount(1);
  await details.getByRole("button", { name: "关闭弹窗", exact: true }).click();
  await read(20);
  await expect(details.getByLabel("原厂低电量亮度事实与预览")).toContainText("当前电池供电 · 电量 21%");
  await expect(details.getByLabel("原厂低电量系统动作事实与预览")).toContainText("当前电池供电 · 电量 20%");
  expect(await writes(page)).toHaveLength(0);
});

async function configure(page) {
  await panel(page).getByRole("button", { name: "配置热键", exact: true }).click();
  return page.getByRole("dialog", { name: "配置热键与屏显", exact: true });
}
const finish = editor => editor.getByRole("button", { name: "完成编辑", exact: true }).click();
async function diagnostics(page) {
  await panel(page).getByRole("button", { name: "详情", exact: true }).click();
  return page.getByRole("dialog", { name: "热键状态与诊断", exact: true });
}
async function apply(page) {
  await submitConfiguration(await previewConfiguration(page, "原厂热键与屏显"));
}

test("接管范围在所属卡片授权，桌面提示直接提交", async ({ page }) => {
  const region = panel(page), editor = await configure(page);
  const enabled = editor.getByRole("switch", { name: "启用原厂热键接管", exact: true });
  await enabled.check();
  await expect(editor).toContainText("请选择要接管的动作");
  await editor.getByRole("checkbox", { name: "Fn 锁切换", exact: true }).check();
  const osd = editor.getByRole("switch", { name: "显示桌面提示", exact: true });
  await osd.click();
  await expect(osd).not.toBeChecked();
  expect((await writes(page))[0].args.request.settings).toEqual({ enabled: false, osd: false, actions: [] });
  await expect(enabled).toBeChecked();
  await finish(editor);
  const review = await previewConfiguration(page, "原厂热键与屏显");
  await expect(review).toContainText("只切换 Fn 锁");
  expect(await writes(page)).toHaveLength(1);
  await submitConfiguration(review);
  await expect(region).toContainText("正在监听");
  const details = await diagnostics(page);
  await details.getByRole("button", { name: "临时停止监听", exact: true }).click();
  await expect(details).toContainText("热键监听已停止");
  await page.keyboard.press("Escape");
  await expect(availableAction(page, "原厂热键与屏显")).toBeEnabled();
  await submitConfiguration(await previewConfiguration(page, "原厂热键与屏显", true), "确认恢复");
  await configure(page);
  await expect(enabled).not.toBeChecked();
});
test("读取失败撤销快照，重读后保留编辑的授权范围", async ({ page }) => {
  const region = panel(page), editor = await configure(page);
  await editor.getByRole("checkbox", { name: "键盘亮度", exact: true }).check();
  await finish(editor);
  await page.evaluate(() => { window.__uiFixture.hotkeysReadFault = "隔离策略损坏"; });
  await region.getByRole("button", { name: "重新读取热键", exact: true }).click();
  const details = await diagnostics(page);
  await expect(details).toContainText("隔离策略损坏");
  await expect(availableAction(page, "原厂热键与屏显")).toHaveCount(0);
  await details.getByRole("button", { name: "临时停止监听", exact: true }).click();
  expect(await page.evaluate(() => window.__uiFixture.calls.some(c => c.command === "stop_hotkeys"))).toBe(true);
  await page.keyboard.press("Escape");
  await page.evaluate(() => { window.__uiFixture.hotkeysReadFault = null; });
  await region.getByRole("button", { name: "重新读取热键", exact: true }).click();
  await configure(page);
  await expect(editor.getByRole("checkbox", { name: "键盘亮度", exact: true })).toBeChecked();
  expect(await writes(page)).toHaveLength(0);
});
test("授权失败保留草稿与诊断", async ({ page }) => {
  const editor = await configure(page);
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await editor.getByRole("checkbox", { name: "Fn 锁切换", exact: true }).check();
  await finish(editor);
  await page.evaluate(() => { window.__uiFixture.hotkeysFault = "隔离订阅失败，原策略已恢复"; });
  await apply(page);
  await expect(await diagnostics(page)).toContainText("隔离订阅失败");
  await page.keyboard.press("Escape");
  await configure(page);
  await expect(editor.getByRole("checkbox", { name: "Fn 锁切换", exact: true })).toBeChecked();
});
test("热键刷新和手动按钮分别回读当前键盘状态", async ({ page }) => {
  await openWorkspace(page, "设备配置");
  const keyboard = page.getByRole("region", { name: "键盘功能", exact: true });
  const fn = keyboard.getByRole("switch", { name: "Fn 锁", exact: true });
  const win = keyboard.getByRole("switch", { name: "Win 锁", exact: true });
  await expect(fn).not.toBeChecked();
  await page.evaluate(() => { window.__uiFixture.keyboardSettings.current.fnLocked = true; window.__uiEmit("hardware-controls-changed", null); });
  await expect(fn).toBeChecked();
  await win.click();
  await expect(win).toBeChecked();
  await page.evaluate(() => { window.__uiFixture.keyboardSettings.current.fnLocked = false; window.__uiEmit("hardware-controls-changed", null); });
  await expect(fn).not.toBeChecked();
  await expect(win).toBeChecked();
});
test("屏显独立入口只读取提示，旧消息不能覆盖新消息", async ({ page }) => {
  await page.goto("/?surface=osd");
  await expect(page.getByRole("region", { name: "原厂热键与屏显", exact: true })).toHaveCount(0);
  const state = (sequence, revision, title, visible = true) => ({ sequence, revision, message: { title, detail: "隔离示例", error: false, level: { value: 3, max: 4 } },
    visible, appearance: null, appearanceError: null, error: null });
  await page.evaluate(value => window.__uiEmit("osd-state", value), state(2, 5, "最新提示"));
  await expect(page.getByRole("status")).toContainText("最新提示");
  await page.setViewportSize({ width: 380, height: 120 });
  await page.screenshot({ path: "target/ui-check/osd.png", animations: "disabled" });
  const unexpected = await page.evaluate(() => window.__uiFixture.calls.filter(call => !["plugin:event|listen", "plugin:event|unlisten", "read_osd"].includes(call.command)));
  expect(unexpected).toEqual([]);
  await page.evaluate(value => window.__uiEmit("osd-state", value), state(1, 3, "旧提示"));
  await expect(page.getByRole("status")).toContainText("最新提示");
  await page.evaluate(value => window.__uiEmit("osd-state", value), state(2, 6, "最新提示", false));
  await expect(page.getByRole("status")).toHaveCount(0);
  await page.evaluate(value => window.__uiEmit("osd-state", value), state(2, 5, "过期显示"));
  await expect(page.getByRole("status")).toHaveCount(0);
});
test("热键卡片和配置弹窗在宽窄窗口保持边界", async ({ page }) => {
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 900 });
      const region = panel(page);
      await region.scrollIntoViewIfNeeded();
      expect(await region.evaluate(el => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
      const editor = await configure(page);
      await expect(editor.getByRole("checkbox", { name: "原厂状态通知", exact: true })).toBeVisible();
      expect(await editor.evaluate(el => el.getBoundingClientRect().right <= innerWidth + 1)).toBe(true);
      await finish(editor);
    }
  }
});
test("键盘灯开关独立授权，亮度策略不隐式包含开关", async ({ page }) => {
  const editor = await configure(page);
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  const brightness = editor.getByRole("checkbox", { name: "键盘亮度", exact: true });
  const lamps = editor.getByRole("checkbox", { name: "键盘灯开关", exact: true });
  await brightness.check();
  await expect(lamps).not.toBeChecked();
  await finish(editor);
  expect(await writes(page)).toHaveLength(0);
  await apply(page);
  expect((await writes(page))[0].args.request.settings.actions).toEqual(["keyboardBrightness"]);
  await configure(page);
  await brightness.uncheck();
  await lamps.check();
  await finish(editor);
  await apply(page);
  expect((await writes(page))[1].args.request.settings.actions).toEqual(["keyboardSwitch"]);
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply|restore)_(keyboard_lighting|perkey_lighting|multi_zone_lighting)$/.test(call.command)))).toEqual([]);
  await submitConfiguration(await previewConfiguration(page, "原厂热键与屏显", true), "确认恢复");
  await configure(page);
  await expect(lamps).not.toBeChecked();
});

test("键盘灯能力失效撤销接管资格，其他可用范围仍可编辑", async ({ page }) => {
  const region = panel(page), editor = await configure(page);
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  const lamps = editor.getByRole("checkbox", { name: "键盘灯开关", exact: true });
  await lamps.check();
  await finish(editor);
  await previewConfiguration(page, "原厂热键与屏显");
  await page.evaluate(() => {
    const hotkeys = window.__uiFixture.hotkeys;
    Object.assign(hotkeys.facts.capabilities.find(cap => cap.scope === "keyboardSwitch"), { identity: null, reason: "隔离键盘身份变化" });
    hotkeys.revision = "switch-device-changed";
  });
  await region.getByRole("button", { name: "重新读取热键", exact: true }).click();
  await expect(availableAction(page, "原厂热键与屏显")).toHaveCount(0);
  await configure(page);
  await expect(editor).toContainText("暂不可用");
  await lamps.click();
  await expect(lamps).toHaveCount(0);
  await editor.getByRole("checkbox", { name: "唤起主窗口", exact: true }).check();
  await finish(editor);
  await expect(availableAction(page, "原厂热键与屏显")).toBeEnabled();
  expect(await writes(page)).toHaveLength(0);
});

