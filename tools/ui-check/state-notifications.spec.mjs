import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const changes = page => panel(page).getByRole("region", { name: "原厂热键与屏显 · 保存热键接管范围", exact: true });
const editor = page => page.getByRole("dialog", { name: "配置热键与屏显", exact: true });
const edit = page => panel(page).getByRole("button", { name: "配置热键", exact: true }).click();
const done = page => editor(page).getByRole("button", { name: "完成编辑", exact: true }).click();
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "apply_hotkeys"));

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与灯光");
});

test("状态通知独立预览确认，保存不切换任何设备", async ({ page }) => {
  const p = panel(page);
  await edit(page);
  await editor(page).getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await editor(page).getByRole("checkbox", { name: "原厂状态通知", exact: true }).check();
  await done(page);
  const review = changes(page);
  await expect(review).toContainText("原厂状态通知");
  await expect(review).toContainText("只报告");
  await expect(review).toContainText("寂静模式");
  await expect(review).toContainText("按原厂规则识别到 USB 键盘时跳过数字锁提示");
  expect(await writes(page)).toEqual([]);
  await review.getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  await expect(p.getByRole("status")).toContainText("正在监听");
  const saved = await writes(page);
  expect(saved[0].args.request.settings).toEqual({ enabled: true, actions: ["stateNotifications"], osd: true });
  const hardware = await page.evaluate(() => window.__uiFixture.calls.filter(({ command }) =>
    /^(apply_|restore_|set_|save_|start_)/.test(command) && command !== "apply_hotkeys"
      && command !== "set_shortcut_recording"));
  expect(hardware).toEqual([]);
});

test("已有动作不继承通知授权，状态记录与桌面显示可分别选择", async ({ page }) => {
  const p = panel(page);
  await edit(page);
  const form = editor(page);
  const notice = form.getByRole("checkbox", { name: "原厂状态通知", exact: true });
  const fnLock = form.getByRole("checkbox", { name: "Fn 锁切换", exact: true });
  await form.getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await fnLock.check();
  await expect(notice).not.toBeChecked();
  await notice.check();
  await fnLock.uncheck();
  await form.getByRole("switch", { name: "显示桌面提示", exact: true }).uncheck();
  await done(page);
  await changes(page).getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  expect((await writes(page)).map(call => call.args.request.settings)).toEqual([
    { enabled: false, actions: [], osd: false },
    { enabled: true, actions: ["stateNotifications"], osd: false },
  ]);
  await p.getByRole("button", { name: "详情", exact: true }).click();
  const details = page.getByRole("dialog", { name: "热键状态与诊断", exact: true });
  await details.getByRole("button", { name: "临时停止监听", exact: true }).click();
  await details.getByRole("button", { name: "关闭弹窗", exact: true }).click();
  await expect(p.getByRole("status")).toContainText("已停止监听");
  await edit(page);
  await expect(notice).toBeChecked();
  await done(page);
  await p.getByRole("button", { name: "恢复原厂热键与屏显", exact: true }).click();
  await page.getByRole("dialog", { name: "恢复原厂热键与屏显", exact: true })
    .getByRole("button", { name: "确认恢复", exact: true }).click();
  await edit(page);
  await expect(notice).not.toBeChecked();
});

test("通知能力变化撤销旧确认，已选范围仍能取消", async ({ page }) => {
  await edit(page);
  await editor(page).getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  const checkbox = editor(page).getByRole("checkbox", { name: "原厂状态通知", exact: true });
  await checkbox.check();
  await done(page);
  await expect(changes(page)).toBeVisible();
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys;
    h.facts.capabilities.find(c => c.scope === "stateNotifications").identity = null;
    h.revision = "changed-notification-version";
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(changes(page)).toHaveCount(0);
  await edit(page);
  await expect(checkbox).toBeChecked();
  await checkbox.click();
  await expect(checkbox).toHaveCount(0);
  expect(await writes(page)).toEqual([]);
});

test("原厂状态在独立屏显显示，屏显窗口只读取提示", async ({ page }) => {
  await page.goto("/?surface=osd");
  for (const [index, title] of ["数字锁", "大写锁", "滚动锁", "扩展坞", "GPS", "寂静模式", "移动网络", "风扇除尘", "Turbo 风扇", "充电状态"].entries()) {
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
