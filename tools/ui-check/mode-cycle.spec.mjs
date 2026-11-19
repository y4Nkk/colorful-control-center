import { availableAction, configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { modeCycleFixture } from "./mode-cycle-fixture.mjs";

const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const dialog = (page, name) => page.getByRole("dialog", { name, exact: true });
const details = page => dialog(page, "热键状态与诊断");
const modeReview = page => dialog(page, "电源模式循环联动预览");
const preview = page => modeReview(page).getByRole("region", { name: "电源模式循环GPU预览", exact: true });
const edit = page => panel(page).getByRole("button", { name: "配置热键", exact: true }).click();
const openDetails = page => panel(page).getByRole("button", { name: "详情", exact: true }).click();
const close = view => view.getByRole("button", { name: "关闭弹窗", exact: true }).click();
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
async function setup(page, off = false) {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(fixture => { window.__uiFixture.modeCycleFixture = fixture; }, modeCycleFixture(off));
  await page.goto("/"); await openWorkspace(page, "键盘与灯光");
  await edit(page);
  const editor = dialog(page, "配置热键与屏显");
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await editor.getByRole("checkbox", { name: "电源模式循环", exact: true }).check();
  await editor.getByRole("button", { name: "完成编辑", exact: true }).click();
  await openDetails(page);
  await details(page).getByRole("button", { name: "电源模式循环联动预览", exact: true }).click();
}

test("热键模式循环展示全部供电目标及独显阶段，确认只启用监听", async ({ page }) => {
  await setup(page);
  await expect(preview(page).getByRole("region", { name: /循环目标$/ })).toHaveCount(4);
  const saving = preview(page).getByRole("region", { name: "省电循环目标", exact: true });
  await expect(saving).toContainText("切换后 · 确保关闭");
  await expect(saving.getByRole("row", { name: /^插电/ }).getByRole("cell").first()).toHaveText("900");
  await expect(saving.getByRole("row", { name: /^电池/ }).getByRole("cell").first()).toHaveText("900");
  await expect(preview(page).getByRole("region", { name: "性能循环目标", exact: true })).toContainText("切换前 · 确保开启");
  await preview(page).getByRole("button", { name: "查看驱动范围", exact: true }).click();
  const driver = dialog(page, "查看驱动范围");
  await expect(driver).toContainText("可编辑");
  await close(driver);
  await close(modeReview(page));
  await close(details(page));
  await previewConfiguration(page, "原厂热键与屏显");
  expect(await calls(page, "apply_hotkeys")).toHaveLength(0);
  await configurationView(page).getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  await expect.poll(async () => (await calls(page, "apply_hotkeys")).length).toBe(1);
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(0);
});

test("关闭独显时不补造频率，驱动预检错误保留事实并禁用旧确认", async ({ page }) => {
  await setup(page, true);
  await expect(preview(page).getByText("开启后核对频率", { exact: true })).toBeVisible();
  await preview(page).getByRole("button", { name: "设备与恢复说明", exact: true }).click();
  const recovery = dialog(page, "设备与恢复说明");
  await expect(recovery).toContainText("独显已关闭；驱动范围和实际频率");
  await close(recovery);
  await expect(preview(page).getByText(/^图形锁频：/)).toHaveCount(0);
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.hotkeys.facts.modeCycle.clocks.driverError = "隔离驱动范围不可用";
    const cap = data.hotkeys.facts.capabilities.find(cap => cap.scope === "powerMode");
    cap.identity = null; cap.reason = "隔离驱动范围不可用";
    data.hotkeys.revision += "-unavailable";
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(preview(page).getByRole("alert")).toContainText("隔离驱动范围不可用");
  await close(modeReview(page));
  await close(details(page));
  await expect(availableAction(page, "原厂热键与屏显")).toHaveCount(0);
  expect(await calls(page, "apply_hotkeys")).toHaveLength(0);
});

test("不可用策略保留未知值，两步清除绑定原文件摘要", async ({ page }) => {
  await setup(page);
  await close(modeReview(page));
  await close(details(page));
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.hotkeys.current = null; data.hotkeys.revision = null;
    data.hotkeys.recordError = "热键策略版本或授权记录无效"; data.hotkeys.resetRevision = "old-file-hash";
    window.__uiEmit("hardware-controls-changed", null);
  });
  const reset = panel(page).getByRole("button", { name: "清除不可用热键策略", exact: true });
  const confirm = dialog(page, "清除不可用热键策略").getByRole("button", { name: "清除并停止监听", exact: true });
  await openDetails(page);
  await expect(details(page)).toContainText("已保存 · 未知");
  await close(details(page));
  await reset.click();
  expect(await calls(page, "reset_invalid_hotkeys")).toHaveLength(0);
  await page.evaluate(() => { window.__uiFixture.hotkeys.resetRevision = "new-file-hash"; });
  await confirm.click();
  await openDetails(page);
  await expect(details(page)).toContainText("不可用热键策略已变化");
  await close(details(page));
  expect(await page.evaluate(() => window.__uiFixture.hotkeys.current)).toBeNull();
  await reset.click();
  await confirm.click();
  await expect(reset).toHaveCount(0);
  await edit(page);
  await expect(dialog(page, "配置热键与屏显").getByRole("switch", { name: "启用原厂热键接管", exact: true })).not.toBeChecked();
});

test("模式循环宽窄及深浅布局保留左右模块边框", async ({ page }, testInfo) => {
  await setup(page);
  for (const [width, theme] of [[1440, "light"], [360, "dark"]]) {
    await page.setViewportSize({ width, height: 1100 });
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    await preview(page).scrollIntoViewIfNeeded();
    const boxes = await preview(page).locator(":scope > .ccc-mode-gpu-part").evaluateAll(cards => cards.map(card => {
      const box = card.getBoundingClientRect(); return { x: box.x, y: box.y, width: box.width, border: parseFloat(getComputedStyle(card).borderTopWidth) };
    }));
    expect(boxes).toHaveLength(2); expect(boxes.every(box => box.border > 0)).toBeTruthy();
    const artwork = preview(page).locator("svg[data-mode-art]");
    await expect(artwork).toHaveCount(4);
    const sizes = await artwork.evaluateAll(items => items.map(svg => svg.getBoundingClientRect().toJSON()));
    expect(sizes.every(size => size.width === 32 && size.height === 32)).toBe(true);
    if (width === 1440) expect(boxes[1].x).toBeGreaterThan(boxes[0].x + boxes[0].width);
    else expect(boxes[1].y).toBeGreaterThan(boxes[0].y);
    await preview(page).screenshot({ path: testInfo.outputPath(`mode-cycle-${theme}-${width}.png`) });
  }
});
