import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panelOf = page => page.getByRole("region", { name: "灯光策略与外围灯区", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply|restore)_lighting_options$/.test(c.command)));
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const options = window.__uiFixture.lightingOptions;
    options.current.logo = { enabled: true, color: { r: 255, g: 0, b: 0 } };
    options.current.lightBar = { enabled: false, color: { r: 0, g: 0, b: 255 } };
    options.capabilities.logo = true; options.capabilities.lightBar = true;
  });
  await page.goto("/"); await openWorkspace(page, "灯光效果");
});

test("闲置、开机及外围颜色先预览，确认后保存并能恢复初始", async ({ page }) => {
  const panel = panelOf(page);
  await panel.getByRole("spinbutton", { name: "闲置等待秒数" }).fill("90");
  await panel.getByRole("switch", { name: "覆盖开机灯效" }).check();
  await panel.getByRole("switch", { name: "灯条开关" }).check();
  await panel.getByLabel("Logo 灯颜色", { exact: true }).fill("#00ff00");
  await expect(panel).toContainText("30 秒后关灯 → 90 秒后关灯");
  await panel.getByRole("button", { name: "应用灯光选项", exact: true }).click();
  expect(await writes(page)).toHaveLength(0);
  await panel.getByRole("button", { name: "确认应用灯光选项？", exact: true }).click();
  const calls = await writes(page);
  expect(calls[0].args.request.values).toMatchObject({ idleTimer: { enabled: true, seconds: 90 }, bootOverride: true,
    logo: { enabled: true, color: { r: 0, g: 255, b: 0 } }, lightBar: { enabled: true } , topCover: null});
  await expect(panel).toContainText("恢复目标：30 秒后关灯");
  await panel.getByRole("button", { name: "恢复灯光选项初始", exact: true }).click();
  expect(await writes(page)).toHaveLength(1);
  await panel.getByRole("button", { name: "恢复本会话修改前的灯光选项？", exact: true }).click();
  await expect(panel.getByRole("spinbutton", { name: "闲置等待秒数" })).toHaveValue("30");
  await expect(panel.getByRole("switch", { name: "覆盖开机灯效" })).not.toBeChecked();
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("草稿变化及新修订撤销确认，重新读取和切页保留草稿", async ({ page }) => {
  const panel = panelOf(page), seconds = panel.getByRole("spinbutton", { name: "闲置等待秒数" });
  await seconds.fill("60");
  await panel.getByRole("button", { name: "应用灯光选项", exact: true }).click();
  await seconds.fill("120");
  await expect(panel.getByRole("button", { name: "应用灯光选项", exact: true })).toBeVisible();
  await panel.getByRole("button", { name: "应用灯光选项", exact: true }).click();
  await page.evaluate(() => { const options = window.__uiFixture.lightingOptions; options.revision = "external"; options.current.idleTimer.seconds = 45; });
  await panel.getByRole("button", { name: "重新读取灯光选项" }).click();
  await expect(panel).toContainText("45 秒后关灯 → 120 秒后关灯");
  await expect(panel.getByRole("button", { name: "应用灯光选项", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await page.getByRole("button", { name: "灯光效果", exact: true }).click();
  await expect(seconds).toHaveValue("120");
  expect(await writes(page)).toHaveLength(0);
});

test("无效秒数拒绝执行，关闭计时不清除时间草稿", async ({ page }) => {
  const panel = panelOf(page), seconds = panel.getByRole("spinbutton", { name: "闲置等待秒数" });
  for (const value of ["4", "7200", "5.5"]) {
    await seconds.fill(value);
    await expect(panel.getByRole("button", { name: "应用灯光选项", exact: true })).toBeDisabled();
  }
  await seconds.fill("7199");
  await panel.getByRole("switch", { name: "启用闲置关灯" }).uncheck();
  await expect(seconds).toBeDisabled();
  await expect(seconds).toHaveValue("7199");
  await expect(panel).toContainText("不自动关灯");
  await panel.getByRole("button", { name: "应用灯光选项", exact: true }).click();
  await panel.getByRole("button", { name: "确认应用灯光选项？", exact: true }).click();
  expect((await writes(page))[0].args.request.values.idleTimer).toEqual({ enabled: false, seconds: 7199 });
});

test("写入失败保留草稿与初始恢复，读取失败撤销可执行快照", async ({ page }) => {
  const panel = panelOf(page), seconds = panel.getByRole("spinbutton", { name: "闲置等待秒数" });
  await seconds.fill("75");
  await page.evaluate(() => { window.__uiFixture.lightingOptionsFault = "隔离部分写入失败"; });
  await panel.getByRole("button", { name: "应用灯光选项", exact: true }).click();
  await panel.getByRole("button", { name: "确认应用灯光选项？", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("隔离部分写入失败");
  await expect(seconds).toHaveValue("75");
  await expect(panel.getByRole("button", { name: "恢复灯光选项初始", exact: true })).toBeEnabled();
  await page.evaluate(() => { window.__uiFixture.lightingOptionsReadFault = "隔离读取失败"; });
  await panel.getByRole("button", { name: "重新读取灯光选项" }).click();
  await expect(panel.getByRole("button", { name: "应用灯光选项", exact: true })).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.lightingOptionsReadFault = null; });
  await panel.getByRole("button", { name: "重新读取灯光选项" }).click();
  await expect(seconds).toHaveValue("75");
});

test("白光及逐键仅提供适用选项，未知外围能力不伪造设备", async ({ page }) => {
  const panel = panelOf(page);
  await page.evaluate(() => {
    const options = window.__uiFixture.lightingOptions;
    options.current.logo = null; options.current.lightBar = null; options.current.bootOverride = null;
    options.capabilities = { keyboardType: 3, peripheralSupportKnown: false, logo: false, lightBar: false, bootOverride: false , topCover: false};
    options.revision = "perkey";
  });
  await panel.getByRole("button", { name: "重新读取灯光选项" }).click();
  await expect(panel).toContainText("外围灯区能力未知");
  await expect(panel.getByRole("switch", { name: "Logo 灯开关" })).toHaveCount(0);
  await expect(panel.getByRole("switch", { name: "覆盖开机灯效" })).toHaveCount(0);
  await expect(panel.getByRole("switch", { name: "启用闲置关灯" })).toBeVisible();
  await panel.getByRole("spinbutton", { name: "闲置等待秒数" }).fill("45");
  await expect(panel.getByRole("button", { name: "应用灯光选项", exact: true })).toBeEnabled();
});

test("窄屏模块和颜色控件不溢出", async ({ page }) => {
  const panel = panelOf(page);
  for (const width of [1440,760,360]) {
    await page.setViewportSize({ width, height: 1000 });
    await expect(panel.getByRole("switch", { name: "灯条开关" })).toBeVisible();
    expect(await panel.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  }
  await page.setViewportSize({ width: 1000, height: 1000 });
  await panel.screenshot({ path: "target/ui-check/lighting-options.png" });
});

test("未知键盘类型不能手动猜测协议，重读识别后才开放", async ({ page }) => {
  await page.addInitScript(() => { window.__uiFixture.family = "unknown"; });
  await page.reload(); await openWorkspace(page, "灯光效果");
  const panel = page.getByRole("region", { name: "键盘灯光", exact: true });
  await expect(panel).toContainText("未能识别键盘灯类型");
  await expect(panel.getByRole("group", { name: "键盘类型" })).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "应用", exact: true })).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.family = "zoneRgb"; });
  await panel.getByRole("button", { name: "重新读取键盘灯", exact: true }).click();
  await expect(panel.getByRole("button", { name: "应用", exact: true })).toBeEnabled();
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply|restore)_keyboard_lighting$/.test(c.command)))).toEqual([]);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
