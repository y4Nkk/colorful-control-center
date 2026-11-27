import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace as navigate, openPowerArea } from "./workspace.mjs";

const pages = ["概览", "性能与散热", "键盘与灯光", "电源与档案", "设备配置", "设置"];
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); });
test("所有工作区共用品牌顶栏，浏览不写硬件", async ({ page }) => {
  const banner = page.getByRole("banner");
  await expect(banner.locator(".ccc-brand-icon")).toBeVisible();
  for (const name of pages) {
    await navigate(page, name);
    await expect(page.getByRole("button", { name, exact: true })).toHaveAttribute("aria-current", "page");
    await expect(banner.getByRole("heading", { name: "Colorful Control Center", level: 1, exact: true })).toBeVisible();
    await expect(banner.getByText(name, { exact: true })).toHaveCount(0);
  }
  await navigate(page, "概览");
  await expect(page.getByRole("img", { name: /最近 5 分钟温度趋势/ })).toHaveCount(2);
  await expect(page.locator("[data-configuration-apply]")).toHaveCount(0);
  const writes = await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|set_|enable_|restore_)/.test(call.command) && call.command !== "set_shortcut_recording"));
  expect(writes).toEqual([]);
});

test("电源页签用键盘移动焦点、激活面板并恢复选中项", async ({ page }) => {
  await openPowerArea(page, "电源设置");
  const tabs = page.getByRole("tablist", { name: "电源区域" });
  const settings = tabs.getByRole("tab", { name: "电源设置" });
  const profiles = tabs.getByRole("tab", { name: "配置档案" });
  const panel = page.getByRole("tabpanel", { name: "电源设置" });
  await expect(settings).toHaveAttribute("aria-controls", await panel.getAttribute("id"));
  await expect(panel).toHaveAttribute("aria-labelledby", await settings.getAttribute("id"));
  await settings.press("ArrowLeft");
  await expect(profiles).toBeFocused();
  await expect(panel).toBeVisible();
  await page.keyboard.press("Tab");
  await expect(panel).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(settings).toBeFocused();
  await page.keyboard.press("End");
  await expect(profiles).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("tabpanel", { name: "配置档案" })).toBeVisible();
  await expect(panel).toBeHidden();
  await page.keyboard.press("Home");
  await expect(settings).toBeFocused();
  await page.keyboard.press("Space");
  await expect(panel).toBeVisible();
});

test("概览与性能散热独立，电源下的档案预览在切页后保留", async ({ page }) => {
  const nav = page.getByRole("navigation", { name: "主导航" });
  const names = ["系统", "风扇散热", "性能调校", "配置档案"];
  for (const name of names)
    await expect(nav.getByRole("button", { name, exact: true })).toHaveCount(0);
  await navigate(page, "概览");
  const overview = page.getByRole("region", { name: "概览", exact: true });
  for (const [part, label, clock] of [["CPU", "活跃频率", "2900 MHz"], ["GPU", "当前频率", "1800 MHz"]]) {
    const card = overview.getByRole("region", { name: `${part} 运行读数`, exact: true });
    await expect(card.getByLabel(label, { exact: true })).toHaveText(clock);
    await expect(card.getByRole("img", { name: new RegExp(`^${part} 最近 5 分钟温度趋势`) })).toBeVisible();
  }
  await expect(page.getByRole("region", { name: "当前模式", exact: true })).toBeVisible();
  await expect(page.getByRole("region", { name: "预设模式", exact: true })).toHaveCount(0);
  await navigate(page, "性能与散热");
  await expect(page.getByRole("main", { name: "性能与散热", exact: true })).toBeVisible();
  await expect(page.getByRole("region", { name: "概览", exact: true })).toBeHidden();
  await expect(page.getByRole("region", { name: "预设模式", exact: true })).toBeVisible();
  await expect(page.getByRole("slider", { name: "持续功耗上限", exact: true })).toBeVisible();
  await expect(page.getByRole("region", { name: "散热控制", exact: true }).getByRole("meter", { name: /^CPU (?:实测|上次)转速$/ }))
    .toHaveAttribute("aria-valuetext", "1800 RPM");
  const offset = page.getByRole("slider", { name: "加速偏移", exact: true });
  await offset.fill("38");
  await expect.poll(() => page.evaluate(() => window.__uiFixture.fan.offsetPct)).toBe(38);
  await expect(offset).toBeEnabled();
  await openPowerArea(page, "配置档案");
  await expect(page.getByRole("tabpanel", { name: "配置档案", exact: true })).toBeVisible();
  const profileApply = page.getByRole("region", { name: "档案预览", exact: true }).getByRole("button", { name: "应用档案", exact: true });
  const selected = page.getByRole("region", { name: "档案列表", exact: true }).getByRole("button", { name: /隔离档案/ });
  await expect(selected).toHaveAttribute("aria-pressed", "true");
  await expect(profileApply).toBeEnabled();
  await navigate(page, "性能与散热");
  await expect(page.getByRole("slider", { name: "持续功耗上限", exact: true })).toBeVisible();
  await expect(page.locator('section[aria-label="预设模式"]')).toHaveCount(1);
  await expect(page.getByRole("region", { name: "预设模式", exact: true })).toBeVisible();
  await expect(offset).toHaveValue("38");
  await expect(profileApply).toHaveCount(0);
  await openPowerArea(page, "配置档案");
  await expect(selected).toHaveAttribute("aria-pressed", "true");
  await expect(profileApply).toBeEnabled();
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(set_fan|apply_fan|apply_profile)/.test(call.command))))
    .toEqual([{ command: "set_fan_offset", args: { expected: { mode: 0, slot: null }, pct: 38 } }]);
});

test("托盘请求打开电源下的配置档案并预览指定档案", async ({ page }) => {
  await navigate(page, "设备配置");
  await page.evaluate(() => window.__uiEmit("tray-navigation-request", { kind: "profile", name: "隔离档案" }));
  const power = page.getByRole("main", { name: "电源与档案", exact: true });
  await expect(power.getByRole("tab", { name: "配置档案", exact: true })).toHaveAttribute("aria-selected", "true");
  const preview = power.getByRole("region", { name: "档案预览", exact: true });
  await expect(preview.getByRole("heading", { name: "隔离档案", exact: true })).toBeVisible();
  await expect(preview.getByRole("button", { name: "应用档案", exact: true })).toBeEnabled();
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "apply_profile"))).toEqual([]);
});

test("托盘风扇设置定位散热，硬件回读事件更新已打开的页面", async ({ page }) => {
  await navigate(page, "设备配置");
  await page.evaluate(() => window.__uiEmit("tray-navigation-request", { kind: "cooling" }));
  const performance = page.getByRole("main", { name: "性能与散热", exact: true });
  const cooling = performance.getByRole("region", { name: "散热控制", exact: true });
  await expect(cooling.getByRole("radio", { name: "自动", exact: true })).toBeChecked();
  await expect(cooling.getByRole("heading", { name: "散热控制", exact: true })).toBeInViewport();
  await page.evaluate(() => {
    window.__uiFixture.fan.currentMode = 1;
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(cooling.getByRole("radio", { name: "全速", exact: true })).toBeChecked();
  const presets = page.getByRole("radiogroup", { name: "预设模式选择", exact: true });
  await expect(presets.getByRole("radio", { name: "性能", exact: true })).toBeChecked();
  await page.evaluate(() => {
    window.__uiFixture.performanceOptions.currentMode = 1;
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(presets.getByRole("radio", { name: "省电", exact: true })).toBeChecked();
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call =>
    ["set_fan_mode", "apply_oem_power_mode"].includes(call.command)))).toEqual([]);
});

for (const legacy of ["profiles", "dashboard:profiles"]) test(`旧档案访问记录 ${legacy} 转到电源的配置档案`, async ({ page }) => {
  await page.evaluate(value => localStorage.setItem("ccc-last-page", value), legacy);
  await page.addInitScript(() => { window.__uiFixture.preferences.startupPage = "lastVisited"; });
  for (let attempt = 0; attempt < 2; attempt++) {
    await page.reload();
    const power = page.getByRole("main", { name: "电源与档案", exact: true });
    await expect(power.getByRole("tab", { name: "配置档案", exact: true })).toHaveAttribute("aria-selected", "true");
    await expect(power.getByRole("button", { name: "保存当前配置", exact: true })).toBeVisible();
    expect(await page.evaluate(() => localStorage.getItem("ccc-last-page"))).toBe("power:profiles");
  }
});

test("上次访问的设备配置仍能在启动时打开", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("ccc-last-page", "graphics");
    window.__uiFixture.preferences.startupPage = "lastVisited";
  });
  await page.reload();
  await expect(page.getByRole("main", { name: "设备配置", exact: true })).toBeVisible();
  await expect(page.getByRole("region", { name: "屏幕显示", exact: true })).toBeVisible();
  await expect(page.getByRole("region", { name: "键盘功能", exact: true })).toBeVisible();
});
for (const savedPage of ["lighting", "input"]) {
  test(`原键盘页面 ${savedPage} 的最近访问记录指向合并页`, async ({ page }) => {
    await page.addInitScript(value => {
      localStorage.setItem("ccc-last-page", value);
      window.__uiFixture.preferences.startupPage = "lastVisited";
    }, savedPage);
    await page.reload();
    await expect(page.getByRole("main", { name: "键盘与灯光", exact: true })).toBeVisible();
    expect(await page.evaluate(() => localStorage.getItem("ccc-last-page"))).toBe("keyboard");
  });
}

test("键盘与灯光共用一个入口，切页保留草稿并在各模块应用", async ({ page }) => {
  await page.evaluate(() => {
    window.__uiFixture.keymap = { revision: "keymap-0", configuration: null, enabledFlags: [0, 0], sourceName: "原厂配置",
      baselineName: null, canRestore: false, recoveryRequired: false, blockedReason: null, warnings: [],
      programsRequired: false, programs: { running: false, error: null } };
  });
  await navigate(page, "键盘与灯光");
  const nav = page.getByRole("navigation", { name: "主导航" });
  for (const title of ["灯光效果", "按键与宏"]) {
    await expect(nav.getByRole("button", { name: title, exact: true })).toHaveCount(0);
  }
  const main = page.getByRole("main", { name: "键盘与灯光", exact: true });
  const hotkeys = main.getByRole("region", { name: "原厂热键与屏显", exact: true });
  for (const title of ["键盘灯光", "灯光策略与外围灯区"]) {
    await expect(main.getByRole("region", { name: title, exact: true })).toBeVisible();
  }
  await expect(main.getByRole("region", { name: "按键映射与宏", exact: true })).toContainText("缺少可恢复的原配置");
  const toggle = page.getByRole("switch", { name: "启用闲置关灯", exact: true });
  await toggle.check();
  await expect(toggle).toBeChecked();
  await page.getByRole("spinbutton", { name: "闲置等待分钟", exact: true }).fill("2");
  await page.getByRole("spinbutton", { name: "闲置等待秒数", exact: true }).fill("5");
  await hotkeys.getByRole("button", { name: "配置热键", exact: true }).click();
  const editor = page.getByRole("dialog", { name: "配置热键与屏显", exact: true });
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await editor.getByRole("checkbox", { name: "打开键盘灯区域", exact: true }).check();
  await editor.getByRole("button", { name: "完成编辑", exact: true }).click();
  await navigate(page, "性能与散热");
  await navigate(page, "键盘与灯光");
  await expect(page.getByRole("spinbutton", { name: "闲置等待分钟", exact: true })).toHaveValue("2");
  await expect(page.getByRole("spinbutton", { name: "闲置等待秒数", exact: true })).toHaveValue("5");
  await expect(page.locator('section[aria-label="灯光策略与外围灯区"]')).toHaveCount(1);
  const review = hotkeys.getByRole("region", { name: "原厂热键与屏显 · 保存热键接管范围", exact: true });
  const lighting = main.getByRole("region", { name: "灯光策略与外围灯区 · 保存灯光参数", exact: true });
  await expect(review).toBeVisible();
  await expect(lighting).toBeVisible();
  const applied = () => page.evaluate(() => window.__uiFixture.calls
    .map(call => call.command).filter(command => /^(apply_hotkeys|apply_lighting_options)$/.test(command)));
  expect(await applied()).toEqual([]);
  await review.getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  await expect.poll(applied).toEqual(["apply_hotkeys"]);
  await expect(lighting).toBeVisible();
});

test("每个分类保留自己的滚动位置", async ({ page }) => {
  await navigate(page, "键盘与灯光");
  const viewport = page.locator(".ccc-workspace > .ccc-scroll-viewport");
  await expect(page.getByRole("switch", { name: "启用闲置关灯", exact: true })).toBeVisible();
  await viewport.evaluate(element => { element.scrollTop = 120; });
  await expect.poll(() => viewport.evaluate(element => element.scrollTop)).toBe(120);
  await page.waitForTimeout(100);
  await navigate(page, "性能与散热");
  await expect.poll(() => viewport.evaluate(element => element.scrollTop)).toBe(0);
  await navigate(page, "键盘与灯光");
  await expect.poll(() => viewport.evaluate(element => element.scrollTop)).toBe(120);
});

for (const theme of ["light", "dark"]) {
  test(`分类页面 ${theme} 主题与窄窗口`, async ({ page }) => {
    await navigate(page, "设置");
    await page.getByRole("radio", { name: theme === "light" ? "浅色" : "深色", exact: true }).click();
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
    for (const width of [1440, 360]) {
      await page.setViewportSize({ width, height: 960 });
      let powerTabsX;
      for (const [name, area] of [["概览"], ["性能与散热"], ["键盘与灯光"], ["设备配置"], ["电源与档案", "电源设置"], ["电源与档案", "配置档案"]]) {
        if (area) await openPowerArea(page, area);
        else await navigate(page, name);
        const main = page.getByRole("main", { name, exact: true });
        const bounds = await main.boundingBox();
        expect(bounds.x + bounds.width).toBeLessThanOrEqual(width + 1);
        const powerTabs = main.getByRole("tablist", { name: "电源区域", exact: true });
        if (area === "电源设置") powerTabsX = (await powerTabs.boundingBox()).x;
        if (area === "配置档案") {
          expect((await powerTabs.boundingBox()).x).toBe(powerTabsX);
          const tabs = powerTabs.getByRole("tab");
          const tops = await tabs.evaluateAll(options => options.map(option => option.getBoundingClientRect().top));
          const rows = [...new Set(tops)].map(top => tops.filter(value => value === top).length);
          expect(rows).toEqual([2]);
        }
        await page.screenshot({ path: `target/ui-check/navigation-${theme}-${width}-${area ?? pages.indexOf(name)}.png` });
      }
    }
  });
}
