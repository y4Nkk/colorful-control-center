import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const warning = page => page.getByRole("alert").filter({ hasText: "电池温度较低" });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
async function setWarning(page, id = 1) {
  await page.evaluate(id => {
    const h = window.__uiFixture.hotkeys;
    h.status.coldWarning = { id, temperatureC: 10, batteryPercent: 80 };
    window.__uiEmit("hotkeys-state", h.status);
  }, id);
  await expect(warning(page)).toBeVisible();
}

test("低温提醒单独授权，展示原厂触发条件", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与灯光");
  await panel(page).getByRole("button", { name: "配置热键", exact: true }).click();
  const editor = page.getByRole("dialog", { name: "配置热键与屏显", exact: true });
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await editor.getByRole("checkbox", { name: "原厂电池低温提醒", exact: true }).check();
  await editor.getByRole("button", { name: "完成编辑", exact: true }).click();
  const review = await previewConfiguration(page, "原厂热键与屏显");
  await expect(review).toContainText("低于 11°C 且电量低于 81%");
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
  await configurationView(page).getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  expect((await calls(page, "apply_hotkeys"))[0].args.request.settings.actions).toEqual(["batteryColdWarning"]);
});

test("全局提醒不依赖普通OSD，用户确认关闭并不写硬件", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与灯光");
  await expect(panel(page)).toBeVisible();
  await page.evaluate(() => { window.__uiFixture.hotkeys.current.osd = false; });
  await setWarning(page, 3);
  await expect(warning(page)).toContainText("电池温度 10°C，电量 80%。请接入电源。");
  await warning(page).getByRole("button", { name: "知道了", exact: true }).click();
  expect((await calls(page, "dismiss_battery_cold_warning"))[0].args).toEqual({ expectedId: 3 });
  await expect(warning(page)).toHaveCount(0);
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
});

test("确认失败保留提醒，新提醒和插电关闭以最新状态为准", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与灯光");
  await expect(panel(page)).toBeVisible(); await setWarning(page, 5);
  await page.evaluate(() => { window.__uiFixture.coldWarningDismissFault = "关闭提醒失败"; });
  await warning(page).getByRole("button", { name: "知道了", exact: true }).click();
  await page.getByRole("button", { name: /^通知（/ }).click();
  await expect(page.getByRole("dialog", { name: "通知", exact: true })).toContainText("关闭提醒失败");
  await page.getByRole("button", { name: "收起通知", exact: true }).click();
  await expect(warning(page)).toBeVisible();
  await setWarning(page, 6);
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys; h.status.coldWarning = null; window.__uiEmit("hotkeys-state", h.status);
  });
  await expect(warning(page)).toHaveCount(0);
  expect(await calls(page, "dismiss_battery_cold_warning")).toHaveLength(1);
});

test("订阅后到达的新提醒不会被较早的状态读取覆盖", async ({ page }) => {
  await page.addInitScript({ content: `${MOCK_IPC}; window.__uiFixture.coldWarningStatusDelay = 1800;` });
  await page.goto("/"); await openWorkspace(page, "键盘与灯光");
  await expect(panel(page)).toBeVisible();
  await setWarning(page, 7);
  await page.waitForTimeout(2000);
  await expect(warning(page)).toBeVisible();
  expect(await page.evaluate(() => window.__uiFixture.hotkeys.status.coldWarning.id)).toBe(7);
});

test("提醒在深浅宽窄窗口内保留模块边框且按钮可操作", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与灯光");
  await expect(panel(page)).toBeVisible(); await setWarning(page, 9);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 1440]) {
      await page.setViewportSize({ width, height: 900 });
      const box = await warning(page).evaluate(el => ({ right: el.getBoundingClientRect().right,
        overflow: el.scrollWidth > el.clientWidth + 1, border: parseFloat(getComputedStyle(el).borderTopWidth) }));
      expect(box.right).toBeLessThanOrEqual(width); expect(box.overflow).toBe(false); expect(box.border).toBeGreaterThan(0);
      await warning(page).screenshot({ path: `target/ui-check/cold-warning-${theme}-${width}.png` });
    }
  }
  await expect(warning(page).getByRole("button", { name: "知道了", exact: true })).toBeEnabled();
});
