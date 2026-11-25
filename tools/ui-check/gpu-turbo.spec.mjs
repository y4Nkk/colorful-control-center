import { openHotkeyDetails, openWorkspace, previewHotkeyRestore } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const facts = page => page.getByRole("region", { name: "原厂 GPU Turbo 事实与预览", exact: true });
const editor = page => page.getByRole("dialog", { name: "配置热键与屏显", exact: true });
const scope = page => editor(page).getByRole("checkbox", { name: "原厂 GPU Turbo", exact: true });
const edit = page => panel(page).getByRole("button", { name: "配置热键", exact: true }).click();
const done = page => editor(page).getByRole("button", { name: "完成编辑", exact: true }).click();
const details = page => panel(page).getByRole("button", { name: "详情", exact: true }).click();
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(c => c.command === command), command);
async function initial(page) {
  await expect(panel(page).getByRole("button", { name: "配置热键", exact: true })).toBeEnabled();
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, gpu = h.facts.gpuTurbo;
    gpu.initial = { devices: [gpu.current.devices[0].id], locks: structuredClone(gpu.current.locks), offsets: [{ coreKhz: 10000, memKhz: 20000 }] };
    gpu.initial.locks[0].value = 900000;
    gpu.restoreCurrent = { locks: structuredClone(gpu.current.locks), offsets: [structuredClone(gpu.current.devices[0].offsets)] };
    gpu.restoreRevision = "gpu-restore-0";
    h.current = { enabled: true, actions: ["gpuTurbo"], osd: true }; h.authorized = true; h.status.running = true; h.revision = "with-gpu-baseline";
    window.__uiEmit("hardware-controls-changed", null);
  });
  await details(page);
  await expect(facts(page)).toContainText("恢复接管前的值");
}
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与灯光"); });

test("授权前展示当前显卡与全部模式目标，选择范围不写入", async ({ page }) => {
  await edit(page);
  await expect(scope(page)).not.toBeChecked();
  await expect(facts(page)).toHaveCount(0);
  await scope(page).check();
  await done(page);
  await details(page);
  await expect(facts(page)).toContainText("图形锁频：1700 MHz");
  await expect(facts(page)).toContainText("关闭：锁频 650 MHz · 偏移保持");
  await expect(facts(page)).toContainText("性能 · 电池锁频保持 · GPU 1 核心 0 / 显存 0 MHz");
  await expect(facts(page)).toContainText("静音增强锁频保持 · 偏移保持");
  await facts(page).getByRole("button", { name: "锁频与恢复说明", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "锁频与恢复说明", exact: true })).toContainText("会同时解除第二个频率域");
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
  expect(await calls(page, "restore_gpu_turbo")).toEqual([]);
});

test("GPU Turbo 独立预览并授权，不继承风扇或电源模式范围", async ({ page }) => {
  await edit(page);
  await editor(page).getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  for (const label of ["风扇模式", "电源模式循环"]) {
    const input = editor(page).getByRole("checkbox", { name: label, exact: true });
    await input.check(); await expect(scope(page)).not.toBeChecked(); await input.uncheck();
  }
  await scope(page).check();
  await done(page);
  const review = panel(page).getByRole("region", { name: "原厂热键与屏显 · 保存热键接管范围", exact: true });
  await expect(review).toContainText("原厂 GPU Turbo");
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
  await review.getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  expect((await calls(page, "apply_hotkeys"))[0].args.request.settings.actions).toEqual(["gpuTurbo"]);
});

test("某个模式超过驱动范围时在授权前展示具体原因", async ({ page }) => {
  await edit(page);
  await scope(page).check();
  await done(page);
  await details(page);
  await expect(facts(page)).toBeVisible();
  await page.evaluate(() => {
    const hotkeys = window.__uiFixture.hotkeys;
    hotkeys.facts.gpuTurbo.current.onTargets.find(item => item.mode === 2 && item.onAc).unavailableReason = "核心频率偏移超出驱动范围";
    hotkeys.revision = "gpu-range-changed"; window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(facts(page)).toContainText("不可执行：核心频率偏移超出驱动范围");
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
});

test("初始锁频和偏移预览后独立恢复，停止监听并保留策略", async ({ page }) => {
  await initial(page);
  await expect(facts(page)).toContainText("图形锁频：1700 MHz → 900 MHz");
  await expect(facts(page)).toContainText("核心 100 MHz → 10 MHz · 显存 -1 MHz → 20 MHz");
  const restore = await previewHotkeyRestore(page, "初始 GPU 状态");
  expect(await calls(page, "restore_gpu_turbo")).toEqual([]);
  await restore.getByRole("button", { name: "确认恢复", exact: true }).click();
  expect((await calls(page, "restore_gpu_turbo"))[0].args).toEqual({ expectedRevision: "gpu-restore-0" });
  await openHotkeyDetails(page);
  await expect(facts(page)).toContainText("图形锁频：900 MHz");
  await page.keyboard.press("Escape");
  await expect(panel(page).getByRole("status")).toContainText("已停止监听");
  await edit(page);
  await expect(scope(page)).toBeChecked();
});

test("来源失效仍可恢复已知基线；恢复失败保留目标并允许重试", async ({ page }) => {
  await initial(page);
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, gpu = h.facts.gpuTurbo;
    gpu.current = null; gpu.identity = null; gpu.error = "固件目标不可读";
    const cap = h.facts.capabilities.find(c => c.scope === "gpuTurbo"); cap.identity = null; cap.reason = gpu.error;
    h.revision = "gpu-source-failed"; window.__uiFixture.gpuTurboRestoreFault = "锁频恢复失败";
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(facts(page)).toContainText("固件目标不可读");
  for (let attempt = 0; attempt < 2; attempt++) {
    const restore = await previewHotkeyRestore(page, "初始 GPU 状态");
    await restore.getByRole("button", { name: "确认恢复", exact: true }).click();
    if (!attempt) await expect(await openHotkeyDetails(page)).toContainText("锁频恢复失败");
  }
  expect(await calls(page, "restore_gpu_turbo")).toHaveLength(2);
  await edit(page);
  await expect(scope(page)).toBeChecked();
  await scope(page).click();
  await expect(scope(page)).toHaveCount(0);
});

test("深浅宽窄卡片保留模块边框、间隔及内部左右布局", async ({ page }) => {
  await initial(page);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 900 }); await facts(page).scrollIntoViewIfNeeded();
      const box = await facts(page).evaluate(el => {
        const columns = el.querySelector(".ccc-gpu-turbo-columns"), parts = [...columns.children];
        const [left, right] = parts.map(p => p.getBoundingClientRect());
        return { overflow: el.scrollWidth > el.clientWidth + 1, right: el.getBoundingClientRect().right,
          borders: parts.every(p => parseFloat(getComputedStyle(p).borderTopWidth) > 0), gap: parseFloat(getComputedStyle(columns).gap),
          layout: el.clientWidth >= 380 ? right.left >= left.right - 1 : right.top >= left.bottom - 1 };
      });
      expect(box.overflow).toBe(false); expect(box.right).toBeLessThanOrEqual(width);
      expect(box.borders).toBe(true); expect(box.gap).toBeGreaterThan(0); expect(box.layout).toBe(true);
      if (width !== 760) await facts(page).screenshot({ path: `target/ui-check/gpu-turbo-${theme}-${width}.png` });
    }
  }
});
