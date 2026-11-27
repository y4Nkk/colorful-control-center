import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "设备配置"); });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(c => c.command === command).length, command);
const mode = (panel, label) => panel.getByRole("button", { name: label, exact: true });

test("四种显卡模式以不同的 SVG 通路示意，宽窄窗保留标签和选中态", async ({ page }) => {
  const panel = page.getByRole("region", { name: "显卡模式", exact: true });
  for (const [label, count] of [["仅集显", 1], ["仅独显", 1], ["混合显卡", 2], ["动态切换", 3]]) {
    const art = mode(panel, label).locator(".ccc-graphics-choice-art");
    await expect(art).toHaveAttribute("aria-hidden", "true");
    await expect(art).toHaveAttribute("fill", "none");
    const routes = art.locator("[data-routes]");
    await expect(routes.locator(":scope > path")).toHaveCount(count);
    if (label === "动态切换") await expect(routes).toHaveAttribute("stroke-dasharray", ".1 5.5");
    // 强调填充只属于选中项里位于通路上的 GPU。
    await expect(art.locator("[data-solid]")).toHaveCount(label === "混合显卡" ? 2 : 0);
  }
  for (const [width, theme] of [[1440, "light"], [360, "dark"]]) {
    await page.setViewportSize({ width, height: 900 });
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    const choices = panel.getByRole("group", { name: "切换显卡模式" });
    const layout = await choices.getByRole("button").evaluateAll(buttons => buttons.map(button => {
      const box = button.getBoundingClientRect();
      const art = button.querySelector("svg").getBoundingClientRect();
      const text = button.querySelector("span").getBoundingClientRect();
      return box.left >= 0 && box.right <= innerWidth && art.width > 0 && art.bottom < text.top;
    }));
    expect(layout).toEqual([true, true, true, true]);
    await expect(mode(panel, "混合显卡")).toHaveAttribute("aria-pressed", "true");
    await panel.screenshot({ path: `target/ui-check/graphics-mode-art-${theme}-${width}.png` });
  }
  expect(await calls(page, "apply_graphics_mode")).toBe(0);
});

test("显卡按钮直接发送选择，固件旧报告与待重启目标分开显示", async ({ page }) => {
  const panel = page.getByRole("region", { name: "显卡模式", exact: true });
  await expect(mode(panel, "混合显卡")).toHaveAttribute("aria-pressed", "true");
  await expect(panel.getByText("固件报告", { exact: true })).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "重启 Windows", exact: true })).toHaveCount(0);
  await expect(panel.getByRole("radio")).toHaveCount(0);
  await expect(panel.getByRole("checkbox")).toHaveCount(0);
  await expect(panel.getByRole("combobox")).toHaveCount(0);
  expect(await calls(page, "apply_graphics_mode")).toBe(0);
  await mode(panel, "仅独显").press("Enter");
  await expect(panel.getByRole("status")).toContainText("仅独显 · 重启 Windows 后生效");
  await expect(panel).toContainText(/固件报告\s*混合显卡/);
  await expect(mode(panel, "仅独显")).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByRole("region", { name: "页面操作", exact: true })).toHaveCount(0);
  await panel.screenshot({ path: "target/ui-check/graphics-mode-pending.png" });

  await mode(panel, "仅独显").click();
  expect(await calls(page, "apply_graphics_mode")).toBe(1);
  expect(await calls(page, "reboot_for_graphics_mode")).toBe(0);
  await panel.getByRole("button", { name: "重启 Windows", exact: true }).click();
  expect(await calls(page, "reboot_for_graphics_mode")).toBe(0);
  await panel.getByRole("button", { name: "已保存工作，确认重启 Windows", exact: true }).click();
  await expect.poll(() => calls(page, "reboot_for_graphics_mode")).toBe(1);
  await page.evaluate(() => {
    window.__uiFixture.graphicsMode.current.mode = 2;
    window.__uiFixture.graphicsMode.lastRequest.verification = "confirmed";
  });
  await panel.getByRole("button", { name: "重新读取显卡模式", exact: true }).click();
  await expect(panel.getByRole("status")).toContainText("重启后已确认");
  await mode(panel, "混合显卡").click();
  await expect(panel).toContainText(/固件报告\s*仅独显/);
  await expect(panel.getByRole("status")).toContainText("混合显卡 · 重启 Windows 后生效");
  await expect(panel.getByRole("button", { name: "恢复初始设置", exact: true })).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("发送失败保持固件回读，重新选择可重试", async ({ page }) => {
  const panel = page.getByRole("region", { name: "显卡模式", exact: true });
  await page.evaluate(() => { window.__uiFixture.graphicsModeFault = "选择记录保存失败，未发送硬件命令"; });
  await mode(panel, "仅独显").click();
  await expect(panel.getByRole("alert")).toContainText("记录保存失败");
  await expect(mode(panel, "混合显卡")).toHaveAttribute("aria-pressed", "true");
  await expect(panel.getByRole("button", { name: "重启 Windows", exact: true })).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.graphicsReboots)).toBe(0);
  await mode(panel, "仅独显").click();
  await expect(panel.getByRole("status")).toContainText("重启 Windows 后生效");
  expect(await calls(page, "apply_graphics_mode")).toBe(2);
});

test("固件候选范围限制按钮，可直接重发未完成的选择", async ({ page }) => {
  await page.evaluate(() => {
    const profile = window.__uiFixture.graphicsMode;
    profile.current.availableMask = 6;
    profile.lastRequest = { target: 2, verification: "uncertain" };
  });
  const panel = page.getByRole("region", { name: "显卡模式", exact: true });
  await panel.getByRole("button", { name: "重新读取显卡模式", exact: true }).click();
  await expect(panel.getByRole("status")).toContainText("上次操作未完成");
  await expect(panel.getByRole("button", { name: "重启 Windows", exact: true })).toHaveCount(0);
  await expect(mode(panel, "仅集显")).toHaveCount(0);
  await expect(mode(panel, "动态切换")).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "恢复初始设置", exact: true })).toHaveCount(0);
  await mode(panel, "仅独显").click();
  await expect(panel.getByRole("status")).toContainText("仅独显 · 重启 Windows 后生效");
  expect(await calls(page, "apply_graphics_mode")).toBe(1);
});
