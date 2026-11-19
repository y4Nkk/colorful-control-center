import { pageActions } from "./configuration-review.mjs";
import { deviceRestoreButton } from "./device-settings.mjs";
import { openWorkspace, openSystemArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);

test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); });

test("分类导航统一使用本地库图标，中文名称与选中状态保留", async ({ page }) => {
  await page.goto("/");
  for (const [title, icon] of [["系统", "overview"], ["键盘与灯光", "keyboard"], ["电源", "power"], ["设备配置", "display"], ["设置", "settings"]]) {
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
  for (const [title, icon, workspace] of [["风扇曲线", "fan", "风扇散热"], ["CPU 功耗与电压", "power", "性能调校"], ["独显电源", "gpu", "设备配置"], ["电池与充电", "charging", "电源"], ["屏幕显示", "display", "设备配置"], ["原厂热键与屏显", "keyboard", "键盘与灯光"]]) {
    if (workspace === "风扇散热" || workspace === "性能调校") await openSystemArea(page, workspace);
    else await openWorkspace(page, workspace);
    await expect(page.getByRole("heading", { name: title, exact: true }).locator("svg")).toHaveAttribute("data-icon", icon);
  }
  expect(await page.locator("svg [data-desktop-gpu]").count()).toBeGreaterThan(0);
  await page.getByRole("button", { name: "设置", exact: true }).click();
  for (const [title, icon] of [["外观", "appearance"], ["窗口与托盘", "window"], ["运行与诊断", "performance"], ["关于", "info"]]) {
    await expect(page.getByRole("heading", { name: title, exact: true }).locator("svg")).toHaveAttribute("data-icon", icon);
  }
});

test("独显切换直接回读，卡内恢复沿用撤销图标", async ({ page }) => {
  await page.addInitScript(() => { Object.assign(window.__uiFixture.gpuPower.devices[0], { drivesDisplay: false, canDisable: true }); });
  await page.goto("/");
  await openWorkspace(page, "设备配置");
  const panel = page.getByRole("region", { name: "独显电源", exact: true });
  expect(await calls(page, "apply_gpu_power")).toHaveLength(0);
  await panel.getByRole("radio", { name: "关闭独显", exact: true }).click();
  await expect.poll(async () => (await calls(page, "apply_gpu_power")).length).toBe(1);
  await expect(deviceRestoreButton(panel).locator("svg")).toHaveAttribute("data-icon", "undo");
  await expect(pageActions(page)).toHaveCount(0);
});

test("深浅主题与窄窗口保持图标尺寸、模块间距和页面宽度", async ({ page }, testInfo) => {
  await page.goto("/"); await openSystemArea(page, "风扇散热");
  for (const [width, theme] of [[1440, "light"], [700, "dark"]]) {
    await page.setViewportSize({ width, height: 1000 });
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    const title = page.getByRole("heading", { name: "风扇曲线", exact: true });
    await expect(title.locator("svg")).toHaveCSS("width", "20px");
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
    expect(overflow).toBeFalsy();
    await page.screenshot({ path: testInfo.outputPath(`lucide-${theme}-${width}.png`) });
  }
});
