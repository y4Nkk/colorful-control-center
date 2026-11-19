import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const facts = page => page.getByRole("region", { name: "原厂低电量系统动作事实与预览", exact: true });
const scope = page => panel(page).getByRole("checkbox", { name: "原厂低电量系统动作", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与灯光"); });

test("预览两类计划动作和一次性效果，不虚构比例", async ({ page }) => {
  await scope(page).check();
  await expect(facts(page)).toContainText("低电量：睡眠");
  await expect(facts(page)).toContainText("严重电量：休眠");
  await expect(facts(page)).toContainText("睡眠和休眠不会自动撤销");
  await expect(facts(page).getByRole("meter")).toHaveCount(0);
  await scope(page).check();
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
});

test("独立确认后启用，软件规则可以单独恢复", async ({ page }) => {
  await panel(page).getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await scope(page).check();
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
  await scope(page).check();
  await expect(facts(page)).toBeVisible();
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, low = h.facts.batteryLowAction;
    low.current.low = 3; low.error = "活动计划已变化"; low.identity = null;
    const cap = h.facts.capabilities.find(value => value.scope === "batteryLowAction"); cap.identity = null; cap.reason = low.error;
    h.revision = "action-changed"; window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(facts(page)).toContainText("计划设置关机，原厂事件不另行请求");
  await expect(facts(page)).toContainText("Windows 电源计划仍可能独立执行其设置");
  await expect(scope(page)).toBeEnabled();
  await scope(page).uncheck();
  await expect(scope(page)).toBeDisabled();
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
});

test("深浅宽窄布局保留模块边框与间隔", async ({ page }) => {
  await scope(page).check();
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
