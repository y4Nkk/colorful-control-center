import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const region = page => page.getByRole("region", { name: "独立灯条", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_light_bar|restore_light_bar)$/.test(call.command)));
async function select(page, label, name) {
  await region(page).getByRole("combobox", { name: label, exact: true }).click();
  await page.getByRole("option", { name, exact: true }).click();
}
async function apply(page) {
  await region(page).getByRole("button", { name: "应用独立灯条", exact: true }).click();
  await region(page).getByRole("button", { name: "确认应用这些灯条设置？", exact: true }).click();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const settings = { effect: "static", speed: 3, brightness: 4, colorId: 1 };
    window.__uiFixture.lightBar = { kind: "x58", revision: "lightbar-0", current: { kind: "x58", settings }, initial: null,
      blockedReason: null, warnings: ["隔离X58保存意图，没有打开HID"], presets: ["static", "breath", "wave", "cycle", "scan", "colorWave"].map(effect =>
        ({ kind: "x58", settings: { ...settings, effect, colorId: effect === "static" ? 1 : effect === "scan" ? 6 : null } })) };
  });
  await page.goto("/"); await openWorkspace(page, "灯光效果");
});

test("X58六模式七色先预览再确认，类型化请求与初始恢复完整", async ({ page }) => {
  const panel = region(page);
  await panel.getByRole("combobox", { name: "独立灯条模式", exact: true }).click();
  await expect(page.getByRole("option")).toHaveCount(6); await page.keyboard.press("Escape");
  await panel.getByRole("combobox", { name: "独立灯条七色", exact: true }).click();
  await expect(page.getByRole("option")).toHaveCount(7); await page.getByRole("option", { name: "紫色", exact: true }).click();
  await expect(panel.getByLabel("独立灯条变更预览")).toContainText("紫色");
  await panel.getByRole("button", { name: "应用独立灯条", exact: true }).click(); expect(await writes(page)).toHaveLength(0);
  await panel.getByRole("button", { name: "确认应用这些灯条设置？", exact: true }).click();
  const calls = await writes(page); expect(calls).toHaveLength(1);
  expect(calls[0].args.request.values).toEqual({ kind: "x58", settings: { effect: "static", speed: 3, brightness: 4, colorId: 7 } });
  await panel.getByRole("button", { name: "恢复初始灯条", exact: true }).click(); expect(await writes(page)).toHaveLength(1);
  await panel.getByRole("button", { name: "确认恢复修改前的独立灯条？", exact: true }).click();
  await expect(panel.getByRole("combobox", { name: "独立灯条七色", exact: true })).toHaveText("红色");
});

test("静态与扫描颜色分别记忆，切换不覆盖另一模式的保存颜色", async ({ page }) => {
  await select(page, "独立灯条七色", "紫色"); await apply(page);
  await select(page, "独立灯条模式", "左右扫描");
  await expect(region(page).getByRole("combobox", { name: "独立灯条七色", exact: true })).toHaveText("蓝色");
  await select(page, "独立灯条七色", "黄色"); await apply(page);
  await select(page, "独立灯条模式", "静态");
  await expect(region(page).getByRole("combobox", { name: "独立灯条七色", exact: true })).toHaveText("紫色");
  expect(await writes(page)).toHaveLength(2);
});

test("最低亮度按原厂零档预览，动态速度为1到3且不提供虚构开关", async ({ page }) => {
  const panel = region(page);
  await expect(panel.getByRole("switch")).toHaveCount(0);
  await expect(panel.getByRole("slider", { name: "独立灯条速度", exact: true })).toHaveCount(0);
  await panel.getByRole("slider", { name: "独立灯条亮度", exact: true }).fill("0");
  await select(page, "独立灯条模式", "呼吸");
  const speed = panel.getByRole("slider", { name: "独立灯条速度", exact: true });
  await expect(speed).toHaveAttribute("min", "1"); await expect(speed).toHaveAttribute("max", "3");
  await speed.fill("1"); await apply(page);
  const calls = await writes(page); expect(calls[0].args.request.values.settings).toMatchObject({ brightness: 0, speed: 1, colorId: null });
  expect("enabled" in calls[0].args.request.values.settings).toBe(false);
});

test("未知的非活动颜色须明确选择，不能自动补成默认红色", async ({ page }) => {
  const panel = region(page);
  await expect(panel).toContainText("当前保存配置");
  await page.evaluate(() => { const value = window.__uiFixture.lightBar; value.presets = value.presets.filter(preset => preset.settings.effect !== "scan"); value.revision = "unknown-scan"; });
  await panel.getByRole("button", { name: "重新读取独立灯条", exact: true }).click();
  await select(page, "独立灯条模式", "左右扫描");
  await expect(panel.getByRole("combobox", { name: "独立灯条七色", exact: true })).toHaveText("请选择");
  await expect(panel).toContainText("保存颜色未知，请先选择七色之一");
  await expect(panel.getByRole("button", { name: "应用独立灯条", exact: true })).toBeDisabled(); expect(await writes(page)).toHaveLength(0);
  await select(page, "独立灯条七色", "青色"); await apply(page);
  expect((await writes(page))[0].args.request.values.settings.colorId).toBe(5);
});

test("重读撤销旧确认，读取失败保留草稿并移除执行快照", async ({ page }) => {
  const panel = region(page);
  await select(page, "独立灯条七色", "蓝色");
  await panel.getByRole("button", { name: "应用独立灯条", exact: true }).click();
  await page.evaluate(() => { window.__uiFixture.lightBar.revision = "external"; });
  await panel.getByRole("button", { name: "重新读取独立灯条", exact: true }).click();
  await expect(panel.getByRole("button", { name: "确认应用这些灯条设置？", exact: true })).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.lightBarReadFault = "隔离X58读取失败"; });
  await panel.getByRole("button", { name: "重新读取独立灯条", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("隔离X58读取失败");
  await expect(panel.getByRole("button", { name: "应用独立灯条", exact: true })).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.lightBarReadFault = null; });
  await panel.getByRole("button", { name: "重新读取独立灯条", exact: true }).click();
  await expect(panel.getByRole("combobox", { name: "独立灯条七色", exact: true })).toHaveText("蓝色"); expect(await writes(page)).toHaveLength(0);
});

test("部分失败留下初始恢复，未知现值仍可恢复原X58配置", async ({ page }) => {
  const panel = region(page); await select(page, "独立灯条七色", "黄色");
  await page.evaluate(() => { window.__uiFixture.lightBarFault = "隔离X58部分保存失败，补偿已尝试"; }); await apply(page);
  await expect(panel.getByRole("alert")).toContainText("隔离X58部分保存失败");
  await page.evaluate(() => { window.__uiFixture.lightBar.current = null; window.__uiFixture.lightBar.revision = "unknown"; });
  await panel.getByRole("button", { name: "重新读取独立灯条", exact: true }).click();
  await expect(panel.getByRole("button", { name: "应用独立灯条", exact: true })).toHaveCount(0);
  await panel.getByRole("button", { name: "恢复初始灯条", exact: true }).click();
  await panel.getByRole("button", { name: "确认恢复修改前的独立灯条？", exact: true }).click();
  await expect(panel.getByRole("combobox", { name: "独立灯条七色", exact: true })).toHaveText("红色");
});

test("型号变化保留旧草稿但不跨型号解释，显式载入新型号再编辑", async ({ page }) => {
  const panel = region(page); await select(page, "独立灯条七色", "紫色");
  await page.evaluate(() => {
    const profile = window.__uiFixture.lightBar, settings = { enabled: true, brightness: 2, speed: 1, effect: "static", color: { r: 1, g: 2, b: 3 } };
    profile.kind = "x170"; profile.current = { kind: "x170", settings }; profile.presets = [profile.current]; profile.revision = "changed-kind";
  });
  await panel.getByRole("button", { name: "重新读取独立灯条", exact: true }).click();
  await expect(panel).toContainText("灯条型号已变化，旧草稿已保留");
  await expect(panel.getByRole("button", { name: "应用独立灯条", exact: true })).toHaveCount(0);
  await panel.getByRole("button", { name: "载入当前灯条草稿", exact: true }).click();
  await expect(panel.getByLabel("独立灯条颜色", { exact: true })).toHaveValue("#010203"); expect(await writes(page)).toHaveLength(0);
});

test("六模式灯条在宽窄屏保留左右卡片间隔及模块边框", async ({ page }) => {
  const panel = region(page); await select(page, "独立灯条模式", "左右扫描");
  for (const width of [360, 760, 1440]) {
    await page.setViewportSize({ width, height: 1600 }); await panel.scrollIntoViewIfNeeded();
    const boxes = await panel.locator(".ccc-light-bar-section").evaluateAll(nodes => nodes.map(node => { const box = node.getBoundingClientRect(); return { x: box.x, right: box.right, y: box.y, bottom: box.bottom, border: parseFloat(getComputedStyle(node).borderLeftWidth) }; }));
    expect(boxes.every(box => box.border > 0)).toBe(true);
    if (width === 1440) expect(boxes[1].x - boxes[0].right).toBeGreaterThanOrEqual(12);
    else expect(boxes[1].y - boxes[0].bottom).toBeGreaterThanOrEqual(12);
    expect(await panel.evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true);
  }
  await panel.screenshot({ path: "target/ui-check/x58-lightbar.png", animations: "disabled" });
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
