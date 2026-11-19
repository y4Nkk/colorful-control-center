import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { restoreDevice } from "./device-settings.mjs";
import { openWorkspace, openSystemArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const display = page => page.getByRole("region", { name: "屏幕显示", exact: true });
const hotkeys = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
async function request(page, id, kind = "displayRotation") {
  await page.evaluate(({ id, kind }) => {
    const status = window.__uiFixture.hotkeys.status;
    status.running = true; status.navigationRequest = { id, kind, ready: true };
    window.__uiEmit("hotkeys-state", status);
  }, { id, kind });
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/"); await openWorkspace(page, "设备配置");
  await expect(display(page)).toBeVisible();
});

test("旋转独立授权，热键定位主屏后由开关直接试用和恢复", async ({ page }) => {
  await openWorkspace(page, "键盘与灯光");
  await hotkeys(page).getByRole("button", { name: "配置热键", exact: true }).click();
  const editor = page.getByRole("dialog", { name: "配置热键与屏显", exact: true });
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await editor.getByRole("checkbox", { name: "原厂屏幕旋转", exact: true }).check();
  await editor.getByRole("button", { name: "完成编辑", exact: true }).click();
  await previewConfiguration(page, "原厂热键与屏显");
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
  await configurationView(page).getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  expect((await calls(page, "apply_hotkeys"))[0].args.request.settings.actions).toEqual(["displayRotation"]);
  await openSystemArea(page, "配置档案");
  await request(page, 30);
  await expect(page.getByRole("region", { name: "显示控制区域", exact: true })).toBeFocused();
  await expect.poll(async () => (await calls(page, "acknowledge_hotkey_navigation")).length).toBe(1);
  const rotate = display(page).getByRole("switch", { name: "翻转主屏", exact: true });
  await expect(rotate).toHaveAccessibleDescription("切换至横向（翻转）");
  expect(await calls(page, "apply_display_rotation")).toEqual([]);
  await rotate.click();
  await expect(display(page).getByRole("status")).toContainText("试用 横向（翻转）");
  await display(page).getByRole("button", { name: "保留屏幕方向", exact: true }).click();
  await expect(rotate).toBeChecked();
  await rotate.click();
  await expect(display(page).getByRole("status")).toContainText("试用 横向 ·");
  await display(page).getByRole("button", { name: "取消本次试用", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__uiFixture.display.displays[0].current.orientation)).toBe(2);
  await restoreDevice(display(page));
  await expect.poll(() => page.evaluate(() => window.__uiFixture.display.displays[0].current.orientation)).toBe(0);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("竖屏预览交换宽高，超时恢复", async ({ page }) => {
  await page.evaluate(() => Object.assign(window.__uiFixture.display.displays[0].current, { orientation: 1, width: 1080, height: 1920 }));
  await expect(display(page).getByRole("img", { name: /1080 × 1920/ })).toBeVisible();
  const rotate = display(page).getByRole("switch", { name: "翻转主屏", exact: true });
  await expect(rotate).toHaveAccessibleDescription("切换至横向（翻转） · 1920 × 1080");
  await rotate.click();
  await expect(display(page).getByRole("img", { name: /1920 × 1080/ })).toBeVisible();
  await page.evaluate(() => { window.__uiFixture.displayExpiresAt = Date.now() - 1; });
  await expect(display(page).getByRole("status")).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => window.__uiFixture.display.displays[0].current.orientation)).toBe(1);
  expect(await page.evaluate(() => [window.__uiFixture.display.displays[0].current.width, window.__uiFixture.display.displays[0].current.height])).toEqual([1080, 1920]);
});

test("新的旋转导航不会被旧键盘导航确认清除", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.navigationAckDelay = 700; });
  await request(page, 40, "keyboard");
  await expect.poll(async () => (await calls(page, "acknowledge_hotkey_navigation")).length).toBe(1);
  await request(page, 41);
  await expect(page.getByRole("region", { name: "显示控制区域", exact: true })).toBeFocused();
  await expect.poll(async () => (await calls(page, "acknowledge_hotkey_navigation")).length).toBe(2);
  await expect.poll(() => page.evaluate(() => window.__uiFixture.hotkeys.status.navigationRequest)).toBeNull();
  expect((await calls(page, "acknowledge_hotkey_navigation")).map(call => call.args.expectedId)).toEqual([40, 41]);
});

test("屏幕卡在宽窄及深浅主题下保持内容边界", async ({ page }) => {
  for (const theme of ["light", "dark"]) {
    await page.evaluate(value => { document.documentElement.dataset.theme = value; }, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 900 });
      await display(page).scrollIntoViewIfNeeded();
      const bounds = await display(page).evaluate(element => ({
        width: element.clientWidth, scroll: element.scrollWidth,
        border: Number.parseFloat(getComputedStyle(element).borderTopWidth),
      }));
      expect(bounds.scroll).toBeLessThanOrEqual(bounds.width + 1);
      expect(bounds.border).toBeGreaterThan(0);
      if (width !== 760) await display(page).screenshot({ path: `target/ui-check/display-rotation-${theme}-${width}.png` });
      await expect(display(page).getByRole("switch", { name: "翻转主屏", exact: true })).toHaveAccessibleDescription("切换至横向（翻转）");
    }
  }
});
