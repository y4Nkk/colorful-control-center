import { availableAction, configurationView, previewConfiguration, submitConfiguration } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { setPickerColor } from "./color-picker.mjs";

const panelOf = page => page.getByRole("region", { name: "灯光策略与外围灯区", exact: true });
const timeField = (panel, unit) => panel.getByRole("spinbutton", { name: `闲置等待${unit}`, exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply|restore)_lighting_options$/.test(c.command)));
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const options = window.__uiFixture.lightingOptions;
    options.current.logo = { enabled: true, color: { r: 255, g: 0, b: 0 } };
    options.current.lightBar = { enabled: false, color: { r: 0, g: 0, b: 255 } };
    options.capabilities.logo = true; options.capabilities.lightBar = true;
  });
  await page.goto("/"); await openWorkspace(page, "键盘与灯光");
});

test("外围开关即时提交，待编辑颜色和等待时长单独保存", async ({ page }) => {
  const panel = panelOf(page);
  await timeField(panel, "分钟").fill("1");
  await panel.getByRole("switch", { name: "覆盖开机灯效" }).click();
  await expect(panel.getByRole("switch", { name: "覆盖开机灯效" })).toBeChecked();
  await panel.getByRole("switch", { name: "灯条开关" }).click();
  await expect(panel.getByRole("switch", { name: "灯条开关" })).toBeChecked();
  await setPickerColor(panel, "Logo 灯颜色", "#00ff00");
  await previewConfiguration(page, "灯光策略与外围灯区");
  await expect(configurationView(page)).toContainText("30 秒后关灯 → 90 秒后关灯");
  const toggleWrites = await writes(page);
  expect(toggleWrites).toHaveLength(2);
  expect(toggleWrites.map(call => call.args.request.values.idleTimer.seconds)).toEqual([30, 30]);
  await submitConfiguration(await previewConfiguration(page, "灯光策略与外围灯区"));
  const calls = await writes(page);
  expect(calls.at(-1).args.request.values).toMatchObject({ idleTimer: { enabled: true, seconds: 90 }, bootOverride: true,
    logo: { enabled: true, color: { r: 0, g: 255, b: 0 } }, lightBar: { enabled: true } , topCover: null});
  await previewConfiguration(page, "灯光策略与外围灯区", true);
  await expect(configurationView(page, true)).toContainText("恢复目标：30 秒后关灯");
  expect(await writes(page)).toHaveLength(3);
  await configurationView(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(timeField(panel, "分钟")).toHaveValue("0");
  await expect(timeField(panel, "秒数")).toHaveValue("30");
  await expect(panel.getByRole("switch", { name: "覆盖开机灯效" })).not.toBeChecked();
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("草稿变化及新修订撤销确认，重新读取和切页保留草稿", async ({ page }) => {
  const panel = panelOf(page), seconds = timeField(panel, "秒数"), minutes = timeField(panel, "分钟");
  await minutes.fill("1");
  await seconds.fill("0");
  await previewConfiguration(page, "灯光策略与外围灯区");
  await page.keyboard.press("Escape");
  await minutes.fill("2");
  await expect(availableAction(page, "灯光策略与外围灯区")).toBeVisible();
  await previewConfiguration(page, "灯光策略与外围灯区");
  await page.keyboard.press("Escape");
  await page.evaluate(() => { const options = window.__uiFixture.lightingOptions; options.revision = "external"; options.current.idleTimer.seconds = 45; });
  await panel.getByRole("button", { name: "重新读取灯光选项" }).click();
  await expect(await previewConfiguration(page, "灯光策略与外围灯区")).toContainText("45 秒后关灯 → 120 秒后关灯");
  await page.keyboard.press("Escape");
  await expect(availableAction(page, "灯光策略与外围灯区")).toBeVisible();
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await page.getByRole("button", { name: "键盘与灯光", exact: true }).click();
  await expect(minutes).toHaveValue("2");
  await expect(seconds).toHaveValue("0");
  expect(await writes(page)).toHaveLength(0);
});

test("无效时分秒拒绝执行，关闭计时保留上限草稿并发送总秒数", async ({ page }) => {
  const panel = panelOf(page), seconds = timeField(panel, "秒数"), minutes = timeField(panel, "分钟"), hours = timeField(panel, "小时");
  for (const value of ["4", "60", "5.5", ""]) {
    await seconds.fill(value);
    await expect(seconds).toHaveAttribute("aria-invalid", "true");
    await expect(availableAction(page, "灯光策略与外围灯区")).toHaveCount(0);
  }
  await seconds.fill("59");
  await minutes.fill("60");
  await expect(availableAction(page, "灯光策略与外围灯区")).toHaveCount(0);
  await minutes.fill("59");
  await hours.fill("2");
  await expect(availableAction(page, "灯光策略与外围灯区")).toHaveCount(0);
  await hours.fill("1");
  await panel.getByRole("switch", { name: "启用闲置关灯" }).click();
  await expect(seconds).toBeDisabled();
  await expect(hours).toHaveValue("1");
  await expect(minutes).toHaveValue("59");
  await expect(seconds).toHaveValue("59");
  await previewConfiguration(page, "灯光策略与外围灯区");
  await expect(configurationView(page)).toContainText("不自动关灯");
  await submitConfiguration(await previewConfiguration(page, "灯光策略与外围灯区"));
  expect((await writes(page)).map(call => call.args.request.values.idleTimer)).toEqual([
    { enabled: false, seconds: 30 }, { enabled: false, seconds: 7199 },
  ]);
});

test("写入失败保留草稿与初始恢复，读取失败撤销可执行快照", async ({ page }) => {
  const panel = panelOf(page), seconds = timeField(panel, "秒数"), minutes = timeField(panel, "分钟");
  await minutes.fill("1");
  await seconds.fill("15");
  await page.evaluate(() => { window.__uiFixture.lightingOptionsFault = "隔离部分写入失败"; });
  await previewConfiguration(page, "灯光策略与外围灯区");
  await submitConfiguration(await previewConfiguration(page, "灯光策略与外围灯区"));
  await expect(panel.getByRole("alert")).toContainText("隔离部分写入失败");
  await expect(minutes).toHaveValue("1");
  await expect(seconds).toHaveValue("15");
  await expect(availableAction(page, "灯光策略与外围灯区", true)).toBeEnabled();
  await page.evaluate(() => { window.__uiFixture.lightingOptionsReadFault = "隔离读取失败"; });
  await panel.getByRole("button", { name: "重新读取灯光选项" }).click();
  await expect(availableAction(page, "灯光策略与外围灯区")).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.lightingOptionsReadFault = null; });
  await panel.getByRole("button", { name: "重新读取灯光选项" }).click();
  await expect(minutes).toHaveValue("1");
  await expect(seconds).toHaveValue("15");
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
  await expect(availableAction(page, "灯光策略与外围灯区")).toBeEnabled();
});

test("窄屏模块和颜色控件不溢出", async ({ page }) => {
  const panel = panelOf(page);
  for (const width of [1440,760,360]) {
    await page.setViewportSize({ width, height: 1000 });
    await expect(panel.getByRole("switch", { name: "灯条开关" })).toBeVisible();
    const bounds = await panel.evaluate(element => {
      const end = element.getBoundingClientRect().right;
      return { visible: element.clientWidth, content: element.scrollWidth,
        overflowing: [...element.querySelectorAll("*")].filter(child => child.getBoundingClientRect().right > end + 1)
          .map(child => child.getAttribute("class") || child.tagName) };
    });
    expect(bounds.content, `${width}px · ${bounds.overflowing.join(", ")}`).toBeLessThanOrEqual(bounds.visible + 1);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  }
  await page.setViewportSize({ width: 1000, height: 1000 });
  await panel.screenshot({ path: "target/ui-check/lighting-options.png" });
});

test("未知键盘类型不能手动猜测协议，重读识别后才开放", async ({ page }) => {
  await page.addInitScript(() => { window.__uiFixture.family = "unknown"; });
  await page.reload(); await openWorkspace(page, "键盘与灯光");
  const panel = page.getByRole("region", { name: "键盘灯光", exact: true });
  await expect(panel).toContainText("未能识别键盘灯类型");
  await expect(panel.getByRole("group", { name: "键盘类型" })).toHaveCount(0);
  await expect(availableAction(page, "键盘灯光")).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.family = "zoneRgb"; });
  await panel.getByRole("button", { name: "重新读取键盘灯", exact: true }).click();
  const brightness = panel.getByRole("group", { name: "亮度", exact: true });
  const maximum = brightness.getByRole("button", { name: "4 级", exact: true });
  await expect(maximum).toBeEnabled();
  const target = await maximum.getAttribute("aria-pressed") === "true"
    ? brightness.getByRole("button", { name: "关闭", exact: true }) : maximum;
  await target.press("Space");
  await expect(target).toHaveAttribute("aria-pressed", "true");
  await expect(availableAction(page, "键盘灯光")).toHaveCount(0);
  const applied = await page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "apply_keyboard_lighting"));
  expect(applied).toHaveLength(1);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
