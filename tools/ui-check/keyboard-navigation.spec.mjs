import { openSystemArea } from "./workspace.mjs";
import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const editor = page => page.getByRole("dialog", { name: "配置热键与屏显", exact: true });
const scope = page => editor(page).getByRole("checkbox", { name: "打开键盘灯区域", exact: true });
const edit = page => panel(page).getByRole("button", { name: "配置热键", exact: true }).click();
const close = page => editor(page).getByRole("button", { name: "完成编辑", exact: true }).click();
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
const nav = page => page.getByRole("navigation", { name: "主导航" });
async function request(page, id, ready = true) {
  await page.evaluate(({ id, ready }) => {
    const status = window.__uiFixture.hotkeys.status;
    status.running = true; status.navigationRequest = { id, ready, kind: "keyboard" }; window.__uiEmit("hotkeys-state", status);
  }, { id, ready });
}
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await nav(page).getByRole("button", { name: "键盘与灯光", exact: true }).click(); await expect(panel(page)).toBeVisible(); });

test("键盘灯入口独立授权且确认前不应用", async ({ page }) => {
  await edit(page);
  await editor(page).getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await scope(page).check();
  await close(page);
  await previewConfiguration(page, "原厂热键与屏显");
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
  await configurationView(page).getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  expect((await calls(page, "apply_hotkeys"))[0].args.request.settings.actions).toEqual(["openKeyboard"]);
});

test("从其他页面切回键盘区域，保留已有草稿并确认导航", async ({ page }) => {
  await edit(page);
  await scope(page).check();
  await close(page);
  await openSystemArea(page, "配置档案");
  await expect(nav(page).getByRole("button", { name: "系统", exact: true })).toHaveAttribute("aria-current", "page");
  await request(page, 3);
  await expect(nav(page).getByRole("button", { name: "键盘与灯光", exact: true })).toHaveAttribute("aria-current", "page");
  await expect(page.getByRole("region", { name: "键盘灯光区域", exact: true })).toBeFocused();
  await edit(page);
  await expect(scope(page)).toBeChecked();
  await expect.poll(async () => (await calls(page, "acknowledge_hotkey_navigation")).length).toBe(1);
  expect((await calls(page, "acknowledge_hotkey_navigation"))[0].args).toEqual({ expectedId: 3 });
  expect(await page.evaluate(() => window.__uiFixture.hotkeys.status.navigationRequest)).toBeNull();
});

test("同页编辑时保留热键导航请求，弹窗关闭后聚焦灯光区域", async ({ page }) => {
  await edit(page);
  await scope(page).check();
  await page.evaluate(() => localStorage.removeItem("ccc-last-page"));
  await request(page, 4);
  await expect.poll(() => page.evaluate(() => localStorage.getItem("ccc-last-page"))).toBe("keyboard");
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  await expect(editor(page)).toBeVisible();
  expect(await calls(page, "acknowledge_hotkey_navigation")).toEqual([]);
  await close(page);
  await expect(page.getByRole("region", { name: "键盘灯光区域", exact: true })).toBeFocused();
  await expect.poll(async () => (await calls(page, "acknowledge_hotkey_navigation")).map(call => call.args.expectedId)).toEqual([4]);
});

test("窗口未就绪时保留请求，完成后才导航", async ({ page }) => {
  await openSystemArea(page, "配置档案");
  await request(page, 7, false);
  await expect(nav(page).getByRole("button", { name: "系统", exact: true })).toHaveAttribute("aria-current", "page");
  expect(await calls(page, "acknowledge_hotkey_navigation")).toEqual([]);
  await request(page, 7, true);
  await expect(page.getByRole("region", { name: "键盘灯光区域", exact: true })).toBeFocused();
  await expect.poll(async () => (await calls(page, "acknowledge_hotkey_navigation")).length).toBe(1);
});

test("较早确认不能清除随后到来的请求", async ({ page }) => {
  await openSystemArea(page, "配置档案");
  await page.evaluate(() => { window.__uiFixture.navigationAckDelay = 700; });
  await request(page, 11);
  await expect.poll(async () => (await calls(page, "acknowledge_hotkey_navigation")).length).toBe(1);
  await request(page, 12);
  await expect.poll(async () => (await calls(page, "acknowledge_hotkey_navigation")).length).toBe(2);
  await expect.poll(() => page.evaluate(() => window.__uiFixture.hotkeys.status.navigationRequest)).toBeNull();
  expect((await calls(page, "acknowledge_hotkey_navigation")).map(call => call.args.expectedId)).toEqual([11, 12]);
});

test("确认失败保留请求，重新进入键盘与灯光后可以重试", async ({ page }) => {
  await openSystemArea(page, "配置档案");
  await page.evaluate(() => { window.__uiFixture.navigationAckFault = "导航确认失败"; });
  await request(page, 15);
  await expect(page.getByText("导航确认失败", { exact: true }).first()).toBeVisible();
  expect(await page.evaluate(() => window.__uiFixture.hotkeys.status.navigationRequest.id)).toBe(15);
  await openSystemArea(page, "配置档案");
  await nav(page).getByRole("button", { name: "键盘与灯光", exact: true }).click();
  await expect.poll(async () => (await calls(page, "acknowledge_hotkey_navigation")).length).toBe(2);
  expect(await page.evaluate(() => window.__uiFixture.hotkeys.status.navigationRequest)).toBeNull();
});

test("界面尚未订阅时的待处理请求可由首次读取恢复", async ({ browser }) => {
  const context = await browser.newContext(); const page = await context.newPage();
  await page.addInitScript({ content: `${MOCK_IPC}; window.__TAURI_INTERNALS__.invoke("read_hotkeys", {}); window.__uiFixture.hotkeys.status.navigationRequest = { id: 16, ready: true, kind: "keyboard" };` });
  await page.goto("/");
  await expect(page.getByRole("region", { name: "键盘灯光区域", exact: true })).toBeFocused();
  await expect.poll(async () => (await calls(page, "acknowledge_hotkey_navigation")).length).toBe(1);
  expect((await calls(page, "acknowledge_hotkey_navigation"))[0].args.expectedId).toBe(16);
  await context.close();
});

test("订阅后新请求不被较早的状态读取覆盖", async ({ browser }) => {
  const context = await browser.newContext(); const page = await context.newPage();
  await page.addInitScript({ content: `${MOCK_IPC}; window.__uiFixture.coldWarningStatusDelay = 1500;` });
  await page.goto("/"); await nav(page).getByRole("button", { name: "键盘与灯光", exact: true }).click(); await expect(panel(page)).toBeVisible();
  await openSystemArea(page, "配置档案");
  await request(page, 17);
  await expect(page.getByRole("region", { name: "键盘灯光区域", exact: true })).toBeFocused();
  await expect.poll(async () => (await calls(page, "acknowledge_hotkey_navigation")).length).toBe(1);
  await page.waitForTimeout(1600);
  expect(await calls(page, "acknowledge_hotkey_navigation")).toHaveLength(1);
  await context.close();
});
