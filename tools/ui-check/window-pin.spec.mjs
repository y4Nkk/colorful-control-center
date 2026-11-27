import { expect, test } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";

const LABEL = "窗口始终置顶";
const pin = page => page.getByRole("button", { name: LABEL, exact: true });

async function openDesktop(page, savedPin = false, administratorAccess = "canElevate") {
  await page.addInitScript({ content: `${MOCK_IPC}
    (() => {
      window.isTauri = true;
      const data = window.__uiFixture;
      data.preferences.alwaysOnTop = ${savedPin};
      data.administratorAccess = ${JSON.stringify(administratorAccess)};
      const ipc = window.__TAURI_INTERNALS__;
      ipc.metadata = { currentWindow: { label: "main" } };
      const invoke = ipc.invoke;
      ipc.invoke = async (command, args) => {
        if (command === "window_is_maximized") return false;
        if (command === "set_app_preferences") {
          if (data.holdPreferences) await new Promise(resolve => { data.finishPreferences = resolve; });
          if (data.preferencesFault) throw new Error(data.preferencesFault);
        }
        if (command === "restart_as_administrator" && data.holdAdministratorRestart) {
          data.calls.push({ command, args });
          return new Promise((_, reject) => { data.cancelAdministratorRestart = reject; });
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
  const tooltip = page.getByRole("tooltip");
  const original = await page.evaluate(() => window.__uiFixture.preferences);
  await expect(toggle).toHaveAttribute("aria-pressed", "false");
  await toggle.focus();
  await expect(tooltip).toHaveText("置顶窗口");
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
  await toggle.hover();
  await expect(tooltip).toHaveText("取消置顶");
  await page.evaluate(() => window.dispatchEvent(new Event("blur")));
  await expect(tooltip).toHaveCount(0);
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
  await page.getByRole("button", { name: "通知（1 条）" }).click();
  const notification = page.getByRole("dialog", { name: "通知", exact: true });
  await expect(notification.getByText("隔离置顶保存失败", { exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
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
      await expect(controls).toBeInViewport({ ratio: 1 });
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
  await expect(page.getByRole("button", { name: "以管理员运行", exact: true })).toHaveCount(0);
});

test("普通权限下管理员按钮确认后才请求重启，取消授权保留当前权限图标", async ({ page }) => {
  await openDesktop(page);
  await page.evaluate(() => {
    window.__uiFixture.gpu.writeBlockedReason = "调整或恢复 GPU 偏移需要管理员权限。";
  });
  await openWorkspace(page, "性能与散热");
  const trigger = page.getByRole("button", { name: "以管理员运行", exact: true });
  const dialog = page.getByRole("dialog", { name: "以管理员运行", exact: true });
  const gpuTrigger = page.getByRole("button", { name: "以管理员身份重启", exact: true });
  const restartCalls = () => page.evaluate(() => window.__uiFixture.calls
    .filter(call => call.command === "restart_as_administrator").length);
  await expect(trigger.locator('[data-icon="shield-off"]')).toBeVisible();
  await trigger.press("Enter");
  await dialog.getByRole("button", { name: "取消", exact: true }).click();
  await expect(trigger).toBeFocused();
  expect(await restartCalls()).toBe(0);

  await page.evaluate(() => { window.__uiFixture.holdAdministratorRestart = true; });
  await trigger.click();
  await dialog.getByRole("button", { name: "请求 Windows 授权", exact: true }).click();
  await expect(trigger).toBeDisabled();
  await expect(gpuTrigger).toBeDisabled();
  expect(await restartCalls()).toBe(1);
  await page.evaluate(() => window.__uiFixture.cancelAdministratorRestart("已取消 Windows 管理员授权"));
  await expect(trigger).toBeEnabled();
  await expect(gpuTrigger).toBeEnabled();
  await expect(trigger.locator('[data-icon="shield-off"]')).toBeVisible();
  await expect(page.getByText("以管理员运行失败", { exact: true })).toBeVisible();
  await gpuTrigger.click();
  await expect(dialog).toBeVisible();
  await expect(page.locator("dialog[open]")).toHaveCount(1);
  await dialog.getByRole("button", { name: "取消", exact: true }).click();
  await expect(gpuTrigger).toBeFocused();
  expect(await restartCalls()).toBe(1);
});

for (const [access, label, icon] of [
  ["elevated", "已以管理员身份运行", "shield-check"],
  ["standard", "当前 Windows 账户无法提升为管理员", "shield-off"],
]) {
  test(`管理员按钮显示真实状态 · ${label}`, async ({ page }) => {
    await openDesktop(page, false, access);
    const status = page.getByRole("button", { name: label, exact: true });
    const symbol = status.locator(`[data-icon="${icon}"]`);
    await expect(status).toBeDisabled();
    await expect(symbol).toBeVisible();
    await status.hover();
    await expect(page.getByRole("tooltip")).toHaveText(label);
    if (access === "elevated") {
      for (const [theme, color] of [["light", "rgb(30, 142, 62)"], ["dark", "rgb(76, 217, 100)"]]) {
        await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
        await expect(symbol).toHaveCSS("stroke", color);
        await expect(status).toHaveCSS("opacity", "1");
        await expect(status).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
        await expect(status).toHaveCSS("border-top-width", "0px");
      }
    }
    await status.press("Enter");
    await expect(page.getByRole("dialog", { name: "以管理员运行", exact: true })).toHaveCount(0);
    expect(await page.evaluate(() => window.__uiFixture.calls
      .some(call => call.command === "restart_as_administrator"))).toBe(false);
  });
}
