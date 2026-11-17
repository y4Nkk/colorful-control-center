import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply_hotkeys|restore_hotkeys)$/.test(c.command)));
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与输入"); });

test("热键接管必须选择范围并确认，临时停止与初始策略恢复分开", async ({ page }) => {
  const region = panel(page);
  await region.getByRole("checkbox", { name: "启用原厂热键接管", exact: true }).check();
  await expect(region.getByRole("button", { name: "应用热键策略", exact: true })).toBeDisabled();
  await region.getByRole("checkbox", { name: "Fn 锁切换", exact: true }).check();
  await expect(region.getByLabel("热键策略预览")).toContainText("Fn 锁切换");
  await region.getByText("已选动作说明", { exact: true }).click();
  await expect(region.getByLabel("热键策略预览").getByText(/只切换 Fn 锁/)).toBeVisible();
  expect(await writes(page)).toHaveLength(0);
  await region.getByRole("button", { name: "应用热键策略", exact: true }).click(); expect(await writes(page)).toHaveLength(0);
  await region.getByRole("button", { name: "确认此范围的热键接管？", exact: true }).click();
  await expect(region).toContainText("正在监听原厂热键");
  await region.getByRole("button", { name: "临时停止监听", exact: true }).click();
  await expect(region).toContainText("热键监听已停止");
  await expect(region.getByRole("checkbox", { name: "启用原厂热键接管", exact: true })).toBeChecked();
  await expect(region.getByRole("button", { name: "应用热键策略", exact: true })).toBeEnabled();
  await region.getByRole("button", { name: "恢复初始热键策略", exact: true }).click();
  await region.getByRole("button", { name: "确认恢复初始软件策略？", exact: true }).click();
  await expect(region.getByRole("checkbox", { name: "启用原厂热键接管", exact: true })).not.toBeChecked();
});
test("读取失败撤销执行快照，仍可停止监听并保留草稿", async ({ page }) => {
  const region = panel(page);
  await region.getByRole("checkbox", { name: "键盘亮度", exact: true }).check();
  await page.evaluate(() => { window.__uiFixture.hotkeysReadFault = "隔离策略损坏"; });
  await region.getByRole("button", { name: "重新读取热键", exact: true }).click();
  await expect(region.getByRole("alert")).toContainText("隔离策略损坏");
  await expect(region.getByRole("button", { name: "应用热键策略", exact: true })).toHaveCount(0);
  await region.getByRole("button", { name: "临时停止监听", exact: true }).click();
  expect(await page.evaluate(() => window.__uiFixture.calls.some(c => c.command === "stop_hotkeys"))).toBe(true);
  await page.evaluate(() => { window.__uiFixture.hotkeysReadFault = null; });
  await region.getByRole("button", { name: "重新读取热键", exact: true }).click();
  await expect(region.getByRole("checkbox", { name: "键盘亮度", exact: true })).toBeChecked();
  expect(await writes(page)).toHaveLength(0);
});
test("能力变化撤销旧确认，失败保留草稿与诊断", async ({ page }) => {
  const region = panel(page);
  await region.getByRole("checkbox", { name: "启用原厂热键接管", exact: true }).check();
  await region.getByRole("checkbox", { name: "Fn 锁切换", exact: true }).check();
  await region.getByRole("button", { name: "应用热键策略", exact: true }).click();
  await page.evaluate(() => { window.__uiFixture.hotkeys.revision = "external"; window.__uiEmit("hardware-controls-changed", null); });
  await expect(region.getByRole("button", { name: "应用热键策略", exact: true })).toBeVisible();
  await page.evaluate(() => { window.__uiFixture.hotkeysFault = "隔离订阅失败，原策略已恢复"; });
  await region.getByRole("button", { name: "应用热键策略", exact: true }).click();
  await region.getByRole("button", { name: "确认此范围的热键接管？", exact: true }).click();
  await expect(region.getByRole("alert")).toContainText("隔离订阅失败");
  await expect(region.getByRole("checkbox", { name: "Fn 锁切换", exact: true })).toBeChecked();
});
test("热键刷新更新干净的键盘状态，保留未提交的修改", async ({ page }) => {
  const keyboard = page.getByRole("region", { name: "键盘功能", exact: true });
  await expect(keyboard.getByRole("checkbox", { name: "Fn 锁", exact: true })).not.toBeChecked();
  await page.evaluate(() => { window.__uiFixture.keyboardSettings.current.fnLocked = true; window.__uiEmit("hardware-controls-changed", null); });
  await expect(keyboard.getByRole("checkbox", { name: "Fn 锁", exact: true })).toBeChecked();
  await keyboard.getByRole("checkbox", { name: "Win 锁", exact: true }).check();
  await page.evaluate(() => { window.__uiFixture.keyboardSettings.current.fnLocked = false; window.__uiEmit("hardware-controls-changed", null); });
  await expect(keyboard.getByRole("checkbox", { name: "Win 锁", exact: true })).toBeChecked();
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
test("热键卡片在窄屏和宽屏保持模块间隔与左右布局", async ({ page }) => {
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 1800 });
      await panel(page).scrollIntoViewIfNeeded();
      const boxes = await panel(page).locator(".ccc-hotkey-section").evaluateAll(elements => elements.map(e => { const r = e.getBoundingClientRect(); return { x: r.x, y: r.y, right: r.right, bottom: r.bottom }; }));
      expect(boxes[0].right).toBeLessThanOrEqual(width + 1); expect(boxes[1].right).toBeLessThanOrEqual(width + 1);
      if (width === 1440) expect(boxes[1].x).toBeGreaterThan(boxes[0].right);
      if (width === 360) expect(boxes[1].y).toBeGreaterThan(boxes[0].bottom);
      if (width === 1440) await panel(page).screenshot({ path: `target/ui-check/hotkeys-${theme}-${width}.png`, animations: "disabled" });
      if (width === 360) {
        await page.setViewportSize({ width, height: 900 });
        await panel(page).locator(".ccc-hotkey-scope").filter({ has: page.getByRole("checkbox", { name: "原厂状态通知", exact: true }) }).scrollIntoViewIfNeeded();
        await page.screenshot({ path: `target/ui-check/hotkeys-${theme}-${width}.png`, animations: "disabled" });
      }
    }
  }
});
test("键盘灯开关独立授权，亮度策略不隐式包含开关", async ({ page }) => {
  const region = panel(page);
  await region.getByRole("checkbox", { name: "启用原厂热键接管", exact: true }).check();
  await region.getByRole("checkbox", { name: "键盘亮度", exact: true }).check();
  await expect(region.getByRole("checkbox", { name: "键盘灯开关", exact: true })).not.toBeChecked();
  await region.getByRole("button", { name: "应用热键策略", exact: true }).click();
  expect(await writes(page)).toHaveLength(0);
  await region.getByRole("button", { name: "确认此范围的热键接管？", exact: true }).click();
  expect((await writes(page))[0].args.request.settings.actions).toEqual(["keyboardBrightness"]);
  await region.getByRole("checkbox", { name: "键盘亮度", exact: true }).uncheck();
  await region.getByRole("checkbox", { name: "键盘灯开关", exact: true }).check();
  await expect(region.getByLabel("热键策略预览")).toContainText("键盘灯开关");
  await region.getByRole("button", { name: "应用热键策略", exact: true }).click();
  await region.getByRole("button", { name: "确认此范围的热键接管？", exact: true }).click();
  expect((await writes(page))[1].args.request.settings.actions).toEqual(["keyboardSwitch"]);
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply|restore)_(keyboard_lighting|perkey_lighting|multi_zone_lighting)$/.test(call.command)))).toEqual([]);
  await region.getByRole("button", { name: "恢复初始热键策略", exact: true }).click();
  await region.getByRole("button", { name: "确认恢复初始软件策略？", exact: true }).click();
  await expect(region.getByRole("checkbox", { name: "键盘灯开关", exact: true })).not.toBeChecked();
});

test("键盘灯能力失效撤销接管确认，不影响其他可用范围", async ({ page }) => {
  const region = panel(page);
  await region.getByRole("checkbox", { name: "启用原厂热键接管", exact: true }).check();
  await region.getByRole("checkbox", { name: "键盘灯开关", exact: true }).check();
  await region.getByRole("button", { name: "应用热键策略", exact: true }).click();
  await page.evaluate(() => {
    const hotkeys = window.__uiFixture.hotkeys;
    Object.assign(hotkeys.facts.capabilities.find(cap => cap.scope === "keyboardSwitch"), { identity: null, reason: "隔离键盘身份变化" });
    hotkeys.revision = "switch-device-changed";
  });
  await region.getByRole("button", { name: "重新读取热键", exact: true }).click();
  await expect(region).toContainText("隔离键盘身份变化");
  await expect(region.getByRole("button", { name: "应用热键策略", exact: true })).toBeDisabled();
  await expect(region.getByRole("button", { name: "确认此范围的热键接管？", exact: true })).toHaveCount(0);
  await region.getByRole("checkbox", { name: "键盘灯开关", exact: true }).uncheck();
  await region.getByRole("checkbox", { name: "唤起控制台", exact: true }).check();
  await expect(region.getByRole("button", { name: "应用热键策略", exact: true })).toBeEnabled();
  expect(await writes(page)).toHaveLength(0);
});

