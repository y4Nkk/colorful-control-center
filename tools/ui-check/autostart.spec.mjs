import { availableAction, configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "登录自启", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await expect(panel(page).getByRole("combobox", { name: "登录自启方式" })).toBeEnabled();
});
async function select(page, label) {
  await panel(page).getByRole("combobox", { name: "登录自启方式" }).click();
  await page.getByRole("option", { name: label, exact: true }).click();
}
test("管理员方式预览与保存恢复均二次确认", async ({ page }) => {
  await select(page, "管理员登录启动");
  expect(await calls(page, "set_autostart")).toEqual([]);
  await openConfigurationReview(page, "登录自启");
  expect(await calls(page, "set_autostart")).toEqual([]);
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  await expect(availableAction(page, "登录自启", true)).toBeEnabled();
  expect((await calls(page, "set_autostart"))[0].args).toEqual({ expectedRevision: "login-0", level: "administrator", restore: false });
  await openConfigurationReview(page, "登录自启", true);
  expect(await calls(page, "set_autostart")).toHaveLength(1);
  await configurationDialog(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(panel(page).getByRole("combobox", { name: "登录自启方式" })).toHaveText("不自动启动");
});
test("旧预览和授权失败保留错误并要求重新读取", async ({ page }) => {
  await select(page, "管理员登录启动");
  await page.evaluate(() => { window.__uiFixture.autostartFault = "隔离系统授权取消"; });
  await openConfigurationReview(page, "登录自启");
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  await expect(panel(page).getByRole("alert")).toContainText("隔离系统授权取消");
  await expect(panel(page)).toContainText("当前进程 · 权限未知");
  await expect(panel(page).getByRole("combobox", { name: "登录自启方式" })).toHaveAttribute("aria-disabled", "true");
  await panel(page).getByRole("button", { name: "重新读取启动状态", exact: true }).click();
  await expect(panel(page).getByRole("combobox", { name: "登录自启方式" })).toBeEnabled();
  await select(page, "管理员登录启动");
  await page.evaluate(() => { window.__uiFixture.autostart = { revision: "external", level: "standard", elevated: false, canRestore: false, legacy: false }; });
  await openConfigurationReview(page, "登录自启");
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  await expect(panel(page).getByRole("alert")).toContainText("预览已变化");
});
test("改变方式撤销旧确认且普通方式不承诺提权", async ({ page }) => {
  await select(page, "管理员登录启动");
  await openConfigurationReview(page, "登录自启");
  await select(page, "普通登录启动");
  await expect(panel(page)).toContainText("后台操作不临时提权");
  await openConfigurationReview(page, "登录自启");
  expect(await calls(page, "set_autostart")).toEqual([]);
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  expect((await calls(page, "set_autostart"))[0].args.level).toBe("standard");
});

test("登录卡片在明暗主题及窄屏保留模块边框与左右布局", async ({ page }) => {
  await select(page, "管理员登录启动");
  for (const theme of ["light", "dark"]) {
    await page.getByRole("radio", { name: theme === "light" ? "浅色" : "深色", exact: true }).check();
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    for (const width of [1440, 360]) {
      await page.setViewportSize({ width, height: 1000 });
      const card = panel(page);
      await card.scrollIntoViewIfNeeded();
      const bounds = await card.boundingBox();
      expect(bounds.x).toBeGreaterThanOrEqual(0);
      expect(bounds.x + bounds.width).toBeLessThanOrEqual(width + 1);
      expect(await card.evaluate(el => getComputedStyle(el).borderTopStyle)).toBe("solid");
      await card.screenshot({ path: `target/ui-check/login-${theme}-${width}.png` });
    }
  }
});
