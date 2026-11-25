import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);

test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); });

test("分类导航统一使用本地库图标，中文名称与选中状态保留", async ({ page }) => {
  await page.goto("/");
  for (const [title, icon] of [["概览", "overview"], ["性能与散热", "performance"], ["键盘与灯光", "keyboard"], ["电源与档案", "power"], ["设备配置", "display"], ["设置", "settings"]]) {
    const button = page.getByRole("button", { name: title, exact: true });
    await button.click();
    await expect(button).toHaveAttribute("aria-current", "page");
    const svg = button.locator("svg");
    await expect(svg).toHaveAttribute("data-icon", icon);
    await expect(svg).toHaveAttribute("aria-hidden", "true");
    await expect(svg).toHaveAttribute("focusable", "false");
    await expect(svg).toHaveAttribute("data-size", "24");
    await expect(svg).toHaveCSS("stroke-width", "2.3px");
    await expect(svg.locator(".ccc-icon-regular, .ccc-icon-fill")).toHaveCount(0);
  }
});

test("领域模块与设置标题显式使用语义图标，硬件插画继续保留", async ({ page }) => {
  await page.goto("/");
  for (const [title, icon, workspace] of [["散热控制", "fan", "性能与散热"], ["CPU 功耗与电压", "power", "性能与散热"], ["独显电源", "gpu", "设备配置"], ["电池与充电", "charging", "电源与档案"], ["屏幕显示", "display", "设备配置"], ["原厂热键与屏显", "keyboard", "键盘与灯光"]]) {
    await openWorkspace(page, workspace);
    await expect(page.getByRole("heading", { name: title, exact: true }).locator("svg")).toHaveAttribute("data-icon", icon);
  }
  expect(await page.locator("svg [data-desktop-gpu]").count()).toBeGreaterThan(0);
  await page.getByRole("button", { name: "设置", exact: true }).click();
  for (const [title, icon] of [["外观", "appearance"], ["窗口与托盘", "window"], ["运行与诊断", "performance"], ["关于", "info"]]) {
    await expect(page.getByRole("heading", { name: title, exact: true }).locator("svg")).toHaveAttribute("data-icon", icon);
  }
});

test("深浅主题与窄窗口保持图标尺寸、模块间距和页面宽度", async ({ page }, testInfo) => {
  await page.goto("/"); await openWorkspace(page, "性能与散热");
  for (const [width, theme] of [[1440, "light"], [700, "dark"]]) {
    await page.setViewportSize({ width, height: 1000 });
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    const title = page.getByRole("heading", { name: "散热控制", exact: true });
    await expect(title.locator("svg")).toHaveCSS("width", "20px");
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
    expect(overflow).toBeFalsy();
    await page.screenshot({ path: testInfo.outputPath(`lucide-${theme}-${width}.png`) });
  }
});
