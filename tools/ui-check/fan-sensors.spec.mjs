import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";

test("每个风扇页签读取各自的 EC 温度和转速，缺值保留空白", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await page.evaluate(() => {
    const fan = window.__uiFixture.fan;
    fan.curveKind = "rpmTable";
    fan.fans = [];
    fan.rpmFans = ["cpu", "gpu1", "gpu2", "sys"].map(id => ({ id, maximumRpm: 5000,
      points: Array.from({ length: 10 }, (_, index) => ({ tempC: 30 + 7 * index, rpmPct: 28 + 8 * index })) }));
  });
  await openWorkspace(page, "性能与散热");
  const panel = page.getByRole("region", { name: "散热控制", exact: true });
  const readings = panel.locator(".ccc-fan-current:visible");
  await expect(readings).toContainText("1800 RPM");
  await page.evaluate(async () => {
    const sample = window.__uiHardwareSnapshot();
    sample.sampledAtMs += 50;
    sample.fan.gpu1.tempC = 57;
    sample.fan.gpu2 = { tempC: 62, rpm: 4321, dutyPct: 45 };
    sample.fan.sys = { tempC: 37, rpm: 0, dutyPct: 0 };
    window.__uiEmit("hardware-snapshot", sample);
  });
  await panel.getByRole("tab", { name: "GPU 1", exact: true }).click();
  await expect(readings).toContainText("57 °C");
  await expect(readings).toContainText("1500 RPM");
  await panel.getByRole("tab", { name: "GPU 2", exact: true }).click();
  await expect(readings).toContainText("62 °C");
  await expect(readings).toContainText("4321 RPM");
  await panel.getByRole("tab", { name: "系统", exact: true }).click();
  await expect(readings).toContainText("37 °C");
  await expect(readings).toContainText("0 RPM");

  await page.evaluate(async () => {
    const sample = window.__uiHardwareSnapshot();
    sample.sampledAtMs += 100;
    window.__uiEmit("hardware-snapshot", sample);
  });
  await expect(readings.locator("dd")).toHaveText(["—", "—"]);
  await panel.getByRole("tab", { name: "GPU 2", exact: true }).click();
  await expect(readings.locator("dd")).toHaveText(["—", "—"]);
});
