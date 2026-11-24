import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";
const card = page => page.getByRole("region", { name: "飞行模式", exact: true });
const calls = page => page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply|restore)_airplane$/.test(call.command)));
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "设备配置");
});
test("飞行模式开关直接提交并回读，写入期间锁定模块", async ({ page }) => {
  const p = card(page);
  const toggle = p.getByRole("switch", { name: "飞行模式", exact: true });
  await expect(toggle).not.toBeChecked();
  await expect(toggle).toHaveCSS("cursor", "pointer");
  await expect(toggle).toHaveAccessibleDescription("开启后可能断开无线连接");
  expect(await calls(page)).toEqual([]);
  await page.evaluate(() => {
    const ipc = window.__TAURI_INTERNALS__;
    const invoke = ipc.invoke;
    ipc.invoke = async (command, args) => {
      if (command === "apply_airplane") await new Promise(resolve => { window.__finishAirplaneWrite = resolve; });
      return invoke(command, args);
    };
  });
  await toggle.click();
  await expect(p.getByRole("button", { name: "重新读取飞行模式", exact: true })).toHaveCSS("cursor", "not-allowed");
  await expect.poll(() => page.evaluate(() => typeof window.__finishAirplaneWrite)).toBe("function");
  await page.evaluate(() => window.__finishAirplaneWrite());
  await expect(toggle).toBeChecked();
  expect((await calls(page))[0].args.request.enabled).toBe(true);
  await expect(p.getByText(/^(已开启|已关闭)$/)).toHaveCount(0);
  await expect(p.getByRole("button", { name: "恢复初始设置", exact: true })).toHaveCount(0);
});
test("失败保留错误与当前状态，未知状态禁用开关", async ({ page }) => {
  const p = card(page);
  const toggle = p.getByRole("switch", { name: "飞行模式", exact: true });
  await page.evaluate(() => { window.__uiFixture.airplaneFault = "原厂设备IOCTL失败"; });
  await toggle.click();
  await expect(p.getByRole("alert")).toContainText("原厂设备IOCTL失败");
  await expect(toggle).not.toBeChecked();
  await page.evaluate(() => { window.__uiFixture.airplane.current = null; window.__uiFixture.airplane.error = "系统报告未知"; });
  await p.getByRole("button", { name: "重新读取飞行模式", exact: true }).click();
  await expect(toggle).toBeDisabled();
  await expect(toggle).toHaveCSS("cursor", "not-allowed");
  await expect(p.getByRole("alert")).toContainText("系统报告未知");
});
test("两个原厂事件分别授权，预览与浏览不发送一次性请求", async ({ page }) => {
  await openWorkspace(page, "键盘与灯光");
  const p = page.getByRole("region", { name: "原厂热键与屏显", exact: true });
  await p.getByRole("button", { name: "配置热键", exact: true }).click();
  const editor = page.getByRole("dialog", { name: "配置热键与屏显", exact: true });
  await editor.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await editor.getByRole("checkbox", { name: "飞行模式按键", exact: true }).check();
  await editor.getByRole("checkbox", { name: "原厂面板维护", exact: true }).check();
  await editor.getByRole("button", { name: "完成编辑", exact: true }).click();
  await previewConfiguration(page, "原厂热键与屏显");
  expect(await calls(page)).toEqual([]);
  await configurationView(page).getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  const actions = await page.evaluate(() => window.__uiFixture.calls.find(call => call.command === "apply_hotkeys").args.request.settings.actions);
  expect(actions).toEqual(["airplaneMode", "panelEvent"]);
});
test("飞行模式左右卡片在窄屏与主题切换后保持可操作", async ({ page }) => {
  for (const width of [360, 760, 1440]) {
    await page.setViewportSize({ width, height: 960 });
    for (const theme of ["light", "dark"]) {
      await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
      const p = card(page); await p.scrollIntoViewIfNeeded();
      await expect(p.locator('[data-icon="airplane"]')).toHaveCount(1);
      await expect(p.locator('svg.ccc-airplane-artwork')).toHaveCount(1);
      expect(await p.evaluate(element => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
      await p.screenshot({ path: `target/ui-check/airplane-${width}-${theme}.png` });
    }
  }
});
