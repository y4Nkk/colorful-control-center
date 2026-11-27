import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { editHotkeys, finishHotkeyEdit, openHotkeyDetails, openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const facts = page => page.getByRole("region", { name: "原厂低电量系统动作事实与预览", exact: true });
const scope = editor => editor.getByRole("checkbox", { name: "原厂低电量系统动作", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
const close = dialog => dialog.getByRole("button", { name: "关闭弹窗", exact: true }).click();
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与灯光"); });

async function openActionDetails(page) {
  const editor = await editHotkeys(page);
  await scope(editor).check();
  await finishHotkeyEdit(editor);
  return openHotkeyDetails(page);
}

async function openExplanation(page) {
  await facts(page).getByRole("button", { name: "事件与恢复说明", exact: true }).click();
  return page.getByRole("dialog", { name: "事件与恢复说明", exact: true });
}

test("预览两类计划动作和一次性效果，不虚构比例", async ({ page }) => {
  await openActionDetails(page);
  const actions = facts(page).locator("dl[aria-label='电池计划动作']");
  await expect(actions.locator("dt")).toHaveText(["低电量", "严重电量"]);
  await expect(actions.locator("dd")).toHaveText(["睡眠", "休眠"]);
  const explanation = await openExplanation(page);
  await expect(explanation).toContainText("本软件不会自动把它唤醒");
  await close(explanation);
  await expect(facts(page).getByRole("meter")).toHaveCount(0);
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
});

test("独立确认后启用，软件规则可以单独恢复", async ({ page }) => {
  const editor = await editHotkeys(page);
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await scope(editor).check();
  await finishHotkeyEdit(editor);
  await previewConfiguration(page, "原厂热键与屏显");
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
  await configurationView(page).getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  expect((await calls(page, "apply_hotkeys"))[0].args.request.settings.actions).toEqual(["batteryLowAction"]);
  await previewConfiguration(page, "原厂热键与屏显", true);
  expect(await calls(page, "restore_hotkeys")).toEqual([]);
  await configurationView(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  expect(await calls(page, "restore_hotkeys")).toHaveLength(1);
});

test("计划关机值说明原厂不另行请求，未知来源不允许新增授权", async ({ page }) => {
  const details = await openActionDetails(page);
  await expect(facts(page)).toBeVisible();
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, low = h.facts.batteryLowAction;
    low.current.low = 3; low.error = "活动计划已变化"; low.identity = null;
    const cap = h.facts.capabilities.find(value => value.scope === "batteryLowAction"); cap.identity = null; cap.reason = low.error;
    h.revision = "action-changed"; window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(facts(page)).toContainText("按电源计划关机");
  const explanation = await openExplanation(page);
  await expect(explanation).toContainText("Windows 自己的电源计划也可能单独让电脑睡眠或休眠");
  await close(explanation);
  await close(details);
  const editor = await editHotkeys(page);
  await expect(scope(editor)).toBeEnabled();
  await scope(editor).click();
  await expect(scope(editor)).toHaveCount(0);
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
});

test("深浅宽窄布局保留模块边框与间隔", async ({ page }) => {
  await openActionDetails(page);
  await expect(facts(page)).toBeVisible();
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 900 }); await facts(page).scrollIntoViewIfNeeded();
      const box = await facts(page).evaluate(el => {
        const columns = el.querySelector(".ccc-battery-action-columns"), parts = [...columns.children];
        const [left, right] = parts.map(part => part.getBoundingClientRect());
        return { overflow: el.scrollWidth > el.clientWidth + 1, right: el.getBoundingClientRect().right,
          borders: parts.every(part => parseFloat(getComputedStyle(part).borderTopWidth) > 0), gap: parseFloat(getComputedStyle(columns).gap),
          layout: el.clientWidth >= 380 ? right.left >= left.right - 1 : right.top >= left.bottom - 1 };
      });
      expect(box.overflow).toBe(false); expect(box.right).toBeLessThanOrEqual(width);
      expect(box.borders).toBe(true); expect(box.gap).toBeGreaterThan(0); expect(box.layout).toBe(true);
      if (width !== 760) await facts(page).screenshot({ path: `target/ui-check/battery-low-action-${theme}-${width}.png` });
    }
  }
});
