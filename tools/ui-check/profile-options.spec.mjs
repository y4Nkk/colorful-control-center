import { openProfileResults, openProfileSave, openStartupParameters, saveProfileButton, showProfileMenu, showProfileSettings } from "./profile-review.mjs";
import { changeButton, configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openWorkspace, openSystemArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { readFile } from "node:fs/promises";

const preview = page => page.getByRole("region", { name: "档案预览", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply_|save_|import_|set_startup)/.test(c.command)));
async function selectProfile(page) {
  await openSystemArea(page, "配置档案");
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await showProfileSettings(page);
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const data = window.__uiFixture;
    data.profile.settings.oemPowerMode = 2;
    data.profile.settings.performanceOptions = { turboFan: true, dtt: true };
  });
  await page.goto("/"); await openSystemArea(page, "性能调校");
});

test("档案组合预览包含目标选项，两次确认携带同一目标", async ({ page }) => {
  await selectProfile(page);
  const review = await previewConfiguration(page);
  await expect(preview(page).getByLabel("档案性能选项")).toContainText("随性能模式应用");
  await expect(preview(page).getByLabel("模式联动目标")).toContainText("Turbo 风扇");
  await expect(preview(page).getByLabel("模式联动目标")).toContainText("动态调校 DTT");
  expect(await writes(page)).toEqual([]);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  const results = await openProfileResults(page);
  await expect(results.getByRole("listitem").filter({ hasText: "性能选项" }).getByRole("img", { name: "成功", exact: true })).toBeVisible();
  const [call] = await writes(page);
  expect(call.args.expectedMode).toMatchObject({ mode: 2, turbo: true, dtt: true, fanMode: 0 });
  expect(call.args.expected.settings.performanceOptions).toEqual({ turboFan: true, dtt: true });
});

test("未开放娱乐档应用的固件保存偏好，预览和导出保留开关值", async ({ page }) => {
  await page.evaluate(() => window.__uiFixture.profile.settings.oemPowerMode = 3);
  await selectProfile(page);
  const panel = preview(page);
  await panel.getByLabel("档案性能选项").getByRole("button", { name: "应用顺序", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "应用顺序", exact: true })).toContainText("娱乐与性能模式按固件能力应用性能选项");
  await page.keyboard.press("Escape");
  await expect(panel.getByLabel("模式联动目标")).not.toContainText("动态调校 DTT");
  const download = page.waitForEvent("download");
  await (await showProfileMenu(page)).getByRole("button", { name: "导出档案", exact: true }).click();
  const file = await download;
  const exported = JSON.parse(await readFile(await file.path(), "utf8"));
  expect(exported.settings.performanceOptions).toEqual({ turboFan: true, dtt: true });
  expect(await writes(page)).toEqual([]);
});

test("支持的娱乐档在档案预览和执行中携带两个选项", async ({ page }) => {
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.entertainmentOptions = true;
    data.profile.settings.oemPowerMode = 3;
  });
  await selectProfile(page);
  await previewConfiguration(page);
  const panel = preview(page);
  await expect(panel.getByLabel("档案性能选项")).toContainText("按娱乐模式能力应用");
  await expect(panel.getByLabel("模式联动目标")).toContainText("Turbo 风扇、动态调校 DTT");
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__uiFixture.modeOptionsCommand)).toBe(195);
  expect((await writes(page))[0].args.expectedMode).toMatchObject({ mode: 3, turbo: true, dtt: true });
});

test("捕获保留当前保存偏好并省略固件未声明的选项", async ({ page }) => {
  await page.evaluate(() => {
    window.__uiFixture.performanceOptions.current = { turboFan: null, dtt: true };
    window.__uiFixture.performanceOptions.currentMode = 3;
  });
  await selectProfile(page);
  await openProfileSave(page, "保存偏好", ["性能与散热"]);
  await saveProfileButton(page).click();
  await expect(preview(page).getByRole("heading", { name: "保存偏好" })).toBeVisible();
  const saved = (await writes(page)).find(call => call.command === "save_profile");
  expect(saved.args.args.settings.performanceOptions).toEqual({ turboFan: null, dtt: true });
  await showProfileSettings(page);
  await expect(preview(page).getByLabel("档案性能选项")).not.toContainText("Turbo 风扇");
});

test("驱动在确认后变化时拒绝激活，重新预览显示错误", async ({ page }) => {
  await selectProfile(page);
  await previewConfiguration(page);
  await page.evaluate(() => window.__uiFixture.performanceOptions.dttDriverAvailable = false);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("不能激活DTT");
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await showProfileSettings(page);
  await expect(changeButton(page)).toHaveCount(0);
  await expect(preview(page).getByRole("button", { name: /^执行明细/ })).toHaveCount(0);
});

test("启动选择展示组合联动，保存完整文件且不发送应用", async ({ page }) => {
  await openWorkspace(page, "电源");
  const panel = page.getByRole("region", { name: "启动档案", exact: true });
  await panel.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  const parameters = await openStartupParameters(page);
  await expect(parameters.getByLabel("模式联动目标")).toContainText("动态调校 DTT");
  await expect(parameters).toContainText("启动时保存插电偏好并应用模式的固件节能联动");
  await page.keyboard.press("Escape");
  await configurationView(page).getByRole("button", { name: "授权启动应用档案", exact: true }).click();
  const calls = await writes(page);
  expect(calls.map(call => call.command)).toEqual(["set_startup_profile"]);
  expect(calls[0].args.expected.settings.performanceOptions).toEqual({ turboFan: true, dtt: true });
});

test("保留开启DTT但驱动未知时，先关闭DTT才能应用其他选项", async ({ page }) => {
  const panel = page.getByRole("region", { name: "性能选项", exact: true });
  await page.evaluate(() => {
    const options = window.__uiFixture.performanceOptions;
    options.current.dtt = true; options.dttDriverAvailable = null;
  });
  await panel.getByRole("button", { name: "重新读取性能选项", exact: true }).click();
  const turbo = panel.getByRole("switch", { name: "Turbo 风扇", exact: true });
  const dtt = panel.getByRole("switch", { name: "动态调校 DTT", exact: true });
  await expect(turbo).toHaveAttribute("aria-disabled", "true");
  await expect(panel).toContainText("请确认驱动或关闭 DTT");
  expect(await writes(page)).toEqual([]);
  await dtt.click();
  await expect(dtt).not.toBeChecked();
  await expect(turbo).toBeEnabled();
  await turbo.click();
  await expect(turbo).toBeChecked();
  expect(await writes(page)).toMatchObject([
    { command: "apply_oem_performance_options", args: { request: { values: { turboFan: false, dtt: false } } } },
    { command: "apply_oem_performance_options", args: { request: { values: { turboFan: true, dtt: false } } } },
  ]);
});

test("配置档案卡片在宽窄及深浅主题下保持模块边框和可读布局", async ({ page }) => {
  await selectProfile(page);
  const panel = preview(page).getByLabel("档案性能选项");
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 1440]) {
      await page.setViewportSize({ width, height: 1800 });
      await panel.scrollIntoViewIfNeeded();
      expect(await panel.evaluate(el => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
      await expect(panel).toHaveAttribute("data-surface", "group");
      await panel.screenshot({ path: `target/ui-check/profile-options-${theme}-${width}.png` });
    }
  }
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
