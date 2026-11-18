import { configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";

const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "apply_hotkeys"));
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/");
  await openWorkspace(page, "键盘与输入");
});

test("恢复入口单独预览确认，保存策略不打开系统窗口", async ({ page }) => {
  const p = panel(page);
  await p.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await p.getByRole("checkbox", { name: "Windows 恢复入口", exact: true }).check();
  await expect(p.getByLabel("热键策略预览")).toContainText("Windows 恢复入口");
  await openConfigurationReview(page, "原厂热键与屏显");
  expect(await writes(page)).toEqual([]);
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  expect((await writes(page))[0].args.request.settings.actions).toEqual(["systemRecovery"]);
  await expect(p).toContainText("正在监听原厂热键");
  const native = await page.evaluate(() => window.__uiFixture.calls.filter(call => /recovery|launch|shell/.test(call.command)));
  expect(native).toEqual([]);
});

test("程序来源变化撤销旧确认并保留选中的草稿", async ({ page }) => {
  const p = panel(page);
  await p.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  const scope = p.getByRole("checkbox", { name: "Windows 恢复入口", exact: true });
  await scope.check();
  await openConfigurationReview(page, "原厂热键与屏显");
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys; h.revision += "-changed-recovery";
    h.facts.capabilities.find(cap => cap.scope === "systemRecovery").identity = null;
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(configurationDialog(page).getByRole("button", { name: "确认应用", exact: true })).toHaveCount(0);
  await expect(scope).toBeChecked(); await expect(scope).toBeEnabled();
  await scope.uncheck(); expect(await writes(page)).toEqual([]);
});

test("停止保留选择，初始恢复只撤销本软件接管策略", async ({ page }) => {
  const p = panel(page);
  await p.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  const scope = p.getByRole("checkbox", { name: "Windows 恢复入口", exact: true }); await scope.check();
  await openConfigurationReview(page, "原厂热键与屏显");
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  await p.getByRole("button", { name: "临时停止监听", exact: true }).click();
  await expect(scope).toBeChecked();
  await openConfigurationReview(page, "原厂热键与屏显", true);
  await configurationDialog(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(scope).not.toBeChecked();
});
