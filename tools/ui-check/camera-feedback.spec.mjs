import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const camera = page => panel(page).getByRole("checkbox", { name: "摄像头状态反馈", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => c.command === "apply_hotkeys"));
async function select(page) {
  await panel(page).getByRole("checkbox", { name: "启用原厂热键接管", exact: true }).check();
  await camera(page).check();
}
async function confirm(page) {
  await panel(page).getByRole("button", { name: "应用热键策略", exact: true }).click();
  await panel(page).getByRole("button", { name: "确认此范围的热键接管？", exact: true }).click();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/"); await openWorkspace(page, "键盘与输入");
});

test("摄像头反馈默认关闭并独立确认，既有通知和麦克风范围不自动包含", async ({ page }) => {
  const p = panel(page);
  await expect(camera(page)).not.toBeChecked();
  await p.getByRole("checkbox", { name: "启用原厂热键接管", exact: true }).check();
  for (const title of ["原厂状态通知", "麦克风静音"]) {
    await p.getByRole("checkbox", { name: title, exact: true }).check();
    await expect(camera(page)).not.toBeChecked();
    await p.getByRole("checkbox", { name: title, exact: true }).uncheck();
  }
  await camera(page).check();
  await expect(p.getByLabel("热键策略预览")).toContainText("摄像头状态反馈");
  await expect(p).toContainText("按原厂规则同步摄像头保存值");
  await expect(p).toContainText("不执行设备开关");
  await p.getByRole("button", { name: "应用热键策略", exact: true }).click();
  expect(await writes(page)).toEqual([]);
  await p.getByRole("button", { name: "确认此范围的热键接管？", exact: true }).click();
  expect((await writes(page))[0].args.request.settings).toEqual({ enabled: true, actions: ["cameraReports"], osd: true });
  const devices = await page.evaluate(() => window.__uiFixture.calls.filter(({ command }) =>
    /^(apply_|restore_|set_|save_|start_)/.test(command) && !["apply_hotkeys", "set_shortcut_recording"].includes(command)));
  expect(devices).toEqual([]);
});

test("关闭桌面提示仍显示反馈结果，临时停止与初始软件规则恢复分开", async ({ page }) => {
  const p = panel(page);
  await select(page);
  await p.getByRole("checkbox", { name: "显示桌面提示", exact: true }).uncheck();
  await confirm(page);
  await page.evaluate(() => window.__uiEmit("hotkeys-state", {
    running: true, error: null, lastResult: "摄像头：原厂报告：已开启；摄像头保存值已同步",
  }));
  await expect(p).toContainText("摄像头保存值已同步");
  expect((await writes(page))[0].args.request.settings.osd).toBe(false);
  await p.getByRole("button", { name: "临时停止监听", exact: true }).click();
  await expect(p).toContainText("热键监听已停止");
  await expect(camera(page)).toBeChecked();
  await p.getByRole("button", { name: "恢复初始热键策略", exact: true }).click();
  await p.getByRole("button", { name: "确认恢复初始软件策略？", exact: true }).click();
  await expect(camera(page)).not.toBeChecked();
  await expect(p.getByRole("checkbox", { name: "启用原厂热键接管", exact: true })).not.toBeChecked();
});

test("摄像头能力变化撤销旧确认并允许取消已选范围", async ({ page }) => {
  await select(page);
  await panel(page).getByRole("button", { name: "应用热键策略", exact: true }).click();
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys;
    h.revision = "camera-source-unavailable";
    const cap = h.facts.capabilities.find(c => c.scope === "cameraReports");
    cap.identity = null; cap.reason = "摄像头报告保存值不可读取";
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(panel(page).getByRole("button", { name: "确认此范围的热键接管？", exact: true })).toHaveCount(0);
  await expect(panel(page)).toContainText("摄像头报告保存值不可读取");
  await expect(camera(page)).toBeEnabled();
  await camera(page).uncheck();
  await expect(camera(page)).toBeDisabled();
  expect(await writes(page)).toEqual([]);
});

test("保存失败保留摄像头草稿并可重新确认", async ({ page }) => {
  await select(page);
  await page.evaluate(() => { window.__uiFixture.hotkeysFault = "摄像头反馈策略原子保存失败"; });
  await confirm(page);
  await expect(page.getByText("摄像头反馈策略原子保存失败", { exact: true }).first()).toBeVisible();
  await expect(camera(page)).toBeChecked();
  await confirm(page);
  await expect(panel(page)).toContainText("正在监听原厂热键");
});

test("摄像头及组合OSD保留原厂报告语义且不请求设备开关", async ({ page }) => {
  await page.goto("/?surface=osd");
  for (const [index, title] of ["摄像头", "摄像头与麦克风"].entries()) {
    await page.evaluate(({ sequence, title }) => window.__uiEmit("osd-state", {
      sequence, revision: sequence, message: { title, detail: "原厂报告：已开启；摄像头保存值已同步", error: false, level: null },
      visible: true, appearance: null, appearanceError: null, error: null,
    }), { sequence: index + 1, title });
    await expect(page.getByRole("status")).toContainText(title);
    await expect(page.getByRole("status")).toContainText("原厂报告");
  }
  const commands = await page.evaluate(() => window.__uiFixture.calls.filter(c =>
    !["plugin:event|listen", "plugin:event|unlisten", "read_osd"].includes(c.command)));
  expect(commands).toEqual([]);
});

test("摄像头反馈在深浅宽窄布局保留模块边框和间隔", async ({ page }) => {
  const row = panel(page).locator(".ccc-hotkey-scope").filter({ has: page.getByRole("checkbox", { name: "摄像头状态反馈", exact: true }) });
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 900 });
      await row.scrollIntoViewIfNeeded();
      const dimensions = await row.evaluate(el => ({
        overflow: el.scrollWidth > el.clientWidth + 1, right: el.getBoundingClientRect().right,
        border: parseFloat(getComputedStyle(el.closest(".ccc-hotkey-section")).borderTopWidth),
        gap: parseFloat(getComputedStyle(el.closest(".ccc-hotkey-columns")).gap),
      }));
      expect(dimensions.overflow).toBe(false);
      expect(dimensions.right).toBeLessThanOrEqual(width);
      expect(dimensions.border).toBeGreaterThan(0);
      expect(dimensions.gap).toBeGreaterThan(0);
      if (width !== 760) await page.screenshot({ path: `target/ui-check/camera-feedback-${theme}-${width}.png` });
    }
  }
});
