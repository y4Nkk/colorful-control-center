import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const applyCalls = page => page.evaluate(() => window.__uiFixture.calls.filter(c => c.command === "apply_hotkeys"));

async function open(page, kind = 22) {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(kind => { window.__uiFixture.keyboard.detectedType = kind; }, kind);
  await page.goto("/"); await openWorkspace(page, "键盘与灯光");
}

test("138范围独立确认，亮度加减和总开关不会自动包含循环", async ({ page }) => {
  await open(page);
  const p = panel(page), cycle = p.getByRole("checkbox", { name: "原厂亮度循环与反馈", exact: true });
  await expect(cycle).toBeEnabled();
  await p.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await p.getByRole("checkbox", { name: "键盘亮度", exact: true }).check();
  await p.getByRole("checkbox", { name: "键盘灯开关", exact: true }).check();
  await expect(cycle).not.toBeChecked();
  await cycle.check();
  await p.getByRole("checkbox", { name: "键盘亮度", exact: true }).uncheck();
  await p.getByRole("checkbox", { name: "键盘灯开关", exact: true }).uncheck();
  await expect(p.getByLabel("热键策略预览")).toContainText("原厂亮度循环与反馈");
  await expect(p).toContainText("关闭时保留亮度");
  await previewConfiguration(page, "原厂热键与屏显");
  expect(await applyCalls(page)).toEqual([]);
  await configurationView(page).getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  expect((await applyCalls(page))[0].args.request.settings.actions).toEqual(["keyboardBrightnessEvent"]);
  const hardware = await page.evaluate(() => window.__uiFixture.calls.filter(({ command }) =>
    /^(apply_|restore_|set_)/.test(command) && !["apply_hotkeys", "set_shortcut_recording"].includes(command)));
  expect(hardware).toEqual([]);
});

test("普通其他型号禁用138，白光型号说明同步报告并复用停止恢复", async ({ page }) => {
  await open(page, 2);
  await expect(panel(page).getByRole("checkbox", { name: "原厂亮度循环与反馈", exact: true })).toBeDisabled();
  await expect(panel(page)).toContainText("此键盘未提供原厂亮度循环或反馈");
  await page.addInitScript(() => { window.__uiFixture.keyboard.detectedType = 1; });
  await page.reload();
  const p = panel(page), cycle = p.getByRole("checkbox", { name: "原厂亮度循环与反馈", exact: true });
  await expect(cycle).toBeEnabled();
  await expect(p).toContainText("读取固件档位并同步保存，不发送亮度命令");
  await p.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check(); await cycle.check();
  await previewConfiguration(page, "原厂热键与屏显");
  await configurationView(page).getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  await p.getByRole("button", { name: "临时停止监听", exact: true }).click();
  await expect(p).toContainText("热键监听已停止"); await expect(cycle).toBeChecked();
  await previewConfiguration(page, "原厂热键与屏显", true);
  await configurationView(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(cycle).not.toBeChecked();
});

test("类型或档位能力变化使旧确认失效，仍允许取消已选范围", async ({ page }) => {
  await open(page);
  const p = panel(page), cycle = p.getByRole("checkbox", { name: "原厂亮度循环与反馈", exact: true });
  await p.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check(); await cycle.check();
  await previewConfiguration(page, "原厂热键与屏显");
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys; h.revision = "changed-138";
    const cap = h.facts.capabilities.find(c => c.scope === "keyboardBrightnessEvent");
    cap.identity = null; cap.reason = "亮度事件设备或规则变化";
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(configurationView(page).getByRole("button", { name: "保存热键接管范围", exact: true })).toHaveCount(0);
  await expect(cycle).toBeEnabled(); await cycle.uncheck();
  expect(await applyCalls(page)).toEqual([]);
});

test("138规则在深浅宽窄布局可读且卡片保持边框间隔", async ({ page }) => {
  await open(page);
  const row = panel(page).locator(".ccc-hotkey-scope").filter({ has: page.getByRole("checkbox", { name: "原厂亮度循环与反馈", exact: true }) });
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 900 }); await row.scrollIntoViewIfNeeded();
      const values = await row.evaluate(el => {
        const surface = el.closest(".ccc-hotkey-section"), box = el.getBoundingClientRect();
        return { overflow: el.scrollWidth > el.clientWidth + 1, right: box.right,
          border: parseFloat(getComputedStyle(surface).borderTopWidth), gap: parseFloat(getComputedStyle(surface.parentElement).gap) };
      });
      expect(values.overflow).toBe(false); expect(values.right).toBeLessThanOrEqual(width);
      expect(values.border).toBeGreaterThan(0); expect(values.gap).toBeGreaterThan(0);
      if (width !== 760) await page.screenshot({ path: `target/ui-check/brightness-event-${theme}-${width}.png` });
    }
  }
});
