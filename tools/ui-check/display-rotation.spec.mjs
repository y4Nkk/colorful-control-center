import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openWorkspace, openPowerArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const display = page => page.getByRole("region", { name: "屏幕显示", exact: true });
const hotkeys = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const guideButton = page => display(page).getByRole("button", { name: "翻转主屏说明", exact: true });
const guide = page => page.getByRole("dialog", { name: "翻转主屏说明", exact: true });
// 翻转目标只在开关旁的说明弹窗里展示，卡片内不再有说明行。
async function readGuide(page, text) {
  await guideButton(page).click();
  await expect(guide(page)).toContainText(text);
  await page.keyboard.press("Escape");
  await expect(guide(page)).toHaveCount(0);
}
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

test("旋转独立授权，热键定位主屏后由开关直接试用、取消和保留", async ({ page }) => {
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
  await openPowerArea(page, "配置档案");
  await request(page, 30);
  await expect(page.getByRole("region", { name: "显示控制区域", exact: true })).toBeFocused();
  await expect.poll(async () => (await calls(page, "acknowledge_hotkey_navigation")).length).toBe(1);
  const rotate = display(page).getByRole("switch", { name: "翻转主屏", exact: true });
  await expect(display(page).getByText(/^切换至/)).toHaveCount(0);
  await readGuide(page, /主屏当前是 横向 · \d+ × \d+。打开开关后切换为 横向（翻转）。/);
  expect(await calls(page, "apply_display_rotation")).toEqual([]);
  await rotate.click();
  await expect(display(page).getByRole("status")).toContainText("试用 横向（翻转）");
  await expect(display(page).locator(":scope > .ccc-display-rotation:last-child").getByRole("switch")).toBeDisabled();
  await display(page).getByRole("button", { name: "保留屏幕方向", exact: true }).click();
  await expect(rotate).toBeChecked();
  await readGuide(page, "关闭开关后切换为 横向。");
  await rotate.click();
  await expect(display(page).getByRole("status")).toContainText("试用 横向 ·");
  await display(page).getByRole("button", { name: "取消本次试用", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__uiFixture.display.displays[0].current.orientation)).toBe(2);
  await rotate.click();
  await display(page).getByRole("button", { name: "保留屏幕方向", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__uiFixture.display.displays[0].current.orientation)).toBe(0);
  await expect(display(page).getByRole("button", { name: "恢复初始设置", exact: true })).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("竖屏预览交换宽高，超时恢复", async ({ page }) => {
  await page.evaluate(() => Object.assign(window.__uiFixture.display.displays[0].current, { orientation: 1, width: 1080, height: 1920 }));
  await expect(display(page).getByRole("img", { name: /1080 × 1920/ })).toBeVisible();
  const rotate = display(page).getByRole("switch", { name: "翻转主屏", exact: true });
  await readGuide(page, "打开开关后切换为 横向（翻转） · 1920 × 1080。");
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
  const rotate = display(page).getByRole("switch", { name: "翻转主屏", exact: true });
  await expect(guideButton(page)).toBeVisible();
  await expect(display(page).getByText(/^切换至/)).toHaveCount(0);
  await expect(display(page).getByRole("img", { name: /^当前主屏：/ })).toBeVisible();
  await expect(display(page).getByText("当前主屏", { exact: true })).toHaveCount(0);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(value => { document.documentElement.dataset.theme = value; }, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 900 });
      await display(page).scrollIntoViewIfNeeded();
      const bounds = await display(page).evaluate(element => {
        const style = getComputedStyle(element);
        const footer = element.querySelector(".ccc-display-rotation");
        const rotation = footer.getBoundingClientRect();
        const body = element.querySelector(".ccc-display-settings").getBoundingClientRect();
        const visual = element.querySelector(".ccc-display-mode-visual > svg").getBoundingClientRect();
        const controls = element.querySelector(".ccc-display-controls").getBoundingClientRect();
        const label = footer.querySelector(".ccc-display-rotation-label");
        const title = label.querySelector("strong").getBoundingClientRect();
        const info = label.querySelector("button[aria-label='翻转主屏说明']").getBoundingClientRect();
        return {
          width: element.clientWidth, scroll: element.scrollWidth,
          border: Number.parseFloat(style.borderTopWidth),
          inset: element.getBoundingClientRect().bottom - rotation.bottom - Number.parseFloat(style.borderBottomWidth),
          padding: Number.parseFloat(style.paddingBottom),
          footerLast: footer === element.lastElementChild,
          fullWidth: Math.abs(rotation.left - body.left) < .5 && Math.abs(rotation.right - body.right) < .5,
          rightOfVisual: controls.left >= visual.right,
          topOffset: controls.top - visual.top,
          labelRow: info.left >= title.right - .5 && Math.abs((info.top + info.bottom) / 2 - (title.top + title.bottom) / 2) < 1.5,
        };
      });
      expect(bounds.scroll).toBeLessThanOrEqual(bounds.width + 1);
      expect(bounds.border).toBeGreaterThan(0);
      expect(bounds.inset).toBeCloseTo(bounds.padding, 0);
      expect(bounds.footerLast).toBe(true);
      expect(bounds.fullWidth).toBe(true);
      expect(bounds.labelRow).toBe(true);
      if (width === 1440) {
        expect(bounds.rightOfVisual).toBe(true);
        expect(bounds.topOffset).toBeCloseTo(0, 0);
        const refresh = page.getByRole("switch", { name: "启用自动刷新率", exact: true });
        expect((await rotate.boundingBox()).y).toBeCloseTo((await refresh.boundingBox()).y, 0);
      }
      if (width !== 760) await display(page).screenshot({ path: `target/ui-check/display-rotation-${theme}-${width}.png` });
      await expect(guideButton(page)).toBeVisible();
    }
  }
});
