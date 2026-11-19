import { availableAction, configurationView, previewConfiguration } from "./configuration-review.mjs";
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
test("选择登录方式立即提交，初始方式可在卡内恢复", async ({ page }) => {
  expect(await calls(page, "set_autostart")).toEqual([]);
  await select(page, "管理员登录启动");
  await expect(availableAction(page, "登录自启", true)).toBeEnabled();
  expect((await calls(page, "set_autostart"))[0].args).toEqual({ expectedRevision: "login-0", level: "administrator", restore: false });
  const restore = await previewConfiguration(page, "登录自启", true);
  expect(await calls(page, "set_autostart")).toHaveLength(1);
  await restore.getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(panel(page).getByRole("combobox", { name: "登录自启方式" })).toHaveText("不自动启动");
});
test("取消系统授权与过时的修订通过模块反馈", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.autostartFault = "隔离系统授权取消"; });
  await select(page, "管理员登录启动");
  await expect(panel(page).getByRole("alert")).toContainText("隔离系统授权取消");
  const field = panel(page).getByRole("combobox", { name: "登录自启方式" });
  await expect(field).toBeDisabled();
  await panel(page).getByRole("button", { name: "重新读取启动状态", exact: true }).click();
  await expect(field).toBeEnabled();
  await page.evaluate(() => { window.__uiFixture.autostart = { revision: "external", level: "standard", elevated: false, canRestore: false, legacy: false }; });
  await select(page, "管理员登录启动");
  await expect(panel(page).getByRole("alert")).toContainText("预览已变化");
});
test("连续两次选择使用最新回读和修订", async ({ page }) => {
  await select(page, "管理员登录启动");
  await expect(panel(page).getByRole("combobox", { name: "登录自启方式" })).toHaveText("管理员登录启动");
  await select(page, "普通登录启动");
  await expect(panel(page).getByRole("combobox", { name: "登录自启方式" })).toHaveText("普通登录启动");
  expect((await calls(page, "set_autostart")).map(call => call.args.level)).toEqual(["administrator", "standard"]);
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
