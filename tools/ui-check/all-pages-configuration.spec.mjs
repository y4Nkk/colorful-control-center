import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";
import { supplyModeFixture } from "./supply-mode-fixture.mjs";
import { installConfigurationFixture } from "./configuration-fixture.mjs";
import { openConfigurationReview } from "./configuration-review.mjs";
const pages = ["概览", "风扇散热", "性能调校", "显示与显卡", "灯光效果", "键盘与输入", "电池与节能", "自动策略", "性能档案", "设置"];
async function setup(page) {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(installConfigurationFixture, supplyModeFixture());
  await page.goto("/");
}
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|set_|enable_|restore_|save_|start_fan|start_battery)/.test(call.command) && call.command !== "set_shortcut_recording"));
async function ready(page, name) {
  if (name === "风扇散热") await expect(page.locator("[data-dust-running]")).toBeVisible();
  if (name === "性能调校") await expect(page.getByRole("slider", { name: "持续功耗上限", exact: true })).toBeVisible();
  if (name === "显示与显卡") await expect(page.locator('[data-mode-frequency="60"]').first()).toBeVisible();
  if (name === "灯光效果") await expect(page.getByRole("combobox", { name: "灯效", exact: true })).toBeVisible();
  if (name === "键盘与输入") await expect(page.getByRole("switch", { name: "Fn 锁", exact: true })).toBeVisible();
  if (name === "电池与节能") await expect(page.getByRole("img", { name: "PD 适配器报告功率 · 100 W", exact: true })).toBeVisible();
  if (name === "自动策略") {
    await expect(page.getByRole("region", { name: "原厂供电模式", exact: true }).locator('[data-power-source="ac"]')).toBeVisible();
    await expect(page.getByRole("switch", { name: "启用自动档案", exact: true })).toBeEnabled();
  }
  if (name === "性能档案") await expect(page.getByRole("region", { name: "档案预览", exact: true }).getByRole("heading", { name: "隔离档案", exact: true })).toBeVisible();
}

for (const theme of ["light", "dark"]) {
  test(`全部十页 ${theme} 宽窄、模块边界与只读浏览`, async ({ page }) => {
    await setup(page);
    await openWorkspace(page, "设置");
    await page.getByRole("radio", { name: theme === "light" ? "浅色" : "深色", exact: true }).check();
    const before = (await writes(page)).length;
    for (const width of [1440, 360]) {
      await page.setViewportSize({ width, height: 1000 });
      for (const [index, name] of pages.entries()) {
        await openWorkspace(page, name);
        if (name === "性能档案") await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
        await ready(page, name);
        const main = page.getByRole("main", { name, exact: true });
        await expect(main.getByRole("heading").first()).toBeVisible();
        const overflow = await main.locator("section,button,input:not([hidden]),.ccc-surface").evaluateAll(elements => elements.filter(element => element.checkVisibility())
          .filter(element => element.getBoundingClientRect().right > innerWidth + 1).map(element => element.getAttribute("aria-label") ?? element.className));
        expect(overflow).toEqual([]);
        const viewport = page.locator(".ccc-workspace > .ccc-scroll-viewport");
        await viewport.evaluate(element => { element.scrollTop = 0; });
        await page.screenshot({ path: `target/ui-check/all-pages-${index}-${theme}-${width}-top.png` });
        await viewport.evaluate(element => { element.scrollTop = element.scrollHeight; });
        await page.screenshot({ path: `target/ui-check/all-pages-${index}-${theme}-${width}-bottom.png` });
      }
    }
    expect((await writes(page)).length).toBe(before);
    expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
  });
}

test("显示SVG跟随IPC变化，读失败撤销画面与执行资格，恢复读取保留草稿", async ({ page }) => {
  await setup(page); await openWorkspace(page, "显示与显卡");
  const panel = page.getByRole("region", { name: "屏幕显示", exact: true });
  await expect(panel.locator('[data-mode-frequency="60"]').first()).toBeVisible();
  await panel.getByRole("combobox", { name: "目标刷新率", exact: true }).click();
  await page.getByRole("option", { name: "144 Hz", exact: true }).click();
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__displayReadFail = true;
    window.__TAURI_INTERNALS__.invoke = (command, args) => command === "read_display_modes" && window.__displayReadFail ? Promise.reject(new Error("隔离读取失败")) : invoke(command, args);
  });
  await expect(panel.getByRole("alert")).toContainText("隔离读取失败");
  await expect(panel.locator("[data-mode-frequency]")).toHaveCount(0);
  await panel.getByRole("button", { name: "重新读取屏幕", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("隔离读取失败");
  await page.evaluate(() => { window.__displayReadFail = false; Object.assign(window.__uiFixture.display.displays[0].current, { width: 1080, height: 1920, orientation: 1 }); });
  await expect(panel.getByRole("combobox", { name: "目标刷新率", exact: true })).toHaveText("144 Hz");
  await expect(panel.locator('[data-mode-orientation="1"]')).toBeVisible();
  await expect(panel.getByRole("alert")).toHaveCount(0);
  expect(await writes(page)).toEqual([]);
});

test("白光SVG区分保存值和草稿，跨页与外部读数更新不混同", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => { window.__uiFixture.family = "white"; });
  await openWorkspace(page, "灯光效果");
  const panel = page.getByRole("region", { name: "键盘灯光", exact: true });
  await expect(panel.getByRole("img", { name: "当前保存亮度：3 级", exact: true })).toBeVisible();
  await panel.getByRole("slider", { name: "亮度", exact: true }).fill("5");
  await expect(panel.getByRole("img", { name: "目标亮度：5 级", exact: true })).toBeVisible();
  await page.evaluate(() => { window.__uiFixture.keyboard.current.brightnessLevel = 2; });
  await openWorkspace(page, "概览"); await openWorkspace(page, "灯光效果");
  await expect(panel.getByRole("img", { name: "目标亮度：5 级", exact: true })).toBeVisible();
  await expect(await openConfigurationReview(page, "键盘灯光")).toContainText("2 级 → 5 级");
  expect(await writes(page)).toEqual([]);
});

test("供电SVG、适配器瓦数与除尘状态只取各自报告，未知不冒充零", async ({ page }) => {
  await setup(page); await openWorkspace(page, "自动策略");
  const supply = page.getByRole("region", { name: "原厂供电模式", exact: true });
  await expect(supply.locator('[data-power-source="ac"]')).toBeVisible();
  await page.evaluate(() => { window.__uiFixture.supplyMode.review.onAc = null; });
  await supply.getByRole("button", { name: "重新读取供电模式", exact: true }).click();
  await expect(supply.locator('[data-power-source="unknown"]')).toBeVisible();
  await openWorkspace(page, "电池与节能");
  const pd = page.getByRole("region", { name: "平台电源", exact: true });
  await expect(pd.getByRole("img", { name: "PD 适配器报告功率 · 100 W", exact: true })).toBeVisible();
  await page.evaluate(() => { window.__uiFixture.platform.powerDelivery.adapterWatts = null; });
  await pd.getByRole("button", { name: "重新读取平台电源", exact: true }).click();
  await expect(pd.getByRole("img", { name: "PD 适配器报告功率 · 未报告功率", exact: true })).toBeVisible();
  await expect(pd).toContainText("未报告"); await expect(pd).not.toContainText("0 W"); await expect(pd).not.toContainText("已插电");
  await openWorkspace(page, "风扇散热");
  const dust = page.getByRole("region", { name: "风扇除尘", exact: true });
  await expect(dust.locator('[data-dust-running="false"]')).toBeVisible();
  await page.evaluate(() => { window.__uiFixture.dust.running = true; });
  await dust.getByRole("button", { name: "重新读取除尘状态", exact: true }).click();
  await expect(dust.locator('[data-dust-running="true"]')).toBeVisible();
  await expect(dust.getByRole("button", { name: "立即除尘", exact: true })).toBeDisabled();
});

test("关灯保留真实档位，读取失败移除白光图形，重试保留亮度草稿", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => { const data = window.__uiFixture; data.family = "white"; data.keyboard.current.enabled = false; data.keyboard.current.brightnessLevel = 4; });
  await openWorkspace(page, "灯光效果");
  const panel = page.getByRole("region", { name: "键盘灯光", exact: true });
  await expect(panel.getByRole("img", { name: "当前保存亮度：已关闭 · 保留 4 级", exact: true })).toBeVisible();
  await expect(panel.locator('[data-backlight-level="4"]')).toHaveAttribute("data-backlight-enabled", "false");
  await panel.getByRole("slider", { name: "亮度", exact: true }).fill("5");
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__lightingReadFail = true;
    window.__TAURI_INTERNALS__.invoke = (command, args) => command === "read_keyboard_lighting" && window.__lightingReadFail ? Promise.reject(new Error("隔离白光读取失败")) : invoke(command, args);
  });
  await openWorkspace(page, "概览"); await openWorkspace(page, "灯光效果");
  await expect(panel.getByRole("alert")).toContainText("隔离白光读取失败");
  await expect(panel.locator("[data-backlight-level]")).toHaveCount(0);
  await page.evaluate(() => { window.__lightingReadFail = false; });
  await panel.getByRole("button", { name: "重新读取键盘灯", exact: true }).click();
  await expect(panel.getByRole("img", { name: "目标亮度：5 级", exact: true })).toBeVisible();
  await expect(panel.getByRole("alert")).toHaveCount(0);
  expect(await writes(page)).toEqual([]);
});

test("三区SVG绑定颜色与亮度草稿，动态固件配色不伪造颜色", async ({ page }) => {
  await setup(page); await openWorkspace(page, "灯光效果");
  const panel = page.getByRole("region", { name: "键盘灯光", exact: true });
  await expect(panel.locator("[data-keyboard-zone]")).toHaveCount(3);
  await panel.getByRole("button", { name: "编辑右区", exact: true }).click();
  await panel.getByLabel("右区颜色", { exact: true }).fill("#123456");
  await expect(panel.locator('[data-keyboard-zone="2"]')).toHaveAttribute("data-zone-color", "#123456");
  await panel.getByRole("slider", { name: "亮度", exact: true }).fill("0");
  await expect(panel.locator('[data-keyboard-zone="2"]')).toHaveAttribute("data-zone-brightness", "0");
  await panel.getByRole("combobox", { name: "灯效", exact: true }).click();
  await page.getByRole("option", { name: "波浪", exact: true }).click();
  await expect(panel.locator('[data-zone-color="firmware"]')).toHaveCount(3);
  await expect(panel.getByText("固件配色", { exact: true })).toBeVisible();
  expect(await writes(page)).toEqual([]);
});
