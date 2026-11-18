import { configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "apply_hotkeys"));

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与输入");
});

test("状态通知独立预览确认，保存不切换任何设备", async ({ page }) => {
  const p = panel(page);
  await p.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await p.getByRole("checkbox", { name: "原厂状态通知", exact: true }).check();
  await expect(p.getByLabel("热键策略预览")).toContainText("原厂状态通知");
  await expect(p).toContainText("不执行开关");
  await expect(p).toContainText("按原厂规则识别到 USB 键盘时跳过数字锁提示");
  await openConfigurationReview(page, "原厂热键与屏显");
  expect(await writes(page)).toEqual([]);
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  await expect(p).toContainText("正在监听原厂热键");
  const saved = await writes(page);
  expect(saved[0].args.request.settings).toEqual({ enabled: true, actions: ["stateNotifications"], osd: true });
  const hardware = await page.evaluate(() => window.__uiFixture.calls.filter(({ command }) =>
    /^(apply_|restore_|set_|save_|start_)/.test(command) && command !== "apply_hotkeys"
      && command !== "set_shortcut_recording"));
  expect(hardware).toEqual([]);
});

test("已有动作不继承通知授权，状态记录与桌面显示可分别选择", async ({ page }) => {
  const p = panel(page);
  await p.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await p.getByRole("checkbox", { name: "Fn 锁切换", exact: true }).check();
  await expect(p.getByRole("checkbox", { name: "原厂状态通知", exact: true })).not.toBeChecked();
  await p.getByRole("checkbox", { name: "原厂状态通知", exact: true }).check();
  await p.getByRole("checkbox", { name: "Fn 锁切换", exact: true }).uncheck();
  await p.getByRole("switch", { name: "显示桌面提示", exact: true }).uncheck();
  await openConfigurationReview(page, "原厂热键与屏显");
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  expect((await writes(page))[0].args.request.settings).toEqual({ enabled: true, actions: ["stateNotifications"], osd: false });
  await p.getByRole("button", { name: "临时停止监听", exact: true }).click();
  await expect(p).toContainText("热键监听已停止");
  await expect(p.getByRole("checkbox", { name: "原厂状态通知", exact: true })).toBeChecked();
  await openConfigurationReview(page, "原厂热键与屏显", true);
  await configurationDialog(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(p.getByRole("checkbox", { name: "原厂状态通知", exact: true })).not.toBeChecked();
});

test("通知能力变化撤销旧确认，已选范围仍能取消", async ({ page }) => {
  const p = panel(page);
  await p.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  const checkbox = p.getByRole("checkbox", { name: "原厂状态通知", exact: true });
  await checkbox.check();
  await openConfigurationReview(page, "原厂热键与屏显");
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys;
    h.facts.capabilities.find(c => c.scope === "stateNotifications").identity = null;
    h.revision = "changed-notification-version";
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(configurationDialog(page).getByRole("button", { name: "确认应用", exact: true })).toHaveCount(0);
  await expect(checkbox).toBeEnabled();
  await checkbox.uncheck();
  expect(await writes(page)).toEqual([]);
});

test("原厂状态在独立屏显显示，屏显窗口只读取提示", async ({ page }) => {
  await page.goto("/?surface=osd");
  for (const [index, title] of ["数字锁", "大写锁", "滚动锁", "扩展坞", "GPS", "静音模式", "移动网络", "风扇除尘", "Turbo 风扇", "充电状态"].entries()) {
    await page.evaluate(({ sequence, title }) => window.__uiEmit("osd-state", {
      sequence, revision: sequence, message: { title, detail: "原厂报告：已开启", error: false, level: null },
      visible: true, appearance: null, appearanceError: null, error: null,
    }), { sequence: index + 1, title });
    await expect(page.getByRole("status")).toContainText(title);
    await expect(page.getByRole("status")).toContainText("原厂报告");
  }
  const unexpected = await page.evaluate(() => window.__uiFixture.calls.filter(call =>
    !["plugin:event|listen", "plugin:event|unlisten", "read_osd"].includes(call.command)));
  expect(unexpected).toEqual([]);
});
