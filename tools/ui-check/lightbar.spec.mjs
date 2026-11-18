import { availableAction, configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const region = page => page.getByRole("region", { name: "独立灯条", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply_light_bar|restore_light_bar)$/.test(c.command)));
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const settings = { enabled: true, brightness: 3, speed: 1, effect: "static", color: { r: 255, g: 0, b: 0 } };
    window.__uiFixture.lightBar = { kind: "x170", revision: "lightbar-0", current: { kind: "x170", settings }, initial: null, blockedReason: null, warnings: ["隔离配置，未操作HID"],
      presets: ["static", "breath", "blink", "cycle", "meteor", "wave", "ripple", "random"].map(effect => ({ kind: "x170", settings: { ...settings, effect, color: ["static", "breath", "blink", "meteor"].includes(effect) ? settings.color : null } })) };
  });
  await page.goto("/"); await openWorkspace(page, "灯光效果");
});
test("独立灯条模式颜色与亮度先预览再确认，可恢复初始", async ({ page }) => {
  const panel = region(page);
  await panel.getByRole("combobox", { name: "独立灯条模式", exact: true }).click();
  await expect(page.getByRole("option")).toHaveCount(8);
  await page.getByRole("option", { name: "呼吸", exact: true }).click();
  await panel.getByLabel("独立灯条颜色", { exact: true }).fill("#123456");
  await expect(panel.getByLabel("独立灯条变更预览")).toContainText("#123456");
  await openConfigurationReview(page, "独立灯条"); expect(await writes(page)).toHaveLength(0);
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  const calls = await writes(page); expect(calls).toHaveLength(1); expect(calls[0].args.request.values.kind).toBe("x170"); expect(calls[0].args.request.values.settings.color).toEqual({ r: 18, g: 52, b: 86 });
  await openConfigurationReview(page, "独立灯条", true); expect(await writes(page)).toHaveLength(1);
  await configurationDialog(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(panel.getByLabel("独立灯条颜色", { exact: true })).toHaveValue("#ff0000");
});
test("读取失败撤销执行快照且不丢草稿，外部变化撤销旧确认", async ({ page }) => {
  const panel = region(page);
  await panel.getByLabel("独立灯条颜色", { exact: true }).fill("#abcdef");
  await openConfigurationReview(page, "独立灯条");
  await page.evaluate(() => { window.__uiFixture.lightBar.revision = "external"; });
  await panel.getByRole("button", { name: "重新读取独立灯条", exact: true }).click();
  await expect(availableAction(page, "独立灯条")).toBeVisible();
  await page.evaluate(() => { window.__uiFixture.lightBarReadFault = "隔离灯条读取失败"; });
  await panel.getByRole("button", { name: "重新读取独立灯条", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("隔离灯条读取失败");
  await expect(availableAction(page, "独立灯条")).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.lightBarReadFault = null; });
  await panel.getByRole("button", { name: "重新读取独立灯条", exact: true }).click();
  await expect(panel.getByLabel("独立灯条颜色", { exact: true })).toHaveValue("#abcdef");
  expect(await writes(page)).toHaveLength(0);
});
test("部分失败保留恢复入口，未知当前值仍可恢复已记录初态", async ({ page }) => {
  const panel = region(page);
  await panel.getByLabel("独立灯条颜色", { exact: true }).fill("#123456");
  await page.evaluate(() => { window.__uiFixture.lightBarFault = "隔离发送失败，恢复已尝试"; });
  await openConfigurationReview(page, "独立灯条");
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("隔离发送失败");
  await page.evaluate(() => { window.__uiFixture.lightBar.current = null; window.__uiFixture.lightBar.revision = "unknown"; });
  await panel.getByRole("button", { name: "重新读取独立灯条", exact: true }).click();
  await expect(panel).toContainText("当前保存值未知");
  await expect(availableAction(page, "独立灯条")).toHaveCount(0);
  await openConfigurationReview(page, "独立灯条", true);
  await configurationDialog(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(panel.getByLabel("独立灯条颜色", { exact: true })).toHaveValue("#ff0000");
});
test("设备冲突阻止写入，无能力隐藏且不生成默认配置", async ({ page }) => {
  const panel = region(page);
  await page.evaluate(() => { window.__uiFixture.lightBar.blockedReason = "原厂控制者仍在运行"; });
  await panel.getByRole("button", { name: "重新读取独立灯条", exact: true }).click();
  await panel.getByLabel("独立灯条颜色", { exact: true }).fill("#123456");
  await expect(availableAction(page, "独立灯条")).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.lightBar = null; });
  await panel.getByRole("button", { name: "重新读取独立灯条", exact: true }).click();
  await expect(panel).toHaveCount(0); expect(await writes(page)).toHaveLength(0);
});
test("左右卡片有间隔，窄屏回流，非颜色模式保留草稿说明", async ({ page }) => {
  const panel = region(page);
  await panel.getByRole("combobox", { name: "独立灯条模式", exact: true }).click();
  await page.getByRole("option", { name: "波浪", exact: true }).click();
  await expect(panel.getByLabel("独立灯条颜色", { exact: true })).toHaveCount(0);
  for (const width of [360, 760, 1440]) {
    await page.setViewportSize({ width, height: 1600 }); await panel.scrollIntoViewIfNeeded();
    const boxes = await panel.locator(".ccc-light-bar-section").evaluateAll(nodes => nodes.map(node => { const r = node.getBoundingClientRect(); return { x: r.x, right: r.right, y: r.y, bottom: r.bottom }; }));
    expect(boxes[1].right).toBeLessThanOrEqual(width + 1);
    if (width === 360) expect(boxes[1].y).toBeGreaterThan(boxes[0].bottom);
    if (width === 1440) expect(boxes[1].x).toBeGreaterThan(boxes[0].right);
  }
  await panel.screenshot({ path: "target/ui-check/lightbar.png", animations: "disabled" });
});
