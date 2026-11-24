import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace, openPowerArea } from "./workspace.mjs";

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
});

async function expectPlotScale(frame, temperatures, measure) {
  await expect(frame.locator(".ccc-plot-measure")).toHaveText(measure);
  await expect(frame.locator(".ccc-plot-unit")).toHaveText("温度 °C");
  await expect(frame.locator(".ccc-plot-axis > span")).toHaveText(temperatures.map(String));
  await expect(frame.locator(".ccc-plot-ylabels > span")).toHaveText(["100%", "80%", "60%", "40%", "20%", "0%"]);
  const alignment = await frame.evaluate(element => {
    const x = [...element.querySelectorAll(".ccc-plot-axis > span")].map(label => label.getBoundingClientRect());
    const y = [...element.querySelectorAll(".ccc-plot-ylabels > span")].map(label => label.getBoundingClientRect());
    const lines = [...element.querySelectorAll(".ccc-plot-grid line")];
    const offsets = lines.map((line, index) => {
      const point = new DOMPoint(Number(line.getAttribute("x1")), Number(line.getAttribute("y1"))).matrixTransform(line.getScreenCTM());
      if (index < x.length) return Math.abs(x[index].left + x[index].width / 2 - point.x);
      const label = y[index - x.length];
      return Math.abs(label.top + label.height / 2 - point.y);
    });
    return {
      offsets,
      tickOverlap: x.slice(1).some((label, index) => label.left < x[index].right),
    };
  });
  expect(alignment.offsets).toHaveLength(temperatures.length + 6);
  expect(Math.max(...alignment.offsets)).toBeLessThan(1);
  expect(alignment.tickOverlap).toBe(false);
}

async function startDrag(page, frame, handle, temperature, percentage) {
  await handle.scrollIntoViewIfNeeded();
  const origin = await handle.boundingBox();
  await page.mouse.move(origin.x + origin.width / 2, origin.y + origin.height / 2);
  await page.mouse.down();
  const target = await frame.locator(".ccc-plot").evaluate((plot, { temperature, percentage }) => {
    const bounds = plot.getBoundingClientRect();
    const domain = plot.querySelector("svg").viewBox.baseVal;
    return { x: bounds.left + (temperature - domain.x) / domain.width * bounds.width,
      y: bounds.top + (1 - percentage / 100) * bounds.height };
  }, { temperature, percentage });
  await page.mouse.move(target.x, target.y);
}

async function expectNoFanWrites(page) {
  const { calls, faults } = await page.evaluate(() => window.__uiFixture);
  expect(calls.filter(({ command }) => /^(apply_|set_fan|restore_fan)/.test(command))).toEqual([]);
  expect(faults).toEqual([]);
}

const fanWrites = (page) => page.evaluate(() => window.__uiFixture.calls.filter(({ command }) =>
  /^(apply_(?:fan|rpm_fan)|set_fan|restore_fan)/.test(command)));

test("固件曲线跟随指针和键盘，松手或释放方向键只写一次", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1050 });
  await page.evaluate(() => {
    const { fan } = window.__uiFixture;
    fan.availableModes.push(8);
    fan.offsetPct = 30;
    Object.assign(fan.fans[0], { t1: 51, d1Pct: 23, t2: 60, d2Pct: 32, t3: 80, d3Pct: 33 });
    fan.fans.push({ ...fan.fans[0], id: "gpu1", t1: 15 });
  });
  await openWorkspace(page, "性能与散热");
  const panel = page.getByRole("region", { name: "风扇设置", exact: true });
  await expect(panel).toHaveCount(1);
  await expect(panel.getByRole("radiogroup", { name: "风扇曲线执行方式", exact: true })).toHaveCount(0);
  await expect(panel.getByRole("radiogroup", { name: "风扇模式", exact: true })).toHaveCount(1);
  const frame = page.locator(".ccc-plot-frame:visible");
  await expectPlotScale(frame, [30, 40, 60, 80, 100], "占空比");
  await expect(frame.locator(".ccc-plot-current")).toHaveCount(0);
  await expect(frame.locator(".ccc-plot-area")).toHaveAttribute("d", /^M51,77 .*L100,100 H51 Z$/);
  const settings = panel.locator(".ccc-fan-settings");
  const fields = panel.locator(".ccc-fan-fields");
  const readings = panel.locator(".ccc-fan-current");
  await expect(readings).toContainText("1800 RPM");
  await expect(panel.getByRole("tablist", { name: "选择风扇" })).toBeVisible();
  await expect(settings.getByRole("button", { name: "恢复设置", exact: true })).toHaveCount(1);
  const plotBox = await frame.boundingBox();
  const settingsBox = await settings.boundingBox();
  const readingsBox = await readings.boundingBox();
  expect(readingsBox.y + readingsBox.height).toBeLessThan(plotBox.y);
  expect((await fields.boundingBox()).x).toBeGreaterThan(plotBox.x + plotBox.width);
  expect(settingsBox.y).toBeGreaterThan(plotBox.y + plotBox.height);
  await panel.screenshot({ path: "target/ui-check/fan-settings-redesign-light.png" });

  const handle = frame.getByRole("slider", { name: /^T3\/D3 可调点/ });
  const duty = page.getByRole("spinbutton", { name: "T3 占空比" });
  const current = page.getByRole("status", { name: "当前控制", exact: true });
  await handle.click();
  await startDrag(page, frame, handle, 80, 60);
  await expect(duty).toHaveValue("60");
  await expect(fields.locator("[data-selected=true]")).toContainText("T3");
  await expect(frame.locator(".ccc-plot-current")).toHaveCount(1);
  await expect(current).toHaveText("固件 · 自动");
  await expectNoFanWrites(page);
  await page.mouse.up();
  await expect(current).toHaveText("固件 · 自定义");
  await expect(duty).toHaveValue("60");
  await expect(frame.locator(".ccc-plot-current")).toHaveCount(0);
  expect(await fanWrites(page)).toEqual([{ command: "apply_fan_curve", args: {
    cpu: { t2: 60, d2Pct: 32, t3: 80, d3Pct: 60 }, gpu1: null, gpu2: null,
  } }]);

  await expect(handle).toBeEnabled();
  await handle.focus();
  await page.keyboard.down("ArrowUp");
  await page.keyboard.down("ArrowUp");
  await expect(duty).toHaveValue("62");
  expect(await fanWrites(page)).toHaveLength(1);
  await page.keyboard.up("ArrowUp");
  await expect.poll(() => fanWrites(page)).toHaveLength(2);
  await expect(duty).toHaveValue("62");
  await expect(handle).toBeEnabled();

  await startDrag(page, frame, handle, 85, 75);
  await expect(duty).toHaveValue("75");
  await handle.dispatchEvent("pointercancel", { pointerId: 1, pointerType: "mouse", isPrimary: true, bubbles: true });
  await page.mouse.up();
  await expect(duty).toHaveValue("62");
  expect(await fanWrites(page)).toHaveLength(2);
  await expect(page.getByRole("heading", { name: "待应用变更", exact: true })).toHaveCount(0);
  await expect(page.getByRole("button", { name: /确认应用|丢弃草稿/ })).toHaveCount(0);
  await page.evaluate(async () => window.__uiEmit("hardware-snapshot", await window.__TAURI_INTERNALS__.invoke("read_hardware_snapshot")));
  await expect(readings).toContainText("1800 RPM");
  await page.screenshot({ path: "target/ui-check/fan-settings-single.png" });

  await panel.getByRole("tab", { name: "GPU 1", exact: true }).click();
  await expectPlotScale(frame, [10, 20, 40, 60, 80, 100], "占空比");
  await expect(frame.locator(".ccc-plot-draft")).toHaveAttribute("d", /^M15,/);
  await panel.getByRole("tab", { name: "GPU 1", exact: true }).press("ArrowLeft");
  const cpu = panel.getByRole("tab", { name: "CPU", exact: true });
  await expect(cpu).toBeFocused();
  await expect(panel.getByRole("tabpanel", { name: "GPU 1" })).toBeVisible();
  await cpu.press("Enter");
  await expect(panel.getByRole("tabpanel", { name: "CPU" })).toBeVisible();
  expect(await fanWrites(page)).toHaveLength(2);
});

test("点位数字回车或失焦即应用，取消与空值不会提交", async ({ page }) => {
  await openWorkspace(page, "性能与散热");
  const input = page.getByRole("spinbutton", { name: "T2 占空比" });
  await input.fill("62");
  await expect(page.locator('.ccc-plot-handle[data-point="p2"]:visible')).toHaveClass(/is-selected/);
  await expect(page.locator(".ccc-plot-draft:visible")).toHaveAttribute("d", /L60,38/);
  await input.press("Escape");
  await input.blur();
  await expect(input).toHaveValue("50");
  await expectNoFanWrites(page);

  await input.fill("62");
  await input.press("Enter");
  await expect.poll(() => fanWrites(page)).toEqual([{ command: "apply_fan_curve", args: {
    cpu: { t2: 60, d2Pct: 62, t3: 90, d3Pct: 99 }, gpu1: null, gpu2: null,
  } }]);
  await expect(input).toHaveValue("62");
  await expect(input).toBeEnabled();
  await input.fill("");
  await input.blur();
  await expect(input).toHaveValue("62");
  expect(await fanWrites(page)).toHaveLength(1);
  await input.fill("70");
  await input.blur();
  await expect.poll(() => fanWrites(page)).toHaveLength(2);
  await expect(input).toHaveValue("70");
  await expect(input).toBeEnabled();
  await page.getByRole("button", { name: "恢复设置", exact: true }).click();
  await page.getByRole("button", { name: "恢复风扇设置", exact: true }).click();
  const restore = page.getByRole("dialog", { name: "恢复风扇设置", exact: true });
  await expect(restore).toContainText("恢复首次调整前");
  await restore.getByRole("button", { name: "取消", exact: true }).click();
  expect(await fanWrites(page)).toHaveLength(2);
});

test("偏移滑杆拖动和方向键连按只在释放后写入一次", async ({ page }) => {
  await openWorkspace(page, "性能与散热");
  const slider = page.getByRole("slider", { name: "加速偏移", exact: true });
  await slider.scrollIntoViewIfNeeded();
  const box = await slider.boundingBox();
  const y = box.y + box.height / 2;
  await page.mouse.move(box.x + box.width * 0.2, y);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.6, y, { steps: 6 });
  const target = Number(await slider.inputValue());
  expect(target).toBeGreaterThan(45);
  expect(target).toBeLessThan(75);
  await expectNoFanWrites(page);
  await page.mouse.up();
  await expect.poll(() => fanWrites(page)).toEqual([{ command: "set_fan_offset", args: { expected: { mode: 0, slot: null }, pct: target } }]);
  await expect(slider).toHaveValue(String(target));
  await expect(slider).toBeEnabled();

  await slider.focus();
  await page.keyboard.down("ArrowRight");
  await page.keyboard.down("ArrowRight");
  await expect(slider).toHaveValue(String(target + 2));
  expect(await fanWrites(page)).toHaveLength(1);
  await page.keyboard.up("ArrowRight");
  await expect.poll(() => fanWrites(page)).toHaveLength(2);
  expect((await fanWrites(page)).at(-1)).toEqual({ command: "set_fan_offset", args: { expected: { mode: 0, slot: null }, pct: target + 2 } });
  await expect(slider).toHaveValue(String(target + 2));
});

test("加速偏移可在自动和寂静模式调节，提交偏移不切换档位", async ({ page }) => {
  await page.evaluate(() => {
    const { fan } = window.__uiFixture;
    fan.availableModes.push(5, 8);
    fan.offsetPct = 50;
  });
  await openWorkspace(page, "性能与散热");
  const fan = page.getByRole("region", { name: "风扇设置", exact: true });
  const slider = fan.getByRole("slider", { name: "加速偏移", exact: true });
  const quiet = fan.getByRole("radio", { name: "寂静模式", exact: true });
  const modeOnly = "仅自动或寂静模式可调节加速偏移。";
  await expect(slider).toBeEnabled();
  await quiet.click();
  await expect(quiet).toBeChecked();
  await expect(slider).toBeEnabled();
  await expect(fan).not.toContainText(modeOnly);
  await expect(slider).toHaveValue("50");

  const quietWrite = { command: "set_fan_mode", args: { mode: 8 } };
  await slider.focus();
  await page.keyboard.down("ArrowRight");
  await expect(slider).toHaveValue("51");
  expect(await fanWrites(page)).toEqual([quietWrite]);
  await page.keyboard.up("ArrowRight");
  const writes = [quietWrite, { command: "set_fan_offset", args: { expected: { mode: 8, slot: null }, pct: 51 } }];
  await expect.poll(() => fanWrites(page)).toEqual(writes);
  await expect(slider).toBeEnabled();
  await expect(slider).toHaveValue("51");
  await expect(quiet).toBeChecked();

  for (const name of ["全速", "能效优先", "自定义"]) {
    const mode = fan.getByRole("radio", { name, exact: true });
    await mode.click();
    await expect(mode).toBeChecked();
    await expect(slider).toBeDisabled();
    await expect(fan).toContainText(modeOnly);
  }
  await fan.getByRole("radio", { name: "自动", exact: true }).click();
  await expect(slider).toBeEnabled();
  await expect(fan).not.toContainText(modeOnly);
  await page.evaluate(() => {
    window.__uiFixture.fan.offsetBlockedReason = "此机型不支持风扇加速偏移。";
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(slider).toBeDisabled();
  await expect(fan).toContainText("此机型不支持风扇加速偏移");
  await quiet.click();
  await expect(quiet).toBeChecked();
  await expect(slider).toBeDisabled();
  expect((await fanWrites(page)).filter(({ command }) => command === "set_fan_offset")).toEqual([writes[1]]);
});

test("娱乐档寂静覆盖显示有效档位，保存偏移时绑定娱乐记忆槽", async ({ page }) => {
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.fan.availableModes.push(8);
    data.fan.offsetPct = 20;
    data.fanRemembersOffset = true;
    data.performanceOptions.currentMode = 3;
    data.entertainmentQuiet = true;
    data.fan.currentMode = 8;
  });
  await openWorkspace(page, "性能与散热");
  const fan = page.getByRole("region", { name: "风扇设置", exact: true });
  await expect(fan.getByRole("status", { name: "当前控制" })).toHaveText("固件 · 寂静模式");
  await expect(fan.getByRole("radio", { name: "寂静模式", exact: true })).toBeChecked();
  const slider = fan.getByRole("slider", { name: "加速偏移", exact: true });
  await expect(slider).toBeEnabled();
  await slider.press("ArrowRight");
  await expect.poll(() => fanWrites(page)).toEqual([
    { command: "set_fan_offset", args: { expected: { mode: 8, slot: 3 }, pct: 21 } },
  ]);
  await expect(slider).toHaveValue("21");
  await expect(fan.getByRole("status", { name: "当前控制" })).toHaveText("固件 · 寂静模式");
  expect(await page.evaluate(() => window.__uiFixture.fan.currentMode)).toBe(8);
});

test("偏移提交遇到电源档切换时回读原值，重新调整使用新记忆槽", async ({ page }) => {
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.fan.offsetPct = 20;
    data.fanRemembersOffset = true;
    const ipc = window.__TAURI_INTERNALS__;
    const invoke = ipc.invoke;
    ipc.invoke = (command, args, ...rest) => {
      if (command === "set_fan_offset") data.performanceOptions.currentMode = 3;
      return invoke(command, args, ...rest);
    };
  });
  await openWorkspace(page, "性能与散热");
  const fan = page.getByRole("region", { name: "风扇设置", exact: true });
  const slider = fan.getByRole("slider", { name: "加速偏移", exact: true });
  await expect(slider).toBeEnabled();
  await slider.press("ArrowRight");
  await expect(fan.getByRole("alert")).toContainText("风扇偏移记忆槽已变化");
  await expect(slider).toHaveValue("20");
  await expect(slider).toBeEnabled();
  await slider.press("ArrowRight");
  await expect.poll(() => fanWrites(page)).toEqual([
    { command: "set_fan_offset", args: { expected: { mode: 0, slot: 2 }, pct: 21 } },
    { command: "set_fan_offset", args: { expected: { mode: 0, slot: 3 }, pct: 21 } },
  ]);
  await expect(slider).toHaveValue("21");
  await expect(fan.getByRole("alert")).toHaveCount(0);
});

test("直接应用复核设备状态，写入失败后恢复实读值", async ({ page }) => {
  await openWorkspace(page, "性能与散热");
  const fullSpeed = page.getByRole("radio", { name: "全速", exact: true });
  await page.evaluate(() => { window.__uiFixture.fan.currentMode = 6; });
  await fullSpeed.click();
  await expect(page.getByRole("alert")).toContainText("风扇设置已变化");
  await expect(page.getByRole("status", { name: "当前控制" })).toHaveText("固件 · 自定义");
  expect(await fanWrites(page)).toEqual([]);
  await page.evaluate(() => { window.__uiFixture.fanWriteFault = "set_fan_mode"; });
  await fullSpeed.click();
  await expect(page.getByRole("alert")).toContainText("隔离风扇写入失败");
  expect(await fanWrites(page)).toEqual([{ command: "set_fan_mode", args: { mode: 1 } }]);
  await expect(fullSpeed).toBeEnabled();
  await expect(fullSpeed).not.toBeChecked();
  await page.evaluate(() => { window.__uiFixture.fanWriteFault = "apply_fan_curve"; });
  const input = page.getByRole("spinbutton", { name: "T2 占空比" });
  await input.fill("101");
  await input.press("Enter");
  await expect.poll(() => fanWrites(page)).toHaveLength(2);
  await expect(input).toBeEnabled();
  await expect(input).toHaveValue("50");
  await expect(page.getByRole("alert")).toContainText("隔离风扇写入失败");
});

test("RPM 自定义点选直接使用原表并保留停转级", async ({ page }) => {
  await page.evaluate(() => {
    const fixture = window.__uiFixture;
    fixture.fanKind = "rpmTable";
    fixture.fan.rpmFans = [{ id: "cpu", maximumRpm: 5000,
      points: Array.from({ length: 10 }, (_, i) => ({ tempC: 40 + i * 5, rpmPct: i === 9 ? 100 : i * 10 })) }];
  });
  await openWorkspace(page, "性能与散热");
  const first = page.getByRole("spinbutton", { name: "RPM 转速" });
  await expect(first).toHaveValue("0");
  await page.getByRole("radio", { name: "自定义", exact: true }).click();
  await expect(first).toHaveValue("0");
  await expect(page.getByRole("status", { name: "当前控制" })).toHaveText("固件 · 自定义");
  await expect(page.getByRole("heading", { name: "待应用变更", exact: true })).toHaveCount(0);
  expect((await fanWrites(page)).map(item => item.command)).toEqual(["set_fan_mode"]);
});

test("软件曲线档案可应用，从同一个风扇设置交还固件控制", async ({ page }) => {
  await page.evaluate(() => {
    const { settings } = window.__uiFixture.profile;
    settings.fanMode = null;
    settings.fanSoftwareCurves = { cpu: { points: [{ tempC: 40, dutyPct: 30 }, { tempC: 95, dutyPct: 100 }],
      hysteresisC: 4, rampPctPerS: 10 } };
  });
  await openPowerArea(page, "配置档案");
  await page.getByRole("region", { name: "档案预览", exact: true }).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__uiFixture.fan.software?.cpu?.hysteresisC)).toBe(4);
  await openWorkspace(page, "性能与散热");
  const panel = page.getByRole("region", { name: "风扇设置", exact: true });
  const current = page.getByRole("status", { name: "当前控制", exact: true });
  await expect(current).toHaveText("软件接管");
  await expect(panel.getByRole("radio", { name: "软件曲线", exact: true })).toHaveCount(0);
  const offset = page.getByRole("slider", { name: "加速偏移", exact: true });
  await expect(offset).toBeDisabled();
  await expect(page.getByRole("spinbutton", { name: "T2 温度", exact: true })).toBeDisabled();
  expect(await fanWrites(page)).toEqual([]);
  await page.getByRole("radio", { name: "自动", exact: true }).click();
  await expect(current).toHaveText("固件 · 自动");
  await expect(offset).toBeEnabled();
  expect(await fanWrites(page)).toEqual([{ command: "set_fan_mode", args: { mode: 0 } }]);
  expect(await page.evaluate(() => window.__uiFixture.profile.settings.fanSoftwareCurves.cpu.hysteresisC)).toBe(4);
});

test("窄窗深色下单一风扇设置展示十级 RPM 图及固定转速端点", async ({ page }) => {
  await page.setViewportSize({ width: 360, height: 720 });
  await page.evaluate(() => {
    document.documentElement.dataset.theme = "dark";
    window.__uiFixture.fanKind = "rpmTable";
  });
  await openWorkspace(page, "性能与散热");
  const frame = page.locator(".ccc-plot-frame:visible");
  await expectPlotScale(frame, [0, 20, 40, 60, 80, 100], "目标转速");
  const settingsBox = await page.locator(".ccc-fan-settings:visible").boundingBox();
  const plotBox = await frame.boundingBox();
  const pointBox = await page.locator(".ccc-fan-fields:visible").boundingBox();
  const readingsBox = await page.locator(".ccc-fan-current:visible").boundingBox();
  expect(readingsBox.y + readingsBox.height).toBeLessThan(plotBox.y);
  expect(plotBox.y + plotBox.height).toBeLessThan(pointBox.y);
  expect(pointBox.y + pointBox.height).toBeLessThan(settingsBox.y);
  await expect(frame.locator(".ccc-plot-draft")).toHaveAttribute("d", /^M0,80 H40 H45 V72.*H100$/);
  await expect(frame.locator(".ccc-plot-area")).toHaveAttribute("d", /^M0,80 .*H100 L100,100 H0 Z$/);
  await expect(page.locator(".ccc-plot-key:visible")).toContainText("100% = 5000 RPM");
  await page.screenshot({ path: "target/ui-check/fan-grid-rpm-dark-360.png" });

  const rpmPoint = frame.getByRole("slider", { name: "第 1 级温度与转速", exact: true });
  await startDrag(page, frame, rpmPoint, 20, 20);
  await expect(rpmPoint).toHaveAttribute("aria-valuetext", "20°C，最大转速的 20%");
  await expect(page.getByRole("spinbutton", { name: "RPM 温度" })).toHaveValue("20");
  await expectNoFanWrites(page);
  await page.mouse.up();
  await expect(page.getByRole("status", { name: "当前控制", exact: true })).toHaveText("固件 · 自定义");
  await expect(rpmPoint).toBeEnabled();
  const last = frame.getByRole("slider", { name: "第 10 级温度与转速", exact: true });
  await expect(last).toHaveAttribute("aria-valuetext", "85°C，最大转速的 100%");
  expect((await fanWrites(page)).map(item => item.command)).toEqual(["apply_rpm_fan_curves"]);
});
