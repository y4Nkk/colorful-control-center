import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const dialog = page => page.getByRole("dialog", { name: "GPU 硬件参数", exact: true });
const fact = (page, label) => dialog(page).locator(".ccc-fact-row")
  .filter({ has: page.getByText(label, { exact: true }) }).locator("dd");
const calls = page => page.evaluate(() =>
  window.__uiFixture.calls.filter(call => call.command === "read_gpu_details").map(call => call.args.connectionId));

test.beforeEach(async ({ page }) => {
  await page.clock.install();
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await expect(page.getByText("Isolated GPU", { exact: true })).toBeVisible();
});

test("GPU 扩展字段仅随详情轮询，容量占用率和可用零值正确显示", async ({ page }) => {
  expect(await calls(page)).toEqual([]);
  await page.getByRole("button", { name: "GPU 规格", exact: true }).click();
  await expect(fact(page, "显存容量占用率")).toHaveText("12.5 %");
  await expect(fact(page, "视频编码占用率")).toHaveText("0 %");
  await expect(fact(page, "视频解码占用率")).toHaveText("34 %");
  await expect(fact(page, "性能状态")).toHaveText("P3");
  await expect(fact(page, "当前链路")).toHaveText("PCIe 3.0 ×8");
  await expect(fact(page, "限频原因")).toHaveText("功耗限制");
  await expect(fact(page, "显存温度")).toHaveCount(0);
  await expect(fact(page, "GPU 目标风扇转速")).toHaveCount(0);
  expect(await calls(page)).toEqual([1]);

  await page.clock.fastForward(5_100);
  await expect.poll(() => calls(page)).toEqual([1, 1]);
  await dialog(page).getByRole("button", { name: "关闭弹窗", exact: true }).click();
  await page.clock.fastForward(15_100);
  expect(await calls(page)).toEqual([1, 1]);
});

test("独显暂停丢弃迟到结果，恢复后只读新连接，隐藏窗口停止轮询", async ({ page }) => {
  await page.evaluate(() => {
    const actual = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      const value = await actual(command, args);
      if (command !== "read_gpu_details" || args.connectionId !== 1) return value;
      return new Promise(resolve => { window.finishGpuRead = () => resolve(value); });
    };
  });
  await page.getByRole("button", { name: "GPU 规格", exact: true }).click();
  await expect.poll(() => page.evaluate(() => typeof window.finishGpuRead)).toBe("function");

  await page.evaluate(() => {
    window.__uiFixture.gpuPower.devices[0].enabled = false;
    window.__uiEmit("hardware-snapshot", window.__uiHardwareSnapshot());
    window.finishGpuRead();
  });
  await expect(fact(page, "视频解码占用率")).toHaveCount(0);
  await page.clock.fastForward(5_100);
  expect(await calls(page)).toEqual([1]);

  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.gpuPower.devices[0].enabled = true;
    data.gpuConnectionId = 2;
    data.gpuDetails = {
      encoderUtilizationPct: null, decoderUtilizationPct: 0, performanceState: null,
      pcieLinkGeneration: null, pcieLinkWidth: null, pcieMaxLinkGeneration: null, pcieMaxLinkWidth: null,
      memoryTemperatureC: 58, fanTargetRpm: 0, clockLimitReasons: [],
    };
    window.__uiEmit("hardware-snapshot", window.__uiHardwareSnapshot());
  });
  await expect(fact(page, "视频解码占用率")).toHaveText("0 %");
  await expect(fact(page, "显存温度")).toHaveText("58 °C");
  await expect(fact(page, "GPU 目标风扇转速")).toHaveText("0 RPM");
  await expect(fact(page, "限频原因")).toHaveText("无");
  expect(await calls(page)).toEqual([1, 2]);
  await page.evaluate(() => {
    Object.defineProperty(document, "visibilityState", { configurable: true, value: "hidden" });
    document.dispatchEvent(new Event("visibilitychange"));
  });
  await expect(fact(page, "显存温度")).toHaveCount(0);
  await page.clock.fastForward(10_100);
  expect(await calls(page)).toEqual([1, 2]);
});
