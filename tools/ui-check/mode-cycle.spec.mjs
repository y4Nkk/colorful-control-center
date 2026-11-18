import { availableAction, configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { modeCycleFixture } from "./mode-cycle-fixture.mjs";

const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const preview = page => panel(page).getByRole("region", { name: "电源模式循环GPU预览", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
async function setup(page, off = false) {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(fixture => { window.__uiFixture.modeCycleFixture = fixture; }, modeCycleFixture(off));
  await page.goto("/"); await openWorkspace(page, "键盘与输入");
  await panel(page).getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await panel(page).getByRole("checkbox", { name: "电源模式循环", exact: true }).check();
  await panel(page).getByText("电源模式循环联动预览", { exact: true }).click();
}

test("热键模式循环展示全部供电目标及独显阶段，确认只启用监听", async ({ page }) => {
  await setup(page);
  await expect(preview(page).getByRole("region", { name: /循环目标$/ })).toHaveCount(4);
  const saving = preview(page).getByRole("region", { name: "省电循环目标", exact: true });
  await expect(saving).toContainText("切换后 · 确保关闭");
  await expect(saving.getByRole("row", { name: /^插电/ }).getByRole("cell").first()).toHaveText("900");
  await expect(saving.getByRole("row", { name: /^电池/ }).getByRole("cell").first()).toHaveText("900");
  await expect(preview(page).getByRole("region", { name: "性能循环目标", exact: true })).toContainText("切换前 · 确保开启");
  await preview(page).getByText("查看驱动范围", { exact: true }).click();
  await expect(preview(page)).toContainText("可编辑");
  await openConfigurationReview(page, "原厂热键与屏显");
  expect(await calls(page, "apply_hotkeys")).toHaveLength(0);
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  await expect.poll(async () => (await calls(page, "apply_hotkeys")).length).toBe(1);
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(0);
});

test("关闭独显时不补造频率，驱动预检错误保留事实并禁用旧确认", async ({ page }) => {
  await setup(page, true);
  await expect(preview(page).getByText("开启后核对频率", { exact: true })).toBeVisible();
  await preview(page).getByText("设备与恢复说明", { exact: true }).click();
  await expect(preview(page)).toContainText("独显已关闭；驱动范围和实际频率");
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
  await expect(availableAction(page, "原厂热键与屏显")).toHaveCount(0);
  expect(await calls(page, "apply_hotkeys")).toHaveLength(0);
});

test("不可用策略保留未知值，两步清除绑定原文件摘要", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.hotkeys.current = null; data.hotkeys.revision = null;
    data.hotkeys.recordError = "热键策略版本或授权记录无效"; data.hotkeys.resetRevision = "old-file-hash";
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(panel(page)).toContainText("已保存：未知");
  await panel(page).getByRole("button", { name: "清除不可用热键策略", exact: true }).click();
  expect(await calls(page, "reset_invalid_hotkeys")).toHaveLength(0);
  await page.evaluate(() => { window.__uiFixture.hotkeys.resetRevision = "new-file-hash"; });
  await panel(page).getByRole("button", { name: "清除该文件并停止热键监听？", exact: true }).click();
  await expect(panel(page).getByRole("alert").first()).toContainText("不可用热键策略已变化");
  expect(await page.evaluate(() => window.__uiFixture.hotkeys.current)).toBeNull();
  await panel(page).getByRole("button", { name: "清除不可用热键策略", exact: true }).click();
  await panel(page).getByRole("button", { name: "清除该文件并停止热键监听？", exact: true }).click();
  await expect(panel(page)).not.toContainText("已保存：未知");
  await expect(panel(page).getByRole("switch", { name: "启用原厂热键接管", exact: true })).not.toBeChecked();
});

test("模式循环宽窄及深浅布局保留左右模块边框", async ({ page }, testInfo) => {
  await setup(page);
  for (const [width, theme] of [[1440, "light"], [700, "dark"]]) {
    await page.setViewportSize({ width, height: 1100 });
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    await preview(page).scrollIntoViewIfNeeded();
    const boxes = await preview(page).locator(":scope > .ccc-mode-gpu-part").evaluateAll(cards => cards.map(card => {
      const box = card.getBoundingClientRect(); return { x: box.x, y: box.y, width: box.width, border: parseFloat(getComputedStyle(card).borderTopWidth) };
    }));
    expect(boxes).toHaveLength(2); expect(boxes.every(box => box.border > 0)).toBeTruthy();
    if (width === 1440) expect(boxes[1].x).toBeGreaterThan(boxes[0].x + boxes[0].width);
    else expect(boxes[1].y).toBeGreaterThan(boxes[0].y);
    await preview(page).screenshot({ path: testInfo.outputPath(`mode-cycle-${theme}-${width}.png`) });
  }
});
