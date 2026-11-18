import { availableAction, configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";
const card = page => page.getByRole("region", { name: "飞行模式", exact: true });
const calls = page => page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply|restore)_airplane$/.test(call.command)));
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与输入");
});
test("系统状态、目标预览、确认与首次恢复共用一个飞行模式状态", async ({ page }) => {
  const p = card(page); await expect(p.getByLabel("系统飞行模式")).toContainText("已关闭");
  await p.getByRole("radio", { name: "开启", exact: true }).check();
  await expect(p.getByLabel("飞行模式目标").first()).toContainText("已关闭 → 已开启");
  expect(await calls(page)).toEqual([]);
  await openConfigurationReview(page, "飞行模式");
  expect(await calls(page)).toEqual([]);
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  await expect(p.getByLabel("系统飞行模式")).toContainText("已开启");
  expect((await calls(page))[0].args.request.enabled).toBe(true);
  await openConfigurationReview(page, "飞行模式", true);
  await configurationDialog(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(p.getByLabel("系统飞行模式")).toContainText("已关闭");
  await expect(availableAction(page, "飞行模式", true)).toHaveCount(0);
});
test("失败保留错误与当前状态，未知状态不提供执行确认", async ({ page }) => {
  const p = card(page);
  await p.getByRole("radio", { name: "开启", exact: true }).check();
  await page.evaluate(() => { window.__uiFixture.airplaneFault = "原厂设备IOCTL失败"; });
  await openConfigurationReview(page, "飞行模式");
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  await expect(p.getByRole("alert")).toContainText("原厂设备IOCTL失败");
  await expect(p.getByLabel("系统飞行模式")).toContainText("已关闭");
  await page.evaluate(() => { window.__uiFixture.airplane.current = null; window.__uiFixture.airplane.error = "系统报告未知"; });
  await p.getByRole("button", { name: "重新读取飞行模式", exact: true }).click();
  await expect(availableAction(page, "飞行模式")).toHaveCount(0);
  await expect(p.getByRole("alert")).toContainText("系统报告未知");
});
test("两个原厂事件分别授权，预览与浏览不发送一次性请求", async ({ page }) => {
  const p = page.getByRole("region", { name: "原厂热键与屏显", exact: true });
  await p.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await p.getByRole("checkbox", { name: "飞行模式按键", exact: true }).check();
  await p.getByRole("checkbox", { name: "原厂面板维护", exact: true }).check();
  await openConfigurationReview(page, "原厂热键与屏显");
  expect(await calls(page)).toEqual([]);
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  const actions = await page.evaluate(() => window.__uiFixture.calls.find(call => call.command === "apply_hotkeys").args.request.settings.actions);
  expect(actions).toEqual(["airplaneMode", "panelEvent"]);
});
test("飞行模式左右卡片在窄屏与主题切换后保持可操作", async ({ page }) => {
  for (const width of [360, 760, 1440]) {
    await page.setViewportSize({ width, height: 960 });
    for (const theme of ["light", "dark"]) {
      await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
      const p = card(page); await p.scrollIntoViewIfNeeded();
      await expect(p.locator('[data-icon="airplane"]')).toHaveCount(1);
      await expect(p.locator('svg.ccc-airplane-artwork')).toHaveCount(1);
      expect(await p.evaluate(element => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
      await p.screenshot({ path: `target/ui-check/airplane-${width}-${theme}.png` });
    }
  }
});
