import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const card = page => page.getByRole("region", { name: "自动刷新率", exact: true });
const trial = page => page.getByRole("dialog", { name: "自动刷新率试用" });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
async function enable(page) {
  const previous = (await calls(page, "enable_automatic_refresh")).length;
  const toggle = card(page).getByRole("switch", { name: "启用自动刷新率" });
  await expect(toggle).toHaveAccessibleDescription("启用时试用 144 Hz");
  await toggle.click();
  await expect(trial(page)).toBeVisible();
  expect((await calls(page, "enable_automatic_refresh")).length).toBe(previous + 1);
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/"); await openWorkspace(page, "设备配置");
  await expect(card(page)).toBeVisible();
  await expect(card(page).getByRole("switch", { name: "启用自动刷新率" })).toBeEnabled();
});

test("开关直接启用，跨页面保留试用后关闭并恢复启用前状态", async ({ page }) => {
  await enable(page);
  await expect(trial(page)).toContainText("60 Hz");
  await expect(trial(page)).toContainText("144 Hz");
  await expect(trial(page).getByRole("status")).toHaveText(/\d+ 秒后恢复/);
  await expect(page.getByRole("region", { name: "屏幕显示", exact: true }).getByRole("status"))
    .toHaveText("请在「自动刷新率试用」中保留或恢复本次试用");
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: "设置" }).click();
  await expect(trial(page)).toBeVisible();
  await trial(page).getByRole("button", { name: "保留刷新率" }).click();
  await expect(trial(page)).toHaveCount(0);
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: "设备配置" }).click();
  const toggle = card(page).getByRole("switch", { name: "启用自动刷新率" });
  await expect(toggle).toBeChecked();
  await expect(card(page).getByText(/^(已启用|未启用)$/)).toHaveCount(0);
  await expect(toggle).toHaveAccessibleDescription("停用时恢复 60 Hz");
  expect(await calls(page, "restore_automatic_refresh")).toEqual([]);
  await toggle.click();
  await expect.poll(() => page.evaluate(() => window.__uiFixture.display.displays[0].current.frequency)).toBe(60);
  await expect(toggle).not.toBeChecked();
  await expect(card(page).getByText(/^(已启用|未启用)$/)).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("取消或到期后恢复本次模式，过期试用无法保留", async ({ page }) => {
  await enable(page);
  await trial(page).getByRole("button", { name: "恢复本次试用" }).click();
  await expect(trial(page)).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => window.__uiFixture.display.displays[0].current.frequency)).toBe(60);
  const reads = (await calls(page, "read_automatic_refresh")).length;
  await expect.poll(async () => (await calls(page, "read_automatic_refresh")).length).toBeGreaterThan(reads);
  await page.evaluate(() => { window.__uiFixture.autoRefresh.active = false; });
  await card(page).getByRole("button", { name: "恢复启用前状态", exact: true }).click();
  await expect(card(page).getByRole("switch", { name: "启用自动刷新率" })).toBeEnabled();
  await enable(page);
  await page.evaluate(() => { window.__uiFixture.displayExpiresAt = Date.now() - 1; });
  await expect(trial(page)).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => window.__uiFixture.display.displays[0].current.frequency)).toBe(60);
});

test("监听不可用时显示原因并禁止启用", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.autoRefresh.listenerOnline = false; window.__uiFixture.autoRefresh.issue = "隔离监听已停止"; });
  await expect(card(page).getByRole("switch", { name: "启用自动刷新率" })).toBeDisabled();
  await expect(card(page).getByRole("alert")).toContainText("隔离监听已停止");
  await card(page).getByRole("button", { name: "详情", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "刷新率规则与恢复" })).toContainText("隔离监听已停止");
  expect(await calls(page, "enable_automatic_refresh")).toEqual([]);
});

test("频率无需变化时直接启用并恢复策略，全程没有显示试用", async ({ page }) => {
  const toggle = card(page).getByRole("switch", { name: "启用自动刷新率" });
  await page.evaluate(() => { window.__uiFixture.autoRefresh.current.savedOrHighest = 60; });
  await expect(toggle).not.toHaveAccessibleDescription(/启用时试用/);
  await toggle.click();
  await expect(toggle).toBeChecked();
  await expect(trial(page)).toHaveCount(0);
  await toggle.click();
  await expect(toggle).not.toBeChecked();
  expect((await calls(page, "enable_automatic_refresh")).length).toBe(1);
  expect((await calls(page, "restore_automatic_refresh")).length).toBe(1);
});

test("模块边框和全局确认适配宽窄及深浅主题", async ({ page }) => {
  await expect(card(page).locator(".ccc-refresh-screen")).toContainText("60 Hz");
  await expect(card(page).locator(".ccc-refresh-rates")).toContainText(/低刷新率.*恢复刷新率/s);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(value => { document.documentElement.dataset.theme = value; }, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 900 });
      await card(page).scrollIntoViewIfNeeded();
      const geometry = await card(page).evaluate(element => ({
        width: element.clientWidth, scroll: element.scrollWidth,
        border: parseFloat(getComputedStyle(element).borderTopWidth),
      }));
      expect(geometry.scroll).toBeLessThanOrEqual(geometry.width + 1);
      expect(geometry.border).toBeGreaterThan(0);
      if (width === 1440) {
        const display = await page.getByRole("region", { name: "屏幕显示", exact: true }).boundingBox();
        const auto = await card(page).boundingBox();
        expect(display.y).toBeCloseTo(auto.y, 0);
      }
      if (width !== 760) await card(page).screenshot({ path: `target/ui-check/auto-refresh-${theme}-${width}.png` });
    }
  }
  await page.setViewportSize({ width: 360, height: 900 });
  await enable(page);
  const rect = await trial(page).boundingBox();
  expect(rect.x).toBeGreaterThanOrEqual(0);
  expect(rect.x + rect.width).toBeLessThanOrEqual(360);
  await trial(page).screenshot({ path: "target/ui-check/auto-refresh-trial-dark-360.png" });
});
