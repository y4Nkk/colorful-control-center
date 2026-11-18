import { availableAction, configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { installMultiZoneFixture } from "./multizone-fixture.mjs";

const region = page => page.getByRole("region", { name: "多区键盘灯", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply_multi_zone_lighting|restore_multi_zone_lighting)$/.test(c.command)));
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(installMultiZoneFixture);
  await page.goto("/"); await openWorkspace(page, "灯光效果");
});
async function editColor(page) {
  await region(page).getByLabel("多区第4区颜色", { exact: true }).fill("#112233");
}
async function apply(page) {
  await openConfigurationReview(page, "多区键盘灯");
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
}
test("四区和外围完整预览，编辑零写入且两步确认绑定来源", async ({ page }) => {
  await expect(region(page).getByLabel("当前多区颜色预设")).toContainText("顶盖区");
  await editColor(page);
  expect(await writes(page)).toEqual([]);
  await openConfigurationReview(page, "多区键盘灯");
  expect(await writes(page)).toEqual([]);
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  const calls = await writes(page);
  expect(calls[0].args.request.expectedRevision).toBe("multi-0");
  expect(calls[0].args.request.target.values.zones[3]).toEqual({ r: 17, g: 34, b: 51 });
  await expect(availableAction(page, "多区键盘灯", true)).toBeEnabled();
});
test("旧预览拒绝写入并保留草稿", async ({ page }) => {
  await editColor(page);
  await page.evaluate(() => window.__uiFixture.multiZone.revision = "foreign");
  await apply(page);
  await expect(region(page).getByRole("alert")).toContainText("预览已变化");
  await expect(region(page).getByLabel("多区第4区颜色", { exact: true })).toHaveValue("#112233");
});
test("失败回滚后草稿保留，读取失败撤销执行快照", async ({ page }) => {
  await editColor(page);
  await page.evaluate(() => window.__uiFixture.multiZoneWriteFault = "隔离保存失败");
  await apply(page);
  await expect(region(page).getByRole("alert")).toContainText("本次设置已恢复");
  await expect(region(page).getByLabel("多区第4区颜色", { exact: true })).toHaveValue("#112233");
  await page.evaluate(() => window.__uiFixture.multiZoneReadFault = "隔离读取断开");
  await region(page).getByRole("button", { name: "重新读取多区键盘", exact: true }).click();
  await expect(availableAction(page, "多区键盘灯")).toHaveCount(0);
});
test("独立开关保留亮度与颜色，初始恢复另行确认", async ({ page }) => {
  await region(page).getByRole("switch", { name: "多区键盘灯开关" }).click();
  await apply(page);
  expect(await page.evaluate(() => window.__uiFixture.multiZone.current.brightness)).toBe(4);
  await openConfigurationReview(page, "多区键盘灯", true);
  expect((await writes(page)).length).toBe(1);
  await configurationDialog(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  expect(await page.evaluate(() => window.__uiFixture.multiZone.current.enabled)).toBe(true);
});
test("不支持的灯区隐藏，静态预设保留在动态模式", async ({ page }) => {
  await page.evaluate(() => {
    const p = window.__uiFixture.multiZone;
    p.capabilities.cover = false; p.capabilities.lightBarEnabled = false;
    p.current.cover = null; p.current.lightBar = null;
  });
  await region(page).getByRole("button", { name: "重新读取多区键盘", exact: true }).click();
  await expect(region(page).getByLabel("多区顶盖颜色")).toHaveCount(0);
  await region(page).getByRole("combobox", { name: "多区键盘模式" }).click();
  await page.getByRole("option", { name: "波浪", exact: true }).click();
  await expect(region(page).getByLabel("多区第1区颜色", { exact: true })).toBeDisabled();
  await expect(region(page)).toContainText("平台已关闭灯条区");
});
for (const [theme, width] of [["light", 1440], ["dark", 360]]) {
  test(`多区左右模块布局 ${theme} ${width}`, async ({ page }) => {
    await page.setViewportSize({ width, height: 1500 });
    await page.evaluate(value => document.documentElement.dataset.theme = value, theme);
    await expect(region(page).getByLabel("当前多区颜色预设")).toBeVisible();
    expect(await region(page).evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
    await region(page).screenshot({ path: `target/ui-check/multizone-${theme}-${width}.png` });
  });
}
