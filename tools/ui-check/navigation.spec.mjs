import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const pages = ["概览", "风扇散热", "性能调校", "显示与显卡", "灯光效果", "键盘与输入", "电池与节能", "自动策略", "性能档案", "设置"];
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); });
const navigate = async (page, name) => {
  await page.getByRole("button", { name, exact: true }).click();
  await expect(page.getByRole("heading", { name, exact: true, level: 1 })).toBeVisible();
};
test("十个语义入口与独立界面，浏览不写硬件", async ({ page }) => {
  for (const name of pages) {
    await navigate(page, name);
    await expect(page.getByRole("button", { name, exact: true })).toHaveAttribute("aria-current", "page");
  }
  await navigate(page, "概览");
  await expect(page.getByRole("region", { name: "温度趋势", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "应用灯光选项", exact: true })).not.toBeVisible();
  await expect(page.getByRole("button", { name: "确认应用模式", exact: true })).not.toBeVisible();
  const writes = await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|set_|enable_|restore_)/.test(call.command) && call.command !== "set_shortcut_recording"));
  expect(writes).toEqual([]);
});
test("跨页保留灯光草稿且不创建重复编辑器", async ({ page }) => {
  await navigate(page, "灯光效果");
  const toggle = page.getByRole("switch", { name: "启用闲置关灯", exact: true });
  await toggle.check();
  await page.getByRole("spinbutton", { name: "闲置等待秒数", exact: true }).fill("125");
  await navigate(page, "风扇散热");
  await navigate(page, "键盘与输入");
  await navigate(page, "灯光效果");
  await expect(page.getByRole("spinbutton", { name: "闲置等待秒数", exact: true })).toHaveValue("125");
  expect(await page.locator('section[aria-label="灯光策略与外围灯区"]').count()).toBe(1);
});

test("每个分类保留自己的滚动位置", async ({ page }) => {
  await navigate(page, "灯光效果");
  const viewport = page.locator(".ccc-workspace > .ccc-scroll-viewport");
  await expect(page.getByRole("switch", { name: "启用闲置关灯", exact: true })).toBeVisible();
  await viewport.evaluate(element => { element.scrollTop = 120; });
  await expect.poll(() => viewport.evaluate(element => element.scrollTop)).toBe(120);
  await page.waitForTimeout(100);
  await navigate(page, "风扇散热");
  await expect.poll(() => viewport.evaluate(element => element.scrollTop)).toBe(0);
  await navigate(page, "灯光效果");
  await expect.poll(() => viewport.evaluate(element => element.scrollTop)).toBe(120);
});

for (const theme of ["light", "dark"]) {
  test(`分类页面 ${theme} 主题与窄窗口`, async ({ page }) => {
    await navigate(page, "设置");
    await page.getByRole("radio", { name: theme === "light" ? "浅色" : "深色", exact: true }).check();
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
    for (const width of [1440, 360]) {
      await page.setViewportSize({ width, height: 960 });
      for (const name of ["概览", "性能调校", "灯光效果", "自动策略"]) {
        await navigate(page, name);
        const main = page.getByRole("main", { name, exact: true });
        const bounds = await main.boundingBox();
        expect(bounds.x + bounds.width).toBeLessThanOrEqual(width + 1);
        await page.screenshot({ path: `target/ui-check/navigation-${theme}-${width}-${pages.indexOf(name)}.png` });
      }
    }
  });
}
