import { hotkeyRestoreButton, openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const power = page => page.getByRole("region", { name: "独显电源", exact: true });
const hotkeys = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const turbo = page => page.getByRole("region", { name: "原厂 GPU Turbo 事实与预览", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);

test("独显图示等待实际回读，切换期间禁用重复操作", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => { Object.assign(window.__uiFixture.gpuPower.devices[0], { drivesDisplay: false, canDisable: true }); });
  await page.goto("/"); await openWorkspace(page, "设备配置");
  const visual = power(page).locator(".ccc-gpu-power-visual");
  await expect(visual).toHaveAttribute("data-enabled", "on");
  await expect(visual.locator("svg[aria-hidden='true'] [data-desktop-gpu]")).toHaveCount(1);
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === "apply_gpu_power") await new Promise(resolve => { window.__finishGpuWrite = resolve; });
      return invoke(command, args);
    };
  });
  const off = power(page).getByRole("radio", { name: "关闭独显", exact: true });
  await off.click();
  await expect(off).toBeDisabled();
  await expect(visual).toHaveAttribute("data-enabled", "on");
  await expect(visual.locator(".ccc-gpu-power-state")).toHaveCount(0);
  expect(await calls(page, "apply_gpu_power")).toHaveLength(0);
  await page.evaluate(() => window.__finishGpuWrite());
  await expect(visual).toHaveAttribute("data-enabled", "off");
  expect(await calls(page, "apply_gpu_power")).toHaveLength(1);
});

test("能力详情按需展示，取消草稿仍保留已有初始恢复入口", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "设备配置");
  await openWorkspace(page, "键盘与灯光");
  const openEditor = () => hotkeys(page).getByRole("button", { name: "配置热键", exact: true }).click();
  const editor = page.getByRole("dialog", { name: "配置热键与屏显", exact: true });
  const done = () => editor.getByRole("button", { name: "完成编辑", exact: true }).click();
  const openDetails = () => hotkeys(page).getByRole("button", { name: "详情", exact: true }).click();
  await openEditor();
  const scope = editor.getByRole("checkbox", { name: "原厂 GPU Turbo", exact: true });
  await expect(scope).not.toBeChecked();
  await expect(turbo(page)).toHaveCount(0);
  await scope.check();
  await done();
  await openDetails();
  await expect(turbo(page)).toBeVisible();
  await page.evaluate(() => {
    const gpu = window.__uiFixture.hotkeys.facts.gpuTurbo;
    gpu.initial = { devices: [gpu.current.devices[0].id], locks: structuredClone(gpu.current.locks), offsets: [structuredClone(gpu.current.devices[0].offsets)] };
    gpu.restoreCurrent = { locks: structuredClone(gpu.current.locks), offsets: [structuredClone(gpu.current.devices[0].offsets)] };
    gpu.restoreRevision = "visual-initial";
    window.__uiFixture.hotkeys.revision += "-initial";
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(turbo(page)).toContainText("恢复接管前的值");
  await page.keyboard.press("Escape");
  await expect(hotkeyRestoreButton(page)).toBeEnabled();
  await openEditor();
  await scope.uncheck();
  await done();
  await openDetails();
  await expect(turbo(page)).toContainText("恢复接管前的值");
  await page.keyboard.press("Escape");
  await expect(hotkeyRestoreButton(page)).toBeVisible();
  expect(await calls(page, "apply_hotkeys")).toHaveLength(0);
});

test("未知独显保持未知图示，不将空数据绘成关闭或零频率", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => { window.__uiFixture.gpuPower.devices = []; });
  await page.goto("/"); await openWorkspace(page, "设备配置");
  const visual = power(page).locator(".ccc-gpu-power-visual");
  await expect(visual).toHaveAttribute("data-enabled", "unknown");
  await expect(visual.getByText("未知", { exact: true })).toBeVisible();
  await expect(power(page).getByRole("radiogroup", { name: "独显电源", exact: true })).toHaveCount(0);
  expect(await calls(page, "apply_gpu_power")).toHaveLength(0);
});
