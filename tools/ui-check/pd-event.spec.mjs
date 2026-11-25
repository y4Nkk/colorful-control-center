import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { editHotkeys, finishHotkeyEdit, hotkeyRestoreButton, openHotkeyDetails, openWorkspace, previewHotkeyRestore } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const facts = page => page.getByRole("region", { name: "原厂 PD 事件事实与预览", exact: true });
const scope = editor => editor.getByRole("checkbox", { name: "原厂 PD 档位事件", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
async function setup(page) {
  await expect(panel(page).getByRole("button", { name: "配置热键", exact: true })).toBeEnabled();
  await page.evaluate(() => {
    const data = window.__uiFixture, h = data.hotkeys;
    const pd = data.platform.powerDelivery = { revision: "pd-event-0", encoding: "adapterV1", currentLevel: 3,
      currentValid: true, availableLevels: [1, 2, 3, 4], zeroBased: false, adapterWatts: 100,
      initial: null, restoreBlockedReason: null, blockedReason: null };
    h.facts.pdEvent = { identity: "isolated-pd-event", current: pd, error: null };
    const cap = h.facts.capabilities.find(value => value.scope === "powerDeliveryEvent"); cap.identity = "isolated-pd-event"; cap.reason = null;
    h.revision = "pd-event-ready"; window.__uiEmit("hardware-controls-changed", null);
  });
}
async function applied(page) {
  await setup(page);
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, pd = window.__uiFixture.platform.powerDelivery;
    pd.initial = { value: 3, zeroBased: false }; pd.currentLevel = 1; pd.revision = "pd-applied";
    h.current = { enabled: true, actions: ["powerDeliveryEvent"], osd: true }; h.authorized = true; h.status.running = true;
    h.revision = "pd-applied-hotkeys"; window.__uiEmit("hardware-controls-changed", null);
  });
  const details = await openHotkeyDetails(page);
  await expect(facts(page)).toContainText("恢复接管前的档位");
  return details;
}
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与灯光"); });

test("预览设备及事件目标，零基设备显示正确档位", async ({ page }) => {
  await setup(page);
  const editor = await editHotkeys(page);
  await scope(editor).check();
  await finishHotkeyEdit(editor);
  await openHotkeyDetails(page);
  await expect(facts(page)).toContainText("当前档位等级 3");
  await expect(facts(page)).toContainText("触发后设置：等级 1");
  await expect(facts(page)).toContainText("适配器报告功率100 W");
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, pd = window.__uiFixture.platform.powerDelivery;
    pd.zeroBased = true; pd.encoding = "legacyZeroBased"; pd.currentLevel = 0; pd.availableLevels = [0, 1]; pd.adapterWatts = null;
    h.revision = "pd-legacy"; window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(facts(page)).toContainText("当前档位等级 1");
  await expect(facts(page)).toContainText("触发后设置：等级 2");
});

test("PD事件独立授权且确认前不发送", async ({ page }) => {
  await setup(page);
  const editor = await editHotkeys(page);
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await scope(editor).check();
  await finishHotkeyEdit(editor);
  await previewConfiguration(page, "原厂热键与屏显");
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
  await configurationView(page).getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  expect((await calls(page, "apply_hotkeys"))[0].args.request.settings.actions).toEqual(["powerDeliveryEvent"]);
});

test("复用平台初始基线，恢复确认前不输出且失败可重试", async ({ page }) => {
  await applied(page);
  await expect(facts(page)).toContainText("恢复接管前的档位：等级 1 → 等级 3");
  await page.evaluate(() => { window.__uiFixture.pdEventRestoreFault = "PD恢复失败"; });
  for (let attempt = 0; attempt < 2; attempt++) {
    const restore = await previewHotkeyRestore(page, "初始 PD 档位");
    if (!attempt) expect(await calls(page, "restore_pd_event")).toEqual([]);
    await restore.getByRole("button", { name: "确认恢复", exact: true }).click();
    if (!attempt) await expect(await openHotkeyDetails(page)).toContainText("PD恢复失败");
  }
  expect(await calls(page, "restore_pd_event")).toHaveLength(2);
  const details = await openHotkeyDetails(page);
  await expect(facts(page)).toContainText("当前档位等级 3");
  await expect(details).toContainText("热键监听已停止");
  expect(await page.evaluate(() => window.__uiFixture.platform.powerDelivery.currentLevel)).toBe(3);
});

test("候选不再包含事件目标时保留可恢复的已知基线", async ({ page }) => {
  const details = await applied(page);
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, pd = window.__uiFixture.platform.powerDelivery;
    pd.availableLevels = [3, 4]; pd.currentLevel = 3; pd.revision = "pd-no-event-target";
    h.facts.pdEvent.identity = null; h.facts.pdEvent.error = "当前适配器未声明原厂目标";
    const cap = h.facts.capabilities.find(value => value.scope === "powerDeliveryEvent"); cap.identity = null; cap.reason = h.facts.pdEvent.error;
    h.revision = "pd-target-removed"; window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(facts(page)).toContainText("当前适配器未声明原厂目标");
  await details.getByRole("button", { name: "关闭弹窗", exact: true }).click();
  await expect(hotkeyRestoreButton(page)).toBeEnabled();
  await expect(scope(await editHotkeys(page))).toBeChecked();
});

test("深浅宽窄卡片有边框和间距，宽区域左右排列", async ({ page }) => {
  await applied(page);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 900 }); await facts(page).scrollIntoViewIfNeeded();
      const box = await facts(page).evaluate(el => {
        const columns = el.querySelector(".ccc-pd-event-columns"), parts = [...columns.children];
        const [left, right] = parts.map(part => part.getBoundingClientRect());
        return { overflow: el.scrollWidth > el.clientWidth + 1, right: el.getBoundingClientRect().right,
          borders: parts.every(part => parseFloat(getComputedStyle(part).borderTopWidth) > 0), gap: parseFloat(getComputedStyle(columns).gap),
          layout: el.clientWidth >= 380 ? right.left >= left.right - 1 : right.top >= left.bottom - 1 };
      });
      expect(box.overflow).toBe(false); expect(box.right).toBeLessThanOrEqual(width);
      expect(box.borders).toBe(true); expect(box.gap).toBeGreaterThan(0); expect(box.layout).toBe(true);
      if (width !== 760) await facts(page).screenshot({ path: `target/ui-check/pd-event-${theme}-${width}.png` });
    }
  }
});
