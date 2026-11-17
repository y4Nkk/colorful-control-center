import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { readFile } from "node:fs/promises";
const navigate = (page, title) => page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: title, exact: true }).click();
const preview = page => page.getByRole("region", { name: "档案预览", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply_|save_|import_|set_startup)/.test(c.command)));
async function selectProfile(page) {
  await navigate(page, "性能档案");
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const current = { kind: "x58", settings: { brightness: 1, speed: 3, effect: "scan", colorId: 6 } };
    window.__uiFixture.lightBar = { kind: "x58", revision: "isolated-lightbar-0", current, presets: [current], initial: null, blockedReason: null, warnings: [] };
    window.__uiFixture.profile.settings.lightBar = structuredClone(current);
  });
  await page.goto("/");
});

test("型号灯条预览与确认携带完整档案，浏览不触发写入", async ({ page }) => {
  await selectProfile(page);
  const panel = preview(page);
  await expect(panel.getByRole("region", { name: "档案独立灯条" })).toContainText("X58 · 左右扫描 · 亮度 1 级 · 速度 3 档 · 蓝色");
  expect(await writes(page)).toEqual([]);
  await panel.getByRole("button", { name: "应用", exact: true }).click();
  expect(await writes(page)).toEqual([]);
  await panel.getByRole("button", { name: "应用「隔离档案」？", exact: true }).click();
  await panel.getByText(/^执行明细/).click();
  await expect(panel.getByLabel("执行结果").getByRole("listitem").filter({ hasText: "独立灯条" }).getByRole("img", { name: "成功", exact: true })).toBeVisible();
  const calls = await writes(page);
  expect(calls).toHaveLength(1);
  expect(calls[0].args.expected.settings.lightBar).toEqual({ kind: "x58", settings: { brightness: 1, speed: 3, effect: "scan", colorId: 6 } });
  expect(calls[0].args.expectedMode).toBeNull();
});

test("保存当前捕获完整灯条配置并回到同一图形预览", async ({ page }) => {
  await navigate(page, "性能档案");
  await page.getByPlaceholder("游戏 / 静音").fill("灯条档案");
  await page.getByRole("button", { name: "保存当前", exact: true }).click();
  expect(await writes(page)).toEqual([]);
  await page.getByRole("button", { name: "保存当前设置？", exact: true }).click();
  await expect(preview(page).getByRole("heading", { name: "灯条档案" })).toBeVisible();
  await expect(preview(page).getByLabel("档案独立灯条")).toContainText("X58");
  const calls = await writes(page);
  expect(calls).toHaveLength(1);
  expect(calls[0].command).toBe("save_profile");
  expect(calls[0].args.args.settings.lightBar.settings.colorId).toBe(6);
});

test("导入X170先显示模式颜色，确认导入仍不应用设备", async ({ page }) => {
  await navigate(page, "性能档案");
  const file = await page.evaluate(() => ({ ...structuredClone(window.__uiFixture.profile), name: "导入灯条",
    settings: { ...window.__uiFixture.profile.settings, lightBar: { kind: "x170", settings: {
      effect: "breath", enabled: true, brightness: 4, speed: 0, color: { r: 18, g: 52, b: 86 } } } } }));
  await page.locator('input[type="file"]').setInputFiles({ name: "灯条.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(file)) });
  await expect(preview(page).getByLabel("档案独立灯条")).toContainText("X170 · 开启 · 呼吸 · 亮度 4 级 · 速度 1 档 · #123456");
  expect(await writes(page)).toEqual([]);
  await preview(page).getByRole("button", { name: "确认导入", exact: true }).click();
  await preview(page).getByRole("button", { name: "导入并覆盖同名档案？", exact: true }).click();
  await expect(preview(page).getByRole("button", { name: "应用", exact: true })).toBeVisible();
  expect((await writes(page)).map(c => c.command)).toEqual(["import_profile"]);
});

test("导出保留型号与全部灯条设置", async ({ page }) => {
  await selectProfile(page);
  const download = page.waitForEvent("download");
  await preview(page).getByRole("button", { name: "导出", exact: true }).click();
  const file = await download;
  const exported = JSON.parse(await readFile(await file.path(), "utf8"));
  expect(exported.settings.lightBar).toEqual(await page.evaluate(() => window.__uiFixture.profile.settings.lightBar));
  expect(await writes(page)).toEqual([]);
});

test("确认期间同名文件变化拒绝旧灯条目标，重新选择撤销确认", async ({ page }) => {
  await selectProfile(page);
  const panel = preview(page);
  await panel.getByRole("button", { name: "应用", exact: true }).click();
  await page.evaluate(() => { window.__uiFixture.profile.settings.lightBar.settings.brightness = 4; });
  await panel.getByRole("button", { name: "应用「隔离档案」？", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("档案已变化");
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await expect(panel.getByLabel("档案独立灯条")).toContainText("亮度 4 级");
  await expect(panel.getByRole("button", { name: "应用", exact: true })).toBeVisible();
  expect(await writes(page)).toHaveLength(1);
});

test("组合失败及恢复失败逐步显示，不报告档案全部成功", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.profileSteps = [
    { step: "独立灯条", ok: true, message: "已确认" }, { step: "GPU 偏移", ok: false, message: "隔离GPU失败" },
    { step: "恢复独立灯条", ok: false, message: "隔离恢复失败，初始快照保留" }]; });
  await selectProfile(page);
  await preview(page).getByRole("button", { name: "应用", exact: true }).click();
  await preview(page).getByRole("button", { name: "应用「隔离档案」？", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("部分设置恢复失败");
  await expect(preview(page).getByLabel("执行结果")).toContainText("隔离恢复失败，初始快照保留");
});

test("启动选择复用完整参数预览，保存只改变启动意图", async ({ page }) => {
  await openWorkspace(page, "自动策略");
  const panel = page.getByRole("region", { name: "启动档案", exact: true });
  await panel.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  await panel.getByText("查看启动档案参数", { exact: true }).click();
  await expect(panel.getByLabel("档案独立灯条")).toContainText("X58 · 左右扫描");
  expect(await writes(page)).toEqual([]);
  await panel.getByRole("button", { name: "保存", exact: true }).click();
  await panel.getByRole("button", { name: "启动时应用「隔离档案」？", exact: true }).click();
  expect((await writes(page)).map(c => c.command)).toEqual(["set_startup_profile"]);
  expect(await page.evaluate(() => window.__uiFixture.startupSelection?.profile)).toBe("隔离档案");
  expect((await writes(page))[0].args.expected.settings.lightBar.kind).toBe("x58");
});

test("设备档案卡片浅深色及窄宽布局保持边框间隔与可读性", async ({ page }) => {
  await selectProfile(page);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 1200 });
      const panel = preview(page).getByRole("region", { name: "档案独立灯条" });
      await panel.scrollIntoViewIfNeeded();
      expect(await panel.evaluate(el => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
      expect(await panel.getAttribute("data-surface")).toBe("group");
      await panel.screenshot({ path: `target/ui-check/profile-devices-${theme}-${width}.png` });
    }
  }
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
