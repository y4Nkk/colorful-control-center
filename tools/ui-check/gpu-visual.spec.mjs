import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const power = page => page.getByRole("region", { name: "独显开关", exact: true });
const hotkeys = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const turbo = page => page.getByRole("region", { name: "原厂 GPU Turbo 事实与预览", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);

test("显卡SVG只装饰真实当前状态，修改草稿不改变当前图示", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => { Object.assign(window.__uiFixture.gpuPower.devices[0], { drivesDisplay: false, canDisable: true }); });
  await page.goto("/"); await openWorkspace(page, "显示与显卡");
  const visual = power(page).locator(".ccc-gpu-power-visual");
  await expect(visual).toHaveAttribute("data-enabled", "on");
  await expect(visual.locator("svg[aria-hidden='true'] [data-desktop-gpu]")).toHaveCount(1);
  await power(page).getByRole("radio", { name: "关闭独显", exact: true }).check();
  await expect(visual).toHaveAttribute("data-enabled", "on");
  await expect(visual).toContainText("开启");
  expect(await calls(page, "apply_gpu_power")).toHaveLength(0);
  await power(page).getByRole("button", { name: "应用独显状态", exact: true }).click();
  expect(await calls(page, "apply_gpu_power")).toHaveLength(0);
  await power(page).getByRole("button", { name: "确认关闭此独显并请求系统授权？", exact: true }).click();
  await expect(visual).toHaveAttribute("data-enabled", "off");
});

test("能力详情按需展示，取消草稿仍保留已有初始恢复入口", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "显示与显卡");
  await openWorkspace(page, "键盘与输入");
  const scope = hotkeys(page).getByRole("checkbox", { name: "原厂 GPU Turbo", exact: true });
  await expect(scope).not.toBeChecked();
  await expect(turbo(page)).toHaveCount(0);
  await scope.check();
  await expect(turbo(page)).toBeVisible();
  const explanation = hotkeys(page).getByText("按原厂事件、模式和供电切换 GPU 锁频及偏移；只对已确认显卡执行，并支持初始恢复。", { exact: false });
  await expect(explanation).not.toBeVisible();
  await hotkeys(page).getByText("已选动作说明", { exact: true }).click();
  await expect(explanation).toBeVisible();
  await page.evaluate(() => {
    const gpu = window.__uiFixture.hotkeys.facts.gpuTurbo;
    gpu.initial = { devices: [gpu.current.devices[0].id], locks: structuredClone(gpu.current.locks), offsets: [structuredClone(gpu.current.devices[0].offsets)] };
    gpu.restoreCurrent = { locks: structuredClone(gpu.current.locks), offsets: [structuredClone(gpu.current.devices[0].offsets)] };
    gpu.restoreRevision = "visual-initial";
    window.__uiFixture.hotkeys.revision += "-initial";
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(turbo(page).getByRole("button", { name: "恢复初始 GPU 状态", exact: true })).toBeEnabled();
  await scope.uncheck();
  await expect(turbo(page).getByRole("button", { name: "恢复初始 GPU 状态", exact: true })).toBeVisible();
  expect(await calls(page, "apply_hotkeys")).toHaveLength(0);
});

test("未知独显保持未知图示，不将空数据绘成关闭或零频率", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => { window.__uiFixture.gpuPower.devices = []; });
  await page.goto("/"); await openWorkspace(page, "显示与显卡");
  const visual = power(page).locator(".ccc-gpu-power-visual");
  await expect(visual).toHaveAttribute("data-enabled", "unknown");
  await expect(visual.getByText("未知", { exact: true })).toBeVisible();
  await expect(power(page).getByRole("button", { name: "应用独显状态", exact: true })).toHaveCount(0);
  expect(await calls(page, "apply_gpu_power")).toHaveLength(0);
});
