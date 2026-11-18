import { configurationDialog, confirmConfiguration, openConfigurationReview } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
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
  await page.goto("/"); await openWorkspace(page, "显示与显卡");
  await expect(display(page)).toBeVisible();
});

test("旋转独立授权，热键定位主屏并在确认后试用和恢复", async ({ page }) => {
  await openWorkspace(page, "键盘与输入");
  await hotkeys(page).getByRole("button", { name: "配置热键", exact: true }).click();
  const editor = page.getByRole("dialog", { name: "配置热键与屏显", exact: true });
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await editor.getByRole("checkbox", { name: "原厂屏幕旋转", exact: true }).check();
  await editor.getByRole("button", { name: "完成编辑", exact: true }).click();
  await openConfigurationReview(page, "原厂热键与屏显");
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  expect((await calls(page, "apply_hotkeys"))[0].args.request.settings.actions).toEqual(["displayRotation"]);
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: "性能档案" }).click();
  await request(page, 30);
  await expect(page.getByRole("region", { name: "显示控制区域", exact: true })).toBeFocused();
  await expect.poll(async () => (await calls(page, "acknowledge_hotkey_navigation")).length).toBe(1);
  const review = await openConfigurationReview(page, "屏幕方向");
  await expect(review.getByLabel("显示旋转预览")).toContainText("横向（翻转）");
  expect(await calls(page, "apply_display_rotation")).toEqual([]);
  await confirmConfiguration(review, "试用屏幕方向");
  await expect(display(page).getByRole("status")).toContainText("试用 横向（翻转）");
  await display(page).getByRole("button", { name: "保留屏幕方向", exact: true }).click();
  await display(page).getByRole("switch", { name: "翻转主屏", exact: true }).check();
  await confirmConfiguration(await openConfigurationReview(page, "屏幕方向"), "试用屏幕方向");
  await display(page).getByRole("button", { name: "取消本次试用", exact: true }).click();
  await display(page).getByRole("button", { name: "恢复本次试用前的显示模式", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__uiFixture.display.displays[0].current.orientation)).toBe(2);
  await openConfigurationReview(page, "屏幕显示", true);
  await configurationDialog(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__uiFixture.display.displays[0].current.orientation)).toBe(0);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("竖屏预览交换宽高，超时恢复", async ({ page }) => {
  await page.evaluate(() => Object.assign(window.__uiFixture.display.displays[0].current, { orientation: 1, width: 1080, height: 1920 }));
  await expect(display(page).getByRole("img", { name: /1080 × 1920/ })).toBeVisible();
  await display(page).getByRole("switch", { name: "翻转主屏", exact: true }).check();
  const review = await openConfigurationReview(page, "屏幕方向");
  await expect(review.getByLabel("显示旋转预览")).toContainText("1920 × 1080");
  await confirmConfiguration(review, "试用屏幕方向");
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
      await display(page).getByRole("switch", { name: "翻转主屏", exact: true }).check();
      const review = await openConfigurationReview(page, "屏幕方向");
      const figures = review.getByLabel("显示旋转预览").getByRole("img");
      await expect(figures).toHaveCount(2);
      expect(await review.evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
      await page.keyboard.press("Escape");
    }
  }
});
