import { deviceRestoreButton, openDeviceRestore, restoreDevice } from "./device-settings.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "麦克风静音", exact: true });
const current = page => panel(page).getByLabel("当前麦克风状态", { exact: true });
const mute = page => panel(page).getByRole("radio", { name: "全部静音", exact: true });
const unmute = page => panel(page).getByRole("radio", { name: "取消静音", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply|restore)_microphones$/.test(call.command)));
const reread = page => panel(page).getByRole("button", { name: "重新读取麦克风", exact: true }).click();

async function details(page) {
  await panel(page).getByRole("button", { name: "详情", exact: true }).click();
  return panel(page).getByRole("dialog", { name: "麦克风详情", exact: true });
}
const endpoint = (dialog, name) => dialog.locator(".ccc-fact-row").filter({ hasText: name }).locator("dd");

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await openWorkspace(page, "设备配置");
  await expect(current(page)).toContainText("部分静音");
});

test("直接统一静音，卡内可恢复原始混合状态", async ({ page }) => {
  const initial = await details(page);
  await expect(endpoint(initial, "内置麦克风")).toHaveText("未静音");
  await expect(endpoint(initial, "USB麦克风")).toHaveText("已静音");
  await page.keyboard.press("Escape");
  expect(await writes(page)).toHaveLength(0);
  await mute(page).click();
  await expect(mute(page)).toBeChecked();
  await expect(current(page)).not.toContainText("全部静音");
  expect((await writes(page))[0].args.request.muted).toBe(true);
  const restore = await openDeviceRestore(panel(page));
  await expect(endpoint(restore, "内置麦克风")).toHaveText("未静音");
  await expect(endpoint(restore, "USB麦克风")).toHaveText("已静音");
  await restore.getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(current(page)).toContainText("部分静音");
  await expect(deviceRestoreButton(panel(page))).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.microphones.current.endpoints.map(endpoint => endpoint.muted))).toEqual([false, true]);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("外部修订和热键事件刷新回读，之后的切换携带新修订", async ({ page }) => {
  await page.evaluate(() => {
    const profile = window.__uiFixture.microphones;
    profile.current.endpoints[0].muted = true;
    profile.revision = "external-mic";
  });
  await reread(page);
  await expect(current(page)).toContainText("全部静音");
  expect(await writes(page)).toHaveLength(0);
  await page.evaluate(() => {
    const profile = window.__uiFixture.microphones;
    profile.current.endpoints.forEach(endpoint => endpoint.muted = false);
    profile.revision = "hotkey-mic";
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(unmute(page)).toBeChecked();
  await mute(page).click();
  await expect(mute(page)).toBeChecked();
  expect((await writes(page))[0].args.request).toEqual({ expectedRevision: "hotkey-mic", muted: true });
});

test("写入及恢复失败显示实际状态，重试可恢复首次基线", async ({ page }) => {
  await page.evaluate(() => window.__uiFixture.microphoneWriteFault = "隔离麦克风写入失败");
  await mute(page).click();
  await expect(panel(page).getByRole("alert")).toContainText("本次前态已恢复");
  await expect(mute(page)).not.toBeChecked();
  await page.evaluate(() => window.__uiFixture.microphoneRestoreFault = "隔离恢复失败，基线保留");
  await restoreDevice(panel(page));
  await expect(panel(page).getByRole("alert")).toContainText("基线保留");
  await page.evaluate(() => { window.__uiFixture.microphoneRestoreFault = null; window.__uiFixture.microphoneWriteFault = null; });
  await restoreDevice(panel(page));
  await expect(current(page)).toContainText("部分静音");
  await expect(deviceRestoreButton(panel(page))).toHaveCount(0);
  await expect(panel(page).getByRole("alert")).toHaveCount(0);
});

test("无活动设备或原厂控制者运行时禁用写入", async ({ page }) => {
  await page.evaluate(() => { const profile = window.__uiFixture.microphones; profile.current.endpoints = []; profile.revision = "empty-mic"; });
  await reread(page);
  await expect(current(page)).toContainText("无活动麦克风");
  await expect(mute(page)).toBeDisabled();
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.microphones.current.endpoints = [{ id: "new", name: null, muted: true }];
    data.microphoneBlockedReason = "原厂控制者仍在运行";
    data.microphones.revision = "owner-mic";
  });
  await reread(page);
  await expect(current(page)).toContainText("全部静音");
  const dialog = await details(page);
  await expect(endpoint(dialog, "麦克风 1")).toHaveText("已静音");
  await expect(dialog).toContainText("原厂控制者仍在运行");
  await expect(mute(page)).toBeDisabled();
  expect(await writes(page)).toHaveLength(0);
});

test("设备替换保留恢复记录，禁用对新设备执行初始恢复", async ({ page }) => {
  await mute(page).click();
  await expect(mute(page)).toBeChecked();
  await page.evaluate(() => { const p = window.__uiFixture.microphones; p.current.endpoints[1].id = "replacement"; p.revision = "replaced-mic"; });
  await reread(page);
  await expect(unmute(page)).toBeDisabled();
  await expect(await details(page)).toContainText("不能恢复到其他设备");
  await expect(deviceRestoreButton(panel(page))).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.microphones.initial)).not.toBeNull();
});

test("麦克风卡片在深浅宽窄布局保持模块边界", async ({ page }) => {
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [1440, 760, 360]) {
      await page.setViewportSize({ width, height: 1200 });
      await panel(page).scrollIntoViewIfNeeded();
      await expect(panel(page)).toHaveCSS("border-top-width", "1px");
      expect(await panel(page).evaluate(element => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      await panel(page).screenshot({ path: `target/ui-check/microphone-${theme}-${width}.png` });
    }
  }
});

test("麦克风热键独立保存授权，端点状态保持", async ({ page }) => {
  await openWorkspace(page, "键盘与灯光");
  const hotkeys = page.getByRole("region", { name: "原厂热键与屏显", exact: true });
  await hotkeys.getByRole("button", { name: "配置热键", exact: true }).click();
  const editor = page.getByRole("dialog", { name: "配置热键与屏显", exact: true });
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await editor.getByRole("checkbox", { name: "麦克风静音", exact: true }).check();
  await editor.getByRole("button", { name: "完成编辑", exact: true }).click();
  const changes = page.getByRole("region", { name: "原厂热键与屏显 · 保存热键接管范围", exact: true });
  await expect(changes).toContainText("麦克风静音");
  await changes.getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  const calls = await page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "apply_hotkeys"));
  expect(calls[0].args.request.settings.actions).toEqual(["microphoneMute"]);
  expect(await writes(page)).toHaveLength(0);
  await openWorkspace(page, "设备配置");
  await expect(current(page)).toContainText("部分静音");
});

test("读取失败禁用切换，读取成功后恢复实际值", async ({ page }) => {
  await page.evaluate(() => window.__uiFixture.microphoneReadFault = "隔离麦克风读取失败");
  await reread(page);
  await expect(panel(page).getByRole("alert")).toContainText("隔离麦克风读取失败");
  await expect(mute(page)).toBeDisabled();
  await page.evaluate(() => window.__uiFixture.microphoneReadFault = null);
  await reread(page);
  await expect(mute(page)).toBeEnabled();
  await expect(mute(page)).not.toBeChecked();
  expect(await writes(page)).toHaveLength(0);
});

test("新增设备保持原状态，恢复只处理初始设备", async ({ page }) => {
  await mute(page).click();
  await expect(mute(page)).toBeChecked();
  await page.evaluate(() => {
    const profile = window.__uiFixture.microphones;
    profile.current.endpoints.push({ id: "new-mic", name: "新麦克风", muted: true });
    profile.revision = "added-mic";
  });
  await reread(page);
  await expect(unmute(page)).toBeDisabled();
  await expect(current(page)).toContainText("可先恢复初始设备");
  await restoreDevice(panel(page));
  await expect(unmute(page)).toBeEnabled();
  expect(await page.evaluate(() => window.__uiFixture.microphones.current.endpoints.map(endpoint => endpoint.muted))).toEqual([false, true, true]);
});
