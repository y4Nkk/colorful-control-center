import { availableAction, configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "麦克风静音", exact: true });
const current = page => panel(page).getByRole("region", { name: "当前麦克风状态", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply|restore)_microphones$/.test(call.command)));
async function apply(page) {
  await openConfigurationReview(page, "麦克风");
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
}
async function restore(page) {
  await openConfigurationReview(page, "麦克风", true);
  await configurationDialog(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
}
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与输入"); });

test("混合状态逐项显示，先预览确认后统一静音并恢复原状态", async ({ page }) => {
  await expect(current(page)).toContainText("内置麦克风未静音");
  await expect(current(page)).toContainText("USB麦克风已静音");
  await expect(availableAction(page, "麦克风")).toHaveCount(0);
  await panel(page).getByRole("radio", { name: "全部静音", exact: true }).check();
  await expect(panel(page)).toContainText("未静音 → 已静音");
  await openConfigurationReview(page, "麦克风");
  expect(await writes(page)).toHaveLength(0);
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  expect((await writes(page))[0].args.request.muted).toBe(true);
  await expect(current(page).getByText("已静音", { exact: true })).toHaveCount(2);
  await expect(panel(page)).toContainText("恢复目标：内置麦克风 未静音；USB麦克风 已静音");
  await restore(page);
  await expect(current(page)).toContainText("内置麦克风未静音");
  await expect(current(page)).toContainText("USB麦克风已静音");
  await expect(availableAction(page, "麦克风", true)).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("新修订撤销确认，热键刷新保留尚未应用的目标", async ({ page }) => {
  await panel(page).getByRole("radio", { name: "全部静音", exact: true }).check();
  await openConfigurationReview(page, "麦克风");
  await page.evaluate(() => { const p = window.__uiFixture.microphones; p.current.endpoints[0].muted = true; p.revision = "external-mic"; });
  await panel(page).getByRole("button", { name: "重新读取麦克风", exact: true }).click();
  await expect(availableAction(page, "麦克风")).toBeVisible();
  expect(await writes(page)).toHaveLength(0);
  await page.evaluate(() => { const p = window.__uiFixture.microphones; p.current.endpoints.forEach(endpoint => endpoint.muted = false); p.revision = "hotkey-mic"; window.__uiEmit("hardware-controls-changed", null); });
  await expect(current(page).getByText("未静音", { exact: true })).toHaveCount(2);
  await expect(panel(page).getByRole("radio", { name: "全部静音", exact: true })).toBeChecked();
});

test("写入及恢复失败保留目标与首次基线，重试后恢复混合状态", async ({ page }) => {
  await panel(page).getByRole("radio", { name: "全部静音", exact: true }).check();
  await page.evaluate(() => window.__uiFixture.microphoneWriteFault = "隔离麦克风写入失败");
  await apply(page);
  await expect(panel(page).getByRole("alert")).toContainText("本次前态已恢复");
  await expect(panel(page).getByRole("radio", { name: "全部静音", exact: true })).toBeChecked();
  await page.evaluate(() => window.__uiFixture.microphoneRestoreFault = "隔离恢复失败，基线保留");
  await restore(page);
  await expect(panel(page).getByRole("alert")).toContainText("基线保留");
  await page.evaluate(() => { window.__uiFixture.microphoneRestoreFault = null; window.__uiFixture.microphoneWriteFault = null; });
  await restore(page);
  await expect(current(page)).toContainText("USB麦克风已静音");
  await expect(panel(page).getByRole("alert")).toHaveCount(0);
});

test("没有活动设备及原厂控制者冲突时不开放写入", async ({ page }) => {
  await expect(current(page)).toContainText("内置麦克风");
  await page.evaluate(() => { const p = window.__uiFixture.microphones; p.current.endpoints = []; p.revision = "empty-mic"; });
  await panel(page).getByRole("button", { name: "重新读取麦克风", exact: true }).click();
  await expect(current(page)).toContainText("没有活动麦克风");
  await expect(panel(page).getByRole("radio", { name: "全部静音", exact: true })).toBeDisabled();
  await page.evaluate(() => { const data = window.__uiFixture; data.microphones.current.endpoints = [{ id: "new", name: null, muted: true }]; data.microphoneBlockedReason = "原厂控制者仍在运行"; data.microphones.revision = "owner-mic"; });
  await panel(page).getByRole("button", { name: "重新读取麦克风", exact: true }).click();
  await expect(current(page)).toContainText("麦克风 1");
  await expect(panel(page)).toContainText("原厂控制者仍在运行");
  await expect(availableAction(page, "麦克风")).toHaveCount(0);
  expect(await writes(page)).toHaveLength(0);
});

test("设备替换保留恢复记录并拒绝把旧基线应用到新设备", async ({ page }) => {
  await panel(page).getByRole("radio", { name: "全部静音", exact: true }).check(); await apply(page);
  await page.evaluate(() => { const p = window.__uiFixture.microphones; p.current.endpoints[1].id = "replacement"; p.revision = "replaced-mic"; });
  await panel(page).getByRole("button", { name: "重新读取麦克风", exact: true }).click();
  await expect(panel(page)).toContainText("不能恢复到其他设备");
  await expect(availableAction(page, "麦克风", true)).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.microphones.initial)).not.toBeNull();
});

test("麦克风卡片左右分组，深浅宽窄布局保留模块边界", async ({ page }) => {
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [1440, 760, 360]) {
      await page.setViewportSize({ width, height: 1200 }); await panel(page).scrollIntoViewIfNeeded();
      await expect(current(page)).toHaveAttribute("data-surface", "inset");
      expect(await panel(page).evaluate(element => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      await panel(page).screenshot({ path: `target/ui-check/microphone-${theme}-${width}.png` });
    }
  }
});

test("麦克风热键需独立确认，保存授权不立即修改端点", async ({ page }) => {
  const hotkeys = page.getByRole("region", { name: "原厂热键与屏显", exact: true });
  await hotkeys.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await hotkeys.getByRole("checkbox", { name: "麦克风静音", exact: true }).check();
  await expect(hotkeys.getByLabel("热键策略预览")).toContainText("麦克风静音");
  await openConfigurationReview(page, "原厂热键与屏显");
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  const calls = await page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "apply_hotkeys"));
  expect(calls[0].args.request.settings.actions).toEqual(["microphoneMute"]);
  expect(await writes(page)).toHaveLength(0);
  await expect(current(page)).toContainText("内置麦克风未静音");
});

test("读取失败撤销执行快照，恢复读取后保留明确目标", async ({ page }) => {
  await panel(page).getByRole("radio", { name: "全部静音", exact: true }).check();
  await page.evaluate(() => window.__uiFixture.microphoneReadFault = "隔离麦克风读取失败");
  await panel(page).getByRole("button", { name: "重新读取麦克风", exact: true }).click();
  await expect(panel(page).getByRole("alert")).toContainText("隔离麦克风读取失败");
  await expect(availableAction(page, "麦克风")).toHaveCount(0);
  await page.evaluate(() => window.__uiFixture.microphoneReadFault = null);
  await panel(page).getByRole("button", { name: "重新读取麦克风", exact: true }).click();
  await expect(panel(page).getByRole("radio", { name: "全部静音", exact: true })).toBeChecked();
  expect(await writes(page)).toHaveLength(0);
});

test("新增设备不自动接管，恢复只处理初始设备并保留新设备状态", async ({ page }) => {
  await panel(page).getByRole("radio", { name: "全部静音", exact: true }).check(); await apply(page);
  await page.evaluate(() => { const p = window.__uiFixture.microphones; p.current.endpoints.push({ id: "new-mic", name: "新麦克风", muted: true }); p.revision = "added-mic"; });
  await panel(page).getByRole("button", { name: "重新读取麦克风", exact: true }).click();
  await expect(panel(page).getByRole("radio", { name: "取消静音", exact: true })).toBeDisabled();
  await expect(panel(page)).toContainText("可先恢复初始设备");
  await expect(availableAction(page, "麦克风", true)).toBeEnabled();
  await restore(page);
  expect(await page.evaluate(() => window.__uiFixture.microphones.current.endpoints.map(endpoint => endpoint.muted))).toEqual([false, true, true]);
  await expect(panel(page).getByRole("radio", { name: "取消静音", exact: true })).toBeEnabled();
});
