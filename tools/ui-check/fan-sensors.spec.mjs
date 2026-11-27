import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";

test("各路实测转速共用环形刻度，自定义档从编辑入口应用曲线", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await page.evaluate(() => {
    const fan = window.__uiFixture.fan;
    window.__uiFixture.fanKind = "rpmTable";
    fan.rpmFans = ["cpu", "gpu1", "gpu2", "sys"].map(id => ({ id, maximumRpm: 6000,
      points: Array.from({ length: 10 }, (_, index) => ({ tempC: 30 + 7 * index, rpmPct: 28 + 8 * index })) }));
  });
  await openWorkspace(page, "性能与散热");
  const panel = page.getByRole("region", { name: "散热控制", exact: true });
  const readings = name => panel.getByRole("group", { name: `${name} 实况` });
  const meter = name => readings(name).getByRole("meter");
  const gauges = panel.locator(".ccc-fan-speed-grid");
  await expect(gauges.getByRole("group")).toHaveCount(4);
  await expect(meter("CPU")).toHaveAttribute("aria-valuetext", "1800 RPM");
  await expect(meter("CPU")).toHaveAttribute("aria-valuemax", "6");
  await expect(meter("CPU").locator("svg text")).toHaveText(["0", "1", "2", "3", "4", "5", "6", "1.8"]);
  const labelsFit = await meter("CPU").locator("svg").evaluate(svg => {
    const frame = svg.viewBox.baseVal;
    return [...svg.querySelectorAll("text")].every(label => {
      const bounds = label.getBBox();
      return bounds.x >= frame.x && bounds.x + bounds.width <= frame.x + frame.width &&
        bounds.y >= frame.y && bounds.y + bounds.height <= frame.y + frame.height;
    });
  });
  expect(labelsFit).toBe(true);
  await expect(panel.locator(".ccc-plot-draft")).toHaveCount(0);
  await page.evaluate(() => {
    const current = window.__uiHardwareSnapshot();
    for (let index = 0; index <= 60; index++) {
      const sample = structuredClone(current);
      sample.sampledAtMs -= (60 - index) * 5000;
      const variation = Math.round(Math.sin(index * Math.PI / 15) * 300);
      sample.fan.cpu.rpm = 1800 + variation;
      sample.fan.gpu1 = { tempC: 57, rpm: 1500 - variation, dutyPct: null };
      sample.fan.gpu2 = { tempC: 62, rpm: index === 30 ? 6200 : 4321 + variation, dutyPct: 45 };
      sample.fan.sys = { tempC: 37, rpm: 0, dutyPct: 0 };
      window.__uiEmit("hardware-snapshot", sample);
    }
  });
  for (const [name, temperature, value] of [["GPU 1", 57, 1.5], ["GPU 2", 62, 4.321], ["系统", 37, 0]]) {
    await expect(readings(name)).toContainText(`${temperature} °C`);
    await expect(meter(name)).toHaveAttribute("aria-valuenow", String(value));
    await expect(meter(name)).toHaveAttribute("aria-valuetext", `${Math.round(value * 1000)} RPM`);
    await expect(meter(name).locator(".ccc-fan-dial-value")).toHaveText(String(value));
    await expect(meter(name)).toHaveAttribute("aria-valuemax", "7");
  }
  await expect(meter("CPU").locator(".ccc-fan-dial-scale")).toHaveText(["0", "1", "2", "3", "4", "5", "6", "7"]);
  const gpuArc = meter("GPU 1").locator(".ccc-fan-dial-fill");
  await expect(gpuArc).toHaveCSS("stroke", "oklch(0.6 0.118 184.704)");
  expect(parseFloat(await gpuArc.getAttribute("stroke-dasharray"))).toBeCloseTo(1500 / 7000 * 100);
  await expect(meter("系统").locator(".ccc-fan-dial-fill")).toHaveCount(0);
  await expect(meter("系统").locator(".ccc-fan-dial-needle")).toHaveAttribute("transform", "rotate(-225 90 82)");
  await gauges.screenshot({ path: test.info().outputPath("fan-speed-gauges.png") });
  const rpmField = name => panel.getByRole("group", { name: `${name} 曲线点` }).getByRole("spinbutton", { name: "RPM 转速" });
  await expect(rpmField("GPU 2")).toHaveCount(0);
  const custom = panel.getByRole("radio", { name: "自定义", exact: true });
  const auto = panel.getByRole("radio", { name: "自动", exact: true });
  await expect(auto).toBeChecked();
  await panel.getByRole("button", { name: "编辑曲线", exact: true }).click();
  await expect(gauges).toHaveCount(0);
  await expect(panel.locator(".ccc-plot-draft")).toHaveCount(4);
  await expect(panel.locator('.ccc-curve-series[data-fan="gpu1"] .ccc-plot-draft')).toHaveCSS("stroke", "oklch(0.6 0.118 184.704)");
  await rpmField("GPU 2").fill("33");
  await rpmField("GPU 2").press("Enter");
  await expect(rpmField("CPU")).toHaveValue("28");
  await expect(panel.locator(".ccc-plot-current")).toHaveCount(1);
  await panel.getByRole("region", { name: "风扇实况与曲线" }).getByRole("button", { name: "应用", exact: true }).click();
  await expect(gauges).toBeVisible();
  await expect(auto).toBeChecked();
  const actualRpm = () => page.evaluate(() => window.__uiFixture.fan.rpmFans.find(fan => fan.id === "gpu2").points[0].rpmPct);
  expect(await actualRpm()).toBe(28);
  await custom.click();
  await expect(custom).toBeChecked();
  await expect.poll(actualRpm).toBe(33);
  const calls = await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(save_custom_fan_curves|set_fan_mode)$/.test(call.command)));
  expect(calls.map(call => call.command)).toEqual(["save_custom_fan_curves", "set_fan_mode"]);
  expect(calls[1].args.mode).toBe(6);
  expect(Object.keys(calls[0].args.changes.curves)).toEqual(["gpu2"]);

  await panel.getByRole("radio", { name: "自动", exact: true }).click();
  await expect(gauges).toBeVisible();
  await expect(meter("GPU 1")).toHaveAttribute("aria-valuenow", "1.5");
  await expect(rpmField("GPU 2")).toHaveCount(0);

  await page.evaluate(async () => {
    const sample = window.__uiHardwareSnapshot();
    sample.sampledAtMs += 100;
    window.__uiEmit("hardware-snapshot", sample);
  });
  for (const name of ["系统", "GPU 2"]) {
    const empty = readings(name).getByRole("img", { name: `${name} 转速未读取` });
    await expect(empty.locator(".ccc-fan-dial-value")).toHaveText("—");
    await expect(empty.locator(".ccc-fan-dial-needle")).toHaveCount(0);
    await expect(readings(name).locator("dd")).toHaveText("—");
  }
});

test("点击两条曲线分别高亮和聚焦点位，风扇名称支持键盘选择", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await page.evaluate(() => {
    window.__uiFixture.fan.currentMode = 6;
    window.__uiFixture.fan.fans.push({ id: "gpu1", present: true, t1: 38, d1Pct: 18,
      t2: 50, d2Pct: 30, t3: 78, d3Pct: 48 });
  });
  await openWorkspace(page, "性能与散热");
  const workspace = page.getByRole("region", { name: "风扇实况与曲线" });
  await page.getByRole("button", { name: "编辑曲线", exact: true }).click();
  const plot = workspace.locator(".ccc-plot");
  const fields = name => workspace.getByRole("group", { name: `${name} 曲线点` });
  const legend = name => workspace.getByRole("button", { name: `调节 ${name} 风扇曲线` });
  for (const [name, duty] of [["CPU", 50 + 49 / 3], ["GPU 1", 30 + 18 * 20 / 28]]) {
    await plot.scrollIntoViewIfNeeded();
    const box = await plot.boundingBox();
    await page.mouse.click(box.x + box.width * 40 / 70, box.y + box.height * (1 - duty / 100));
    await expect(legend(name)).toHaveAttribute("aria-pressed", "true");
    await expect(fields(name)).toBeFocused();
    await expect(fields(name)).toHaveAttribute("data-selected", "true");
    await expect(workspace.getByRole("group", { name: `${name} 风扇曲线` }).locator(".ccc-plot-draft")).toHaveCSS("stroke-width", "3px");
  }
  await expect(workspace.locator(".ccc-plot-draft:visible")).toHaveCount(2);
  await workspace.screenshot({ path: "target/ui-check/fan-curve-selection.png" });
  await legend("CPU").press("Enter");
  await expect(fields("CPU")).toBeFocused();
  await expect(legend("GPU 1")).toHaveAttribute("aria-pressed", "false");
  await page.keyboard.press("Tab");
  await expect(fields("CPU").getByRole("spinbutton", { name: "T2 温度" })).toBeFocused();
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => /^apply_(fan|rpm_fan)/.test(call.command)))).toEqual([]);
});

test("设备拒绝自定义档切换时保留实测表盘", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await openWorkspace(page, "性能与散热");
  const panel = page.getByRole("region", { name: "散热控制" });
  await page.evaluate(() => { window.__uiFixture.fanWriteFault = "set_fan_mode"; });
  await panel.getByRole("radio", { name: "自定义", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("隔离风扇写入失败");
  await expect(panel.getByRole("radio", { name: "自动", exact: true })).toBeChecked();
  await expect(panel.getByRole("heading", { name: "风扇转速", exact: true })).toBeVisible();
  await expect(panel.getByRole("spinbutton", { name: "T2 风扇强度" })).toHaveCount(0);
});
