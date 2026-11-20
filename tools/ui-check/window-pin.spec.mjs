import { expect, test } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";

const LABEL = "窗口始终置顶";
const pin = page => page.getByRole("button", { name: LABEL, exact: true });

async function openDesktop(page, savedPin = false) {
  await page.addInitScript({ content: `${MOCK_IPC}
    (() => {
      window.isTauri = true;
      const data = window.__uiFixture;
      data.preferences.alwaysOnTop = ${savedPin};
      const ipc = window.__TAURI_INTERNALS__;
      ipc.metadata = { currentWindow: { label: "main" } };
      const invoke = ipc.invoke;
      ipc.invoke = async (command, args) => {
        if (command === "window_is_maximized") return false;
        if (command === "set_app_preferences") {
          if (data.holdPreferences) await new Promise(resolve => { data.finishPreferences = resolve; });
          if (data.preferencesFault) throw new Error(data.preferencesFault);
        }
        return invoke(command, args);
      };
    })();` });
  await page.goto("/");
}

test("顶栏图钉与设置共用已确认的置顶值和保存锁", async ({ page }) => {
  await openDesktop(page);
  await openWorkspace(page, "设置");
  const toggle = pin(page);
  const setting = page.getByRole("switch", { name: LABEL, exact: true });
  const original = await page.evaluate(() => window.__uiFixture.preferences);
  await expect(toggle).toHaveAttribute("aria-pressed", "false");
  await expect(toggle).toHaveAttribute("title", "置顶窗口");
  await page.evaluate(() => { window.__uiFixture.holdPreferences = true; });

  await toggle.press("Space");
  await page.waitForFunction(() => typeof window.__uiFixture.finishPreferences === "function");
  await expect(toggle).toBeDisabled();
  await expect(setting).toBeDisabled();
  await expect(toggle).toHaveAttribute("aria-pressed", "false");
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.holdPreferences = false;
    data.finishPreferences();
  });

  await expect(toggle).toHaveAttribute("aria-pressed", "true");
  await expect(toggle).toHaveAttribute("title", "取消置顶");
  await expect(setting).toBeChecked();
  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-pressed", "false");
  await expect(setting).not.toBeChecked();
  await setting.click();
  await expect(toggle).toHaveAttribute("aria-pressed", "true");
  const writes = await page.evaluate(() => window.__uiFixture.calls
    .filter(({ command }) => command === "set_app_preferences").map(({ args }) => args.settings));
  const enabled = { ...original, alwaysOnTop: true };
  expect(writes).toEqual([enabled, original, enabled]);
});

test("在设置页之外提示置顶保存失败并允许重试", async ({ page }) => {
  await openDesktop(page);
  await openWorkspace(page, "概览");
  const toggle = pin(page);
  await page.evaluate(() => { window.__uiFixture.preferencesFault = "隔离置顶保存失败"; });
  await toggle.click();
  await expect(page.getByText("置顶窗口失败", { exact: true })).toBeVisible();
  await expect(page.getByText("隔离置顶保存失败", { exact: true })).toBeVisible();
  await expect(toggle).toHaveAttribute("aria-pressed", "false");
  await expect(toggle).toBeEnabled();
  await page.evaluate(() => { window.__uiFixture.preferencesFault = null; });
  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-pressed", "true");
});

test("已保存置顶和窗口按钮在浅深主题及不同宽度下正常显示", async ({ page }, testInfo) => {
  await openDesktop(page, true);
  const toggle = pin(page);
  await expect(toggle).toHaveAttribute("aria-pressed", "true");
  const controls = page.getByRole("group", { name: "窗口控制", exact: true });
  await expect(controls.getByRole("button").first()).toHaveAccessibleName(LABEL);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    for (const width of [360, 900, 1440]) {
      await page.setViewportSize({ width, height: 720 });
      const box = await controls.boundingBox();
      expect(box.x).toBeGreaterThanOrEqual(0);
      expect(box.x + box.width).toBeLessThanOrEqual(width);
      await expect(toggle).toBeVisible();
    }
    await page.locator(".ccc-topbar").screenshot({ path: testInfo.outputPath(`window-pin-${theme}.png`) });
  }
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("普通浏览器预览隐藏原生窗口按钮", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await expect(page.getByRole("main", { name: "概览", exact: true })).toBeVisible();
  await expect(pin(page)).toHaveCount(0);
});
