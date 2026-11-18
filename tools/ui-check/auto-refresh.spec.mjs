import { configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const card = page => page.getByRole("region", { name: "自动刷新率", exact: true });
const trial = page => page.getByRole("dialog", { name: "自动刷新率试用" });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
async function enable(page) {
  const previous = (await calls(page, "enable_automatic_refresh")).length;
  await card(page).getByRole("switch", { name: "启用自动刷新率" }).check();
  await openConfigurationReview(page, "自动刷新率");
  await expect(configurationDialog(page)).toContainText("启用时试用 60 → 144 Hz");
  expect((await calls(page, "enable_automatic_refresh")).length).toBe(previous);
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  await expect(trial(page)).toBeVisible();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/"); await openWorkspace(page, "显示与显卡");
  await expect(card(page)).toBeVisible();
  await expect(card(page).getByRole("switch", { name: "启用自动刷新率" })).toBeEnabled();
});

test("先预览确认，跨页面保留试用后可恢复启用前状态", async ({ page }) => {
  await enable(page);
  await expect(trial(page)).toContainText("60 Hz");
  await expect(trial(page)).toContainText("144 Hz");
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: "设置" }).click();
  await expect(trial(page)).toBeVisible();
  await trial(page).getByRole("button", { name: "保留刷新率" }).click();
  await expect(trial(page)).toHaveCount(0);
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: "显示与显卡" }).click();
  await expect(card(page)).toContainText("已启用");
  await openConfigurationReview(page, "自动刷新率", true);
  expect(await calls(page, "restore_automatic_refresh")).toEqual([]);
  await configurationDialog(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__uiFixture.display.displays[0].current.frequency)).toBe(60);
  await expect(card(page)).toContainText("未启用");
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("取消或到期后恢复本次模式，过期试用无法保留", async ({ page }) => {
  await enable(page);
  await trial(page).getByRole("button", { name: "恢复本次试用" }).click();
  await expect(trial(page)).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => window.__uiFixture.display.displays[0].current.frequency)).toBe(60);
  const reads = (await calls(page, "read_automatic_refresh")).length;
  await expect.poll(async () => (await calls(page, "read_automatic_refresh")).length).toBeGreaterThan(reads);
  await openConfigurationReview(page, "自动刷新率", true);
  await configurationDialog(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(card(page).getByRole("switch", { name: "启用自动刷新率" })).toBeEnabled();
  await enable(page);
  await page.evaluate(() => { window.__uiFixture.displayExpiresAt = Date.now() - 1; });
  await expect(trial(page)).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => window.__uiFixture.display.displays[0].current.frequency)).toBe(60);
});

test("监听不可用时显示原因并禁止启用", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.autoRefresh.listenerOnline = false; window.__uiFixture.autoRefresh.issue = "隔离监听已停止"; });
  await expect(card(page).getByRole("switch", { name: "启用自动刷新率" })).toBeDisabled();
  await card(page).getByRole("button", { name: "详情", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "刷新率规则与恢复" })).toContainText("隔离监听已停止");
  expect(await calls(page, "enable_automatic_refresh")).toEqual([]);
});

test("模块边框和全局确认适配宽窄及深浅主题", async ({ page }) => {
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
        const mux = await page.getByRole("region", { name: "显卡模式", exact: true }).boundingBox();
        const auto = await card(page).boundingBox();
        expect(mux.y).toBeCloseTo(auto.y, 0);
        expect(mux.height).toBeCloseTo(auto.height, 0);
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
