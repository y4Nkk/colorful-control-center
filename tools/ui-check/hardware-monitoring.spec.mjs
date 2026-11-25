import { expect, test } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";

const demands = page => page.evaluate(() => window.__uiFixture.calls
  .filter(call => call.command === "update_hardware_monitoring").map(call => call.args.active));

test("实时采样只在可见的监控页面订阅，监控页之间共享连接", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await expect.poll(() => demands(page)).toEqual([true]);
  await openWorkspace(page, "性能与散热");
  await expect(page.getByRole("region", { name: "风扇实况与曲线" })).toBeVisible();
  expect(await demands(page)).toEqual([true]);
  await openWorkspace(page, "设置");
  await expect.poll(() => demands(page)).toEqual([true, false]);
  await openWorkspace(page, "概览");
  await expect.poll(() => demands(page)).toEqual([true, false, true]);

  const visibility = hidden => {
    Object.defineProperty(document, "visibilityState", { configurable: true, value: hidden ? "hidden" : "visible" });
    document.dispatchEvent(new Event("visibilitychange"));
  };
  await page.evaluate(visibility, true);
  await expect.poll(() => demands(page)).toEqual([true, false, true, false]);
  await page.evaluate(visibility, false);
  await expect.poll(() => demands(page)).toEqual([true, false, true, false, true]);
  await expect(page.getByLabel("CPU 当前温度")).toHaveText("50°C");
});

test("样本停滞时重新订阅并恢复，恢复后只保留一个监听入口", async ({ page }) => {
  await page.clock.install();
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await expect(page.getByLabel("CPU 当前温度")).toHaveText("50°C");
  await page.clock.runFor(3100);
  await expect(page.getByText("读数停滞", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "重试", exact: true }).click();
  await expect.poll(() => demands(page)).toEqual([true, false, true]);
  await expect(page.getByText("读数停滞", { exact: true })).toHaveCount(0);
  const ipc = await page.evaluate(() => window.__uiFixture.calls.filter(call =>
    /plugin:event\|(listen|unlisten)/.test(call.command) && call.args.event === "hardware-snapshot"));
  const verbs = ipc.map(call => call.command).slice(-3);
  expect(verbs).toEqual(["plugin:event|listen", "plugin:event|unlisten", "plugin:event|listen"]);
});

test("首次采样未响应时显示重试，恢复后清除连接失败", async ({ page }) => {
  await page.clock.install();
  await page.addInitScript({ content: `${MOCK_IPC}; window.__uiFixture.pauseHardwareSnapshots = true;` });
  await page.goto("/");
  await expect.poll(() => demands(page)).toEqual([true]);
  await page.clock.runFor(3100);
  await expect(page.getByText("监控不可用", { exact: true })).toBeVisible();
  await page.evaluate(() => { window.__uiFixture.pauseHardwareSnapshots = false; });
  await page.getByRole("button", { name: "重试", exact: true }).click();
  await expect(page.getByLabel("CPU 当前温度")).toHaveText("50°C");
  await expect(page.getByText("监控不可用", { exact: true })).toHaveCount(0);
});
