import { expect, test } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";

test("设置使用居中单列，分组和控件对齐并保留插画", async ({ page }, testInfo) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("/");
  await openWorkspace(page, "设置");

  const main = page.getByRole("main", { name: "设置", exact: true });
  const groups = main.locator(".ccc-settings-section");
  await expect(groups.locator(":scope > h3")).toHaveText(["外观", "窗口与托盘", "启动", "运行与诊断", "关于"]);
  const title = main.getByRole("heading", { name: "设置", level: 2, exact: true });
  const appearance = main.getByRole("region", { name: "外观", exact: true });
  const windowSettings = main.getByRole("region", { name: "窗口与托盘", exact: true });
  const startup = main.getByRole("region", { name: "启动", exact: true });
  const runtime = main.getByRole("region", { name: "运行与诊断", exact: true });
  const logo = main.getByRole("region", { name: "关于", exact: true }).locator("svg.ccc-about-logo");
  await expect(appearance.locator(".ccc-preview-content > svg")).toHaveCount(4);
  await expect(windowSettings.locator("svg.ccc-preference-illustration")).toHaveCount(2);
  await expect(logo).toBeVisible();
  await expect(startup.getByRole("combobox", { name: "登录自启方式" })).toBeEnabled();
  await expect(runtime.getByRole("switch", { name: "自动关闭原厂软件" })).toBeVisible();

  for (const [width, theme] of [[1280, "light"], [820, "dark"], [600, "light"], [480, "light"], [360, "dark"]]) {
    await page.setViewportSize({ width, height: 900 });
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    await page.waitForFunction(width => document.querySelector(".ccc-shell").classList.contains("is-collapsed") === (width <= 700), width);
    await expect(main.getByRole("searchbox", { name: "搜索设置", exact: true })).toHaveCount(0);
    await expect(main.getByRole("navigation", { name: "设置分区", exact: true })).toHaveCount(0);
    const layout = await groups.evaluateAll(sections => sections.map(section => {
      const heading = section.querySelector("h3");
      const title = heading.getBoundingClientRect();
      const text = heading.querySelector("span").getBoundingClientRect();
      const icon = heading.querySelector("svg")?.getBoundingClientRect();
      const panel = section.querySelector(".ccc-settings-group").getBoundingClientRect();
      return { heading: title.left, textLeft: text.left, iconRight: icon?.right,
        left: panel.left, right: panel.right, top: title.top, bottom: panel.bottom };
    }));
    const frame = await main.locator(".ccc-page:visible").boundingBox();
    const midpoint = (layout[0].left + layout[0].right) / 2;
    expect(Math.abs(midpoint - (frame.x + frame.width / 2))).toBeLessThanOrEqual(1);
    expect((await title.boundingBox()).x).toBeCloseTo(layout[0].left);
    for (const [index, group] of layout.entries()) {
      expect(group.heading).toBeCloseTo(group.left);
      expect(group.textLeft).toBeCloseTo(layout[0].textLeft);
      if (group.iconRight != null) expect(group.iconRight).toBeLessThan(group.textLeft);
      expect(group.left).toBeCloseTo(layout[0].left);
      expect(group.right).toBeCloseTo(layout[0].right);
      expect(group.right).toBeLessThanOrEqual(width + 1);
      if (index > 0) expect(group.top).toBeGreaterThan(layout[index - 1].bottom);
    }
    expect(await main.evaluate(root => root.scrollWidth - root.clientWidth)).toBeLessThanOrEqual(1);
    const illustrations = main.locator(".ccc-preview-content > svg, svg.ccc-preference-illustration");
    const collapsed = await illustrations.evaluateAll(elements => elements.flatMap((element, index) => {
      const bounds = element.getBoundingClientRect();
      return bounds.width > 0 && bounds.height > 0 ? [] : [{ index, width: bounds.width, height: bounds.height }];
    }));
    expect(collapsed, `${width}px ${theme}`).toEqual([]);

    const switchBounds = await Promise.all([
      appearance.getByRole("switch", { name: "减少动态效果" }).boundingBox(),
      windowSettings.getByRole("switch", { name: "窗口始终置顶" }).boundingBox(),
      runtime.getByRole("switch", { name: "硬件加速" }).boundingBox(),
    ]);
    for (const bounds of switchBounds) expect(bounds.x + bounds.width).toBeCloseTo(switchBounds[0].x + switchBounds[0].width);
    if (width === 1280) {
      const font = await appearance.getByRole("combobox", { name: "选择界面字体" }).boundingBox();
      const name = await appearance.getByText("界面字体", { exact: true }).boundingBox();
      expect(font.x + font.width).toBeCloseTo(switchBounds[0].x + switchBounds[0].width);
      expect(font.x).toBeGreaterThan(name.x);
      await page.screenshot({ path: testInfo.outputPath("settings-desktop.png") });
      await startup.scrollIntoViewIfNeeded();
      await page.screenshot({ path: testInfo.outputPath("settings-startup.png") });
    } else if (width === 360) {
      const choices = await appearance.getByRole("radiogroup", { name: "主界面缩放" }).locator("label").evaluateAll(labels =>
        labels.map(label => label.getBoundingClientRect().top));
      expect(choices[0]).toBeCloseTo(choices[1]);
      expect(choices[2]).toBeCloseTo(choices[3]);
      expect(choices[2]).toBeGreaterThan(choices[0]);
      await main.locator(".ccc-scroll-viewport").evaluate(viewport => { viewport.scrollTop = 0; });
      await page.screenshot({ path: testInfo.outputPath("settings-narrow.png") });
    }
  }
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(set_app_preferences|set_autostart|restart_application)$/.test(call.command)))).toEqual([]);
});

test("日志级别可以直接从设置行调整", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await openWorkspace(page, "设置");

  const writes = () => page.evaluate(() => window.__uiFixture.calls
    .filter(call => call.command === "set_app_preferences").map(call => call.args.settings.runtimeLogLevel));
  expect(await writes()).toEqual([]);
  const runtime = page.getByRole("region", { name: "运行与诊断", exact: true });
  const logLevel = runtime.getByRole("combobox", { name: "日志级别", exact: true });
  await logLevel.press("Enter");
  await page.getByRole("option", { name: "警告" }).click();
  await expect(logLevel).toHaveText("警告");
  expect(await writes()).toEqual(["warn"]);
});
