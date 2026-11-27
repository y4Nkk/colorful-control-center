import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { setPickerColor } from "./color-picker.mjs";
import { openWorkspace } from "./workspace.mjs";

const title = "键盘与灯光";
const workspace = page => page.getByRole("main", { name: title, exact: true });
const loading = page => workspace(page).getByText(`正在加载${title}…`, { exact: true });

async function holdRead(page, command) {
  await page.evaluate(command => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__releaseKeyboardRead = null;
    window.__TAURI_INTERNALS__.invoke = async (name, args) => {
      if (name === command && !window.__releaseKeyboardRead) {
        const failure = await new Promise(resolve => { window.__releaseKeyboardRead = resolve; });
        if (failure) throw new Error(failure);
      }
      return invoke(name, args);
    };
  }, command);
}

const waitForRead = page => page.waitForFunction(() => !!window.__releaseKeyboardRead);
const releaseRead = (page, failure = null) => page.evaluate(failure => window.__releaseKeyboardRead(failure), failure);

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await expect(page.getByRole("main", { name: "概览", exact: true })).toBeVisible();
});

test("首次切入时分包和设备读取共用加载状态，未支持的灯光卡片不闪现", async ({ page }) => {
  const chunk = Promise.withResolvers();
  await page.route(/\/(?:src\/app\/KeyboardWorkspace\.tsx|assets\/KeyboardWorkspace-[\w-]+\.js)(?:\?.*)?$/, async route => {
    await chunk.promise;
    await route.continue();
  });
  await holdRead(page, "read_light_bar");
  await openWorkspace(page, title);
  try {
    await expect(loading(page)).toBeVisible();
    const bounds = await loading(page).boundingBox();
    chunk.resolve();
    await waitForRead(page);
    await expect(workspace(page).locator(".ccc-panel:visible")).toHaveCount(0);
    await expect(loading(page)).toBeVisible();
    expect(await loading(page).boundingBox()).toEqual(bounds);
  } finally {
    chunk.resolve();
  }

  await releaseRead(page);
  await expect(workspace(page).getByRole("img", { name: "全键盘背光示意", exact: true })).toBeVisible();
  await expect(workspace(page).getByRole("region", { name: "原厂热键与屏显", exact: true })).toBeVisible();
  await expect(workspace(page).getByRole("region", { name: "独立灯条", exact: true })).toHaveCount(0);
  await expect(workspace(page).getByRole("region", { name: "多区键盘灯", exact: true })).toHaveCount(0);
  await expect(loading(page)).toHaveCount(0);
});

test("逐键灯光的首读失败后呈现页面和重读入口", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.family = "perkey"; });
  await holdRead(page, "read_perkey_lighting");
  await openWorkspace(page, title);
  await waitForRead(page);
  const perkey = workspace(page).getByRole("region", { name: "逐键 RGB", exact: true });
  await expect(loading(page)).toBeVisible();
  await expect(perkey).toHaveCount(0);

  await releaseRead(page, "隔离逐键读取失败");
  await expect(perkey.getByRole("alert")).toContainText("隔离逐键读取失败");
  await expect(workspace(page).getByRole("region", { name: "灯光策略与外围灯区", exact: true })).toBeVisible();
  await expect(loading(page)).toHaveCount(0);
  await perkey.getByRole("button", { name: "重新读取逐键灯", exact: true }).click();
  await expect(perkey.getByRole("slider", { name: "逐键亮度", exact: true })).toBeVisible();
  await expect(perkey.getByRole("alert")).toHaveCount(0);
});

test("切回时直接恢复灯区草稿和滚动位置，读回完成后原位更新", async ({ page }) => {
  await openWorkspace(page, title);
  const panel = workspace(page).getByRole("region", { name: "键盘灯光", exact: true });
  const selectedZone = panel.locator('[data-keyboard-zone="2"]');
  await panel.getByRole("tab", { name: "右区", exact: true }).click();
  await setPickerColor(panel, "右区颜色", "#123456");
  const viewport = workspace(page).locator(".ccc-scroll-viewport");
  await viewport.evaluate(async element => {
    element.scrollTop = 120;
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  });
  await openWorkspace(page, "概览");

  await holdRead(page, "read_keyboard_lighting");
  await page.evaluate(() => { window.__uiFixture.keyboard.current.brightnessLevel = 2; });
  await openWorkspace(page, title);
  await waitForRead(page);
  await expect(loading(page)).toHaveCount(0);
  await expect(selectedZone).toHaveAttribute("data-selected", "true");
  await expect(selectedZone).toHaveAttribute("data-zone-color", "#123456");
  await expect(selectedZone).toHaveAttribute("data-zone-brightness", "3");
  expect(await viewport.evaluate(element => element.scrollTop)).toBe(120);

  await releaseRead(page);
  await expect(selectedZone).toHaveAttribute("data-zone-brightness", "2");
  await expect(selectedZone).toHaveAttribute("data-zone-color", "#123456");
  expect(await viewport.evaluate(element => element.scrollTop)).toBe(120);
});
