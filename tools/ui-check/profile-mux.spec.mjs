import { openProfileResults, openProfileSave, openStartupParameters, saveProfileButton, showProfileMenu, showProfileSettings } from "./profile-review.mjs";
import { changeButton, configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openWorkspace, openPowerArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "档案预览", exact: true });
const muxPanel = page => panel(page).getByLabel("档案显卡模式", { exact: true });
async function select(page) {
  await openPowerArea(page, "配置档案");
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await showProfileSettings(page);
}
async function apply(page) {
  await previewConfiguration(page);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const data = window.__uiFixture;
    data.profile.settings.graphicsMode = { owner: "a".repeat(64), availableMask: 15, mode: 2 };
  });
  await page.goto("/"); await openWorkspace(page, "设备配置");
});

test("完整来源预览，一次应用只发送显卡选择", async ({ page }) => {
  await select(page);
  const mux = muxPanel(page);
  await expect(mux).toContainText("混合");
  await expect(mux).toContainText("需重启生效 · 单独确认重启");
  await expect(mux.getByRole("meter")).toHaveCount(0);
  await expect(mux).toHaveCount(1);
  await expect(panel(page).getByRole("button", { name: "详情", exact: true })).toHaveCount(0);
  await expect(panel(page).locator("time")).toHaveCount(0);
  await expect(page.getByRole("region", { name: "档案列表", exact: true })).toContainText("显示与显卡");
  await previewConfiguration(page);
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(c => c.command === "apply_profile"))).toEqual([]);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  const results = await openProfileResults(page);
  await expect(results).toContainText("显卡模式选择请求（重启后核对）");
  const state = await page.evaluate(() => ({ mode: window.__uiFixture.graphicsMode, reboots: window.__uiFixture.graphicsReboots,
    args: window.__uiFixture.calls.find(c => c.command === "apply_profile").args }));
  expect(state.mode.current.mode).toBe(3);
  expect(state.mode.lastRequest.target).toBe(2);
  expect(state.mode.lastRequest.verification).toBe("pendingReboot");
  expect(state.reboots).toBe(0);
  expect(state.args.expectedGraphicsMode.before.revision).toBe("graphics-0");
});

test("旧选择修订阻止执行", async ({ page }) => {
  await select(page);
  await page.evaluate(() => window.__uiFixture.graphicsMode.revision = "foreign");
  await apply(page);
  await expect(page.getByRole("alert").first()).toContainText("显卡模式预览已变化");
  expect(await page.evaluate(() => window.__uiFixture.graphicsMode.lastRequest)).toBeNull();
});

test("预览失败保留文件目标并禁用执行", async ({ page }) => {
  await page.evaluate(() => window.__uiFixture.profileMuxPreviewFault = "隔离MUX不可用");
  await select(page);
  await expect(muxPanel(page)).toContainText("未读取");
  await expect(muxPanel(page)).toHaveCount(1);
  await expect(changeButton(page)).toHaveCount(0);
  await expect((await showProfileMenu(page)).getByRole("button", { name: "导出档案", exact: true })).toBeEnabled();
});

test("整组显示试用保留后才发送MUX", async ({ page }) => {
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.profile.settings.display = { displays: data.display.displays.map(d => ({ ...d.current, frequency: 144 })) };
  });
  await select(page); await apply(page);
  const host = page.getByRole("region", { name: "档案显示确认", exact: true });
  await expect(host).toBeVisible();
  expect(await page.evaluate(() => window.__uiFixture.graphicsMode.lastRequest)).toBeNull();
  await host.getByRole("button", { name: "保留显示并继续档案", exact: true }).click();
  await expect(host).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.graphicsMode.lastRequest.target)).toBe(2);
});

test("后续失败恢复已知选择", async ({ page }) => {
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.graphicsMode.lastRequest = { target: 4, verification: "pendingReboot" };
    data.profileMuxAfterSendFault = "隔离接受记录保存失败";
  });
  await select(page);
  await expect(muxPanel(page)).toContainText("动态");
  await apply(page);
  const results = await openProfileResults(page);
  await expect(results).toContainText("恢复本次显卡选择");
  expect(await page.evaluate(() => window.__uiFixture.graphicsMode.lastRequest.target)).toBe(4);
});

test("启动选择复用完整MUX预览且保存不执行硬件", async ({ page }) => {
  await openPowerArea(page, "电源设置");
  const startup = page.getByRole("region", { name: "启动档案", exact: true });
  await startup.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  const parameters = await openStartupParameters(page);
  await expect(parameters.getByLabel("档案显卡模式")).toContainText("需重启生效 · 单独确认重启");
  await page.keyboard.press("Escape");
  await configurationView(page).getByRole("button", { name: "授权启动应用档案", exact: true }).click();
  expect(await page.evaluate(() => window.__uiFixture.graphicsMode.lastRequest)).toBeNull();
});


test("捕获保留当前已接受选择，不复制旧档案目标", async ({ page }) => {
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.graphicsMode.lastRequest = { target: 4, verification: "pendingReboot" };
  });
  await select(page);
  await openProfileSave(page, "当前MUX", ["显示与显卡"]);
  await saveProfileButton(page).click();
  await expect(panel(page).getByRole("heading", { name: "当前MUX", exact: true })).toBeVisible();
  const saved = await page.evaluate(() => window.__uiFixture.calls.find(c => c.command === "save_profile"));
  expect(saved.args.args.settings.graphicsMode.mode).toBe(4);
  expect(await page.evaluate(() => window.__uiFixture.graphicsReboots)).toBe(0);
});

for (const [theme, width] of [["light", 1440], ["dark", 360]]) {
  test(`MUX模块布局 ${theme} ${width}`, async ({ page }) => {
    await page.setViewportSize({ width, height: 1000 });
    await page.emulateMedia({ colorScheme: theme });
    await select(page);
    await page.evaluate(value => document.documentElement.dataset.theme = value, theme);
    const mux = muxPanel(page);
    await expect(mux).toBeVisible();
    expect(await mux.evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
    await mux.screenshot({ path: `target/ui-check/profile-mux-${theme}-${width}.png` });
  });
}
