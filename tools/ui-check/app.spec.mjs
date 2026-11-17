import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const navigate = (page, title) => page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: title, exact: true }).click();
async function choose(page, label, option) {
  await page.getByRole("combobox", { name: label, exact: true }).click();
  await page.getByRole("option", { name: option, exact: true }).click();
}
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); });

test("分类页面的下拉/分段入口统一接入且只改草稿", async ({ page }) => {
  await navigate(page, "灯光效果");
  await choose(page, "灯效", "呼吸");
  await expect(page.getByRole("checkbox", { name: "左区开关" })).toBeVisible();
  await navigate(page, "显示与显卡");
  await choose(page, "目标刷新率", "144 Hz");
  await expect(page.getByRole("button", { name: "应用刷新率", exact: true })).toBeEnabled();
  await navigate(page, "自动策略");
  await choose(page, "启动档案", "隔离档案");
  await navigate(page, "显示与显卡");
  await expect(page.getByRole("radio", { name: "关闭独显" })).toBeDisabled();
  await navigate(page, "风扇散热");
  await page.getByRole("radio", { name: "软件曲线", exact: true }).check();
  await choose(page, "选择软件控制点", "第 2 点");
  await expect(page.getByRole("spinbutton", { name: "软件点温度", exact: true })).toHaveValue(/\d+/);
  await page.getByRole("radio", { name: "固件曲线", exact: true }).check();
  await page.evaluate(() => window.__uiFixture.fanKind = "rpmTable");
  await navigate(page, "设置"); await navigate(page, "风扇散热");
  await choose(page, "选择 RPM 级别", "第 4 级");
  await expect(page.getByRole("spinbutton", { name: "RPM 温度", exact: true })).toHaveValue("55");
  await expect(page.locator("select,option")).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(({ command }) => /^(apply_|restore_|set_fan|save_|import_|delete_|set_startup)/.test(command)))).toEqual([]);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("档案输入/文件引用和设置搜索选择接入同一原语", async ({ page }) => {
  await navigate(page, "性能档案");
  const name = page.getByPlaceholder("游戏 / 静音");
  await name.fill("保留的草稿");
  await page.locator('input[type="file"]').setInputFiles({ name: "fixture.json", mimeType: "application/json", buffer: Buffer.from("{}") });
  await expect(page.getByRole("button", { name: "确认导入", exact: true })).toBeVisible();
  await navigate(page, "设置");
  await page.evaluate(() => window.__uiFixture.delay = 200);
  const font = page.getByRole("combobox", { name: "选择界面字体", exact: true });
  await font.click(); await page.getByRole("searchbox", { name: "搜索字体" }).fill("Consolas");
  await page.getByRole("option", { name: /Consolas/ }).click();
  await expect(font).toBeFocused(); await expect(font).toHaveText("Consolas");
  await expect(font).toBeEnabled();
  await navigate(page, "性能档案"); await expect(name).toHaveValue("保留的草稿");
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("实际十页浅深与窄宽窗口无控件溢出", async ({ page }) => {
  for (const title of ["控制台", "风扇散热", "性能调校", "显示与显卡", "灯光效果", "键盘与输入", "电池与节能", "自动策略", "性能档案", "设置"]) {
    await navigate(page, title);
    for (const theme of ["light", "dark"]) {
      await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
      for (const width of [360, 700, 1280]) {
        await page.setViewportSize({ width, height: 720 });
        await page.waitForFunction(width => document.querySelector(".ccc-shell").classList.contains("is-collapsed") === (width <= 700), width);
        const overflow = await page.locator("button,input:not([hidden]),.ccc-surface").evaluateAll(nodes => nodes
          .filter(el => el.checkVisibility() && el.closest('.ccc-page:not([hidden])'))
          .filter(el => el.getBoundingClientRect().right > innerWidth + 1).map(el => el.getAttribute("aria-label") ?? el.className));
        expect(overflow).toEqual([]);
      }
    }
  }
});
