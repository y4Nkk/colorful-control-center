import { expect, test } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

async function launch(page, setup) {
  await page.addInitScript({ content: `${MOCK_IPC}
    const data = window.__uiFixture;
    ${setup}
  ` });
  await page.goto("/");
}

const progress = page => page.getByRole("progressbar", { name: "初始化进度" });
const nav = page => page.getByRole("navigation", { name: "主导航" });
const calls = page => page.evaluate(() => window.__uiFixture.calls.map(call => call.command));
const advance = (page, stage) => page.evaluate(stage => { window.__uiFixture.preparation = { stage }; }, stage);

test("启动页跟随真实阶段、允许窗口操作，并直接进入上次访问页面", async ({ page }, testInfo) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await launch(page, `
    data.preferences.startupPage = "lastVisited";
    data.preferences.motion = "system";
    localStorage.setItem("ccc-last-page", "graphics");
    data.preparation = { stage: "checkingOem" };
    window.isTauri = true;
    const ipc = window.__TAURI_INTERNALS__;
    ipc.metadata = { currentWindow: { label: "main" } };
    const invoke = ipc.invoke;
    ipc.invoke = (command, args) => {
      if (["window_is_maximized", "window_minimize"].includes(command)) {
        data.calls.push({ command });
        return Promise.resolve(false);
      }
      if (command === "get_app_preferences" && !data.preferencesReady) {
        return new Promise(resolve => { data.releasePreferences = () => resolve(invoke(command, args)); });
      }
      return invoke(command, args);
    };
  `);
  await expect(page.getByText("正在检查原厂控制软件…")).toBeVisible();
  await expect(page.locator(".ccc-preparation-logo")).toBeVisible();
  await expect(progress(page)).toBeVisible();
  await expect(progress(page).locator("span")).toHaveCSS("animation-name", "ccc-progress");
  await expect(page.getByText(/已用时 0:0[1-9]/)).toBeVisible();
  await page.getByRole("button", { name: "最小化窗口", exact: true }).click();
  expect(await calls(page)).toContain("window_minimize");
  await advance(page, "hardware");
  await expect(page.getByText("正在初始化硬件控制…")).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath("preparation-light.png"), animations: "disabled" });
  await expect(nav(page)).toHaveCount(0);
  expect(await calls(page)).not.toContain("read_hotkey_status");
  await advance(page, "services");
  await expect(page.getByText("正在启动后台服务…")).toBeVisible();
  await advance(page, "ready");
  await expect(page.getByText("正在读取软件偏好…")).toBeVisible();
  await expect(nav(page)).toHaveCount(0);
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.preferencesReady = true;
    data.releasePreferences();
  });
  await expect(page.getByRole("main", { name: "设备配置", exact: true })).toBeVisible();
  await expect(progress(page)).toHaveCount(0);
});

test("窄窗深色启动页按偏好停止动画，唤回请求待就绪后生效", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 380, height: 420 });
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await launch(page, `
    data.preferences.theme = "dark";
    data.preferences.motion = "reduce";
    data.preparation = { stage: "hardware" };
  `);
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect(progress(page).locator("span")).toHaveCSS("animation-name", "none");
  await expect(page.getByRole("button", { name: "退出应用", exact: true })).toBeInViewport();
  await page.screenshot({ path: testInfo.outputPath("preparation-dark.png") });
  await page.evaluate(() => window.__uiEmit("tray-navigation-request", { kind: "cooling" }));
  await expect(nav(page)).toHaveCount(0);
  await advance(page, "ready");
  await expect(page.getByRole("main", { name: "性能与散热", exact: true })).toBeVisible();
});

test("初始化失败显示原始原因并提供日志、重启和退出", async ({ page }, testInfo) => {
  await launch(page, `
    data.preparation = { stage: "failed", detail: "隔离诊断：初始化硬件控制失败" };
    const ipc = window.__TAURI_INTERNALS__, invoke = ipc.invoke;
    ipc.invoke = (command, args) => {
      if (command === "restart_application") {
        data.calls.push({ command });
        return Promise.reject("隔离诊断：应用重启失败");
      }
      return invoke(command, args);
    };
  `);
  await expect(page.getByRole("heading", { name: "初始化失败" })).toBeVisible();
  await expect(page.getByRole("alert")).toHaveText("隔离诊断：初始化硬件控制失败");
  await expect(progress(page)).toHaveCount(0);
  await expect(nav(page)).toHaveCount(0);
  await page.screenshot({ path: testInfo.outputPath("preparation-failed.png") });
  await page.getByRole("button", { name: "打开日志目录", exact: true }).click();
  await page.getByRole("button", { name: "重新启动", exact: true }).click();
  await expect(page.getByText("重新启动失败。 隔离诊断：应用重启失败")).toBeVisible();
  await page.getByRole("button", { name: "退出应用", exact: true }).click();
  expect(await calls(page)).toEqual(expect.arrayContaining(["open_startup_logs", "restart_application", "exit_application"]));
});

test("初始化状态连接失败后可以重新检查", async ({ page }) => {
  await launch(page, `
    data.connectionFault = true;
    const ipc = window.__TAURI_INTERNALS__, invoke = ipc.invoke;
    ipc.invoke = (command, args) => {
      if (command === "read_application_preparation" && data.connectionFault) return Promise.reject("隔离连接失败");
      return invoke(command, args);
    };
  `);
  await expect(page.getByRole("alert")).toHaveText("隔离连接失败");
  await page.evaluate(() => { window.__uiFixture.connectionFault = false; });
  await page.getByRole("button", { name: "重新检查", exact: true }).click();
  await expect(page.getByRole("main", { name: "概览", exact: true })).toBeVisible();
});

for (const recovery of ["重新读取", "进入设置"]) {
  test(`软件偏好读取失败后可以${recovery}`, async ({ page }) => {
    await launch(page, `
      data.preferencesFault = true;
      const ipc = window.__TAURI_INTERNALS__, invoke = ipc.invoke;
      ipc.invoke = (command, args) => {
        if (command === "get_app_preferences" && data.preferencesFault) return Promise.reject("隔离偏好不可读");
        return invoke(command, args);
      };
    `);
    await expect(page.getByRole("alert")).toHaveText("隔离偏好不可读");
    await expect(nav(page)).toHaveCount(0);
    if (recovery === "重新读取") await page.evaluate(() => { window.__uiFixture.preferencesFault = false; });
    await page.getByRole("button", { name: recovery, exact: true }).click();
    await expect(page.getByRole("main", { name: recovery === "重新读取" ? "概览" : "设置", exact: true })).toBeVisible();
    await expect(progress(page)).toHaveCount(0);
  });
}
