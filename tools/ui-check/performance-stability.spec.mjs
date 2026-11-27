import { expect, test } from "@playwright/test";
import { availableAction } from "./configuration-review.mjs";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";

const panel = (page, name) => page.getByRole("region", { name, exact: true });
const slider = (page, name) => page.getByRole("slider", { name, exact: true });
const readCount = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command).length, command);

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await openWorkspace(page, "性能与散热");
});

async function holdReadback(page, write, read) {
  await page.evaluate(({ write, read }) => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    let hold = false;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (hold && command === read) {
        hold = false;
        await new Promise(resolve => { window.__releaseControlRead = resolve; });
      }
      const result = await invoke(command, args);
      if (command === write) hold = true;
      return result;
    };
  }, { write, read });
}

const waitForReadback = page => expect.poll(() => page.evaluate(() => typeof window.__releaseControlRead)).toBe("function");
const releaseReadback = page => page.evaluate(() => window.__releaseControlRead());

test("GPU 提交后保持滑杆位置和恢复按钮，直到设备回读", async ({ page }) => {
  const core = slider(page, "核心频率偏移");
  await expect(core).toBeVisible();
  await page.evaluate(() => {
    const fixture = window.__uiFixture;
    fixture.gpu = { ...fixture.gpu, baseline: { coreOffsetMhz: 0, memOffsetMhz: 0 } };
    window.__uiEmit("hardware-controls-changed", null);
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = (command, args) => {
      if (command === "apply_gpu_clock_offsets") {
        fixture.gpu = { ...fixture.gpu, coreOffsetMhz: args.coreMhz };
        return Promise.resolve({ ok: true, message: "隔离写入成功", rollbackError: null });
      }
      return invoke(command, args);
    };
  });
  const restore = availableAction(page, "GPU 频率", true);
  await expect(restore).toBeEnabled();
  await core.fill("50");
  const original = await restore.elementHandle();
  const restoreOffset = () => restore.evaluate(button => button.getBoundingClientRect().top - button.closest("section[aria-label='GPU 频率']").getBoundingClientRect().top);
  const before = await restoreOffset();
  await holdReadback(page, "apply_gpu_clock_offsets", "read_gpu_profile");
  await availableAction(page, "GPU 频率").click();
  await waitForReadback(page);
  await expect(core).toHaveValue("50");
  await expect(core).toBeDisabled();
  await expect(core.locator("..")).toHaveCSS("opacity", "1");
  await expect(restore).toBeDisabled();
  expect(await original.evaluate(node => node.isConnected)).toBe(true);
  expect(await restoreOffset()).toBeCloseTo(before);
  await releaseReadback(page);
  await expect(core).toBeEnabled();
  await expect(core).toHaveValue("50");
});

test("自动档打开曲线，切到全速时保留草稿，保存后仍是全速", async ({ page }) => {
  const fan = panel(page, "散热控制");
  const fullSpeed = fan.getByRole("radio", { name: "全速", exact: true });
  await expect(fan.getByRole("radio", { name: "自动", exact: true })).toBeChecked();
  const edit = fan.getByRole("button", { name: "编辑曲线", exact: true });
  const apply = fan.getByRole("region", { name: "风扇实况与曲线" }).getByRole("button", { name: "应用", exact: true });
  await edit.click();
  const handle = fan.getByRole("slider", { name: /^T2\/D2 可调点/ });
  const duty = fan.getByRole("spinbutton", { name: "T2 风扇强度", exact: true });
  await expect(duty).toHaveValue("50");
  await handle.focus();
  await page.keyboard.down("ArrowUp");
  await expect(duty).toHaveValue("51");
  const before = await readCount(page, "read_fan_profile");
  await page.evaluate(() => window.__uiEmit("hardware-controls-changed", null));
  await expect.poll(() => readCount(page, "read_fan_profile")).toBeGreaterThan(before);
  await expect(duty).toHaveValue("51");
  await page.keyboard.up("ArrowUp");
  expect(await readCount(page, "save_custom_fan_curves")).toBe(0);
  await fullSpeed.click();
  await expect(fullSpeed).toBeChecked();
  await expect(duty).toHaveValue("51");
  await holdReadback(page, "save_custom_fan_curves", "read_fan_profile");
  await apply.click();
  await waitForReadback(page);
  await expect(duty).toHaveValue("51");
  await expect(handle).toBeDisabled();
  await expect(handle).toHaveCSS("opacity", "1");
  await expect(apply).toBeDisabled();
  await releaseReadback(page);
  await expect(fan.getByRole("heading", { name: "风扇转速" })).toBeVisible();
  await expect(fullSpeed).toBeChecked();
  await expect(edit).toBeFocused();
  await edit.click();
  await expect(duty).toHaveValue("51");
  expect(await readCount(page, "save_custom_fan_curves")).toBe(1);
});

test("切档复核状态，曲线应用失败保留草稿并拦截过期设置", async ({ page }) => {
  const fan = panel(page, "散热控制");
  const fullSpeed = fan.getByRole("radio", { name: "全速", exact: true });
  const error = fan.getByRole("alert");
  await expect(fan.getByRole("radio", { name: "自动", exact: true })).toBeChecked();
  await page.evaluate(() => { window.__uiFixture.fan.currentMode = 6; });
  await fullSpeed.click();
  await expect(error).toContainText("风扇设置已变化");
  await expect(fan.getByRole("radio", { name: "自定义", exact: true })).toBeChecked();
  expect(await readCount(page, "set_fan_mode")).toBe(0);
  await page.evaluate(() => { window.__uiFixture.fanWriteFault = "set_fan_mode"; });
  await fullSpeed.click();
  await expect(error).toContainText("隔离风扇写入失败");
  await expect(fullSpeed).toBeEnabled();
  await expect(fullSpeed).not.toBeChecked();
  await fan.getByRole("button", { name: "编辑曲线", exact: true }).click();
  await page.evaluate(() => { window.__uiFixture.fanWriteFault = "save_custom_fan_curves"; });
  const input = fan.getByRole("spinbutton", { name: "T2 风扇强度" });
  const apply = fan.getByRole("region", { name: "风扇实况与曲线" }).getByRole("button", { name: "应用", exact: true });
  await input.fill("101");
  await input.press("Enter");
  await apply.click();
  await expect(input).toBeEnabled();
  await expect(input).toHaveValue("98");
  await expect(error).toContainText("隔离曲线保存失败");
  await page.evaluate(() => {
    window.__uiFixture.fanWriteFault = null;
    window.__uiFixture.fan.fans[0].d2Pct = 55;
  });
  await apply.click();
  await expect(error).toContainText("风扇曲线配置已变化");
  await expect(apply).toBeDisabled();
  await expect(input).toHaveValue("98");
  expect(await page.evaluate(() => window.__uiFixture.fan.fans[0].d2Pct)).toBe(55);
});

test("CPU 独立调节不刷新其他卡片", async ({ page }) => {
  await expect(panel(page, "预设模式").locator(".ccc-preset-current")).toHaveText("性能模式");
  await expect(panel(page, "散热控制").getByRole("switch", { name: "Turbo 风扇", exact: true })).toBeEnabled();
  await expect(slider(page, "核心频率偏移")).toBeVisible();
  await expect(slider(page, "加速偏移")).toBeEnabled();
  const others = ["read_oem_power_mode", "read_oem_performance_options", "read_gpu_profile", "read_fan_profile"];
  const before = await Promise.all(others.map(command => readCount(page, command)));
  const pl1 = slider(page, "持续功耗上限");
  await pl1.fill("70");
  await availableAction(page, "立即应用 · CPU").click();
  await expect(pl1).toBeEnabled();
  await expect(pl1).toHaveValue("70");
  await expect(availableAction(page, "立即应用 · CPU")).toHaveCount(0);
  expect(await Promise.all(others.map(command => readCount(page, command)))).toEqual(before);
});

test("风扇换档触发后台回读时保留预设插画与布局", async ({ page }) => {
  const modes = panel(page, "预设模式");
  const artwork = modes.locator(".ccc-preset-artwork");
  await expect(artwork).toHaveAttribute("data-preset-palette", "performance");
  const location = () => artwork.evaluate(node => node.getBoundingClientRect().top - node.closest("section").getBoundingClientRect().top);
  const before = await location();
  await holdReadback(page, "set_fan_mode", "read_oem_power_mode");
  await panel(page, "散热控制").getByRole("radio", { name: "全速", exact: true }).click();
  await waitForReadback(page);
  await expect(modes.getByRole("status")).toHaveText("");
  await expect(artwork).toHaveAttribute("data-preset-palette", "performance");
  expect(await location()).toBeCloseTo(before);
  await releaseReadback(page);
  await expect(modes.getByRole("radio", { name: "性能", exact: true })).toBeEnabled();
});

test("后台读取失败保留面板和实读值，恢复后解除写入锁", async ({ page }) => {
  const cpu = slider(page, "持续功耗上限");
  const gpu = slider(page, "核心频率偏移");
  const turbo = panel(page, "散热控制").getByRole("switch", { name: "Turbo 风扇", exact: true });
  for (const control of [cpu, gpu, turbo]) await expect(control).toBeEnabled();
  const handles = await Promise.all([cpu, gpu, turbo].map(control => control.elementHandle()));
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = (command, args) => {
      if (window.__controlReadFault && ["read_power_profile", "read_gpu_profile", "read_oem_performance_options"].includes(command)) {
        return Promise.reject(new Error("隔离后台回读失败"));
      }
      return invoke(command, args);
    };
    window.__controlReadFault = true;
    window.__uiEmit("hardware-controls-changed", null);
  });
  for (const name of ["CPU 功耗与电压", "GPU 频率", "散热控制"]) {
    await expect(panel(page, name).getByRole("alert")).toContainText("隔离后台回读失败");
  }
  for (const control of [cpu, gpu, turbo]) await expect(control).toBeDisabled();
  await expect(cpu).toHaveValue("65");
  await expect(gpu).toHaveValue("0");
  for (const handle of handles) expect(await handle.evaluate(node => node.isConnected)).toBe(true);
  await page.evaluate(() => {
    window.__controlReadFault = false;
    window.__uiEmit("hardware-controls-changed", null);
  });
  for (const control of [cpu, gpu, turbo]) await expect(control).toBeEnabled();
});

test("传感器采样暂停时保留风扇读数并标记来源", async ({ page }) => {
  const cooling = panel(page, "散热控制");
  const readings = cooling.getByRole("group", { name: "CPU 实况" });
  const dial = readings.getByRole("meter");
  const needle = dial.locator(".ccc-fan-dial-needle");
  await expect(readings).toContainText("°C");
  await expect(dial).toHaveAttribute("aria-valuetext", "1800 RPM");
  const values = await readings.locator("dd").allTextContents();
  const direction = await needle.getAttribute("transform");
  await expect(readings.getByText("上次温度", { exact: true })).toBeVisible();
  await expect(readings.getByText("上次转速", { exact: true })).toBeVisible();
  await expect(dial).toHaveAccessibleName("CPU 上次转速");
  expect(await readings.locator("dd").allTextContents()).toEqual(values);
  await expect(needle).toHaveAttribute("transform", direction);
  await page.evaluate(async () => window.__uiEmit("hardware-snapshot", window.__uiHardwareSnapshot()));
  await expect(dial).toHaveAccessibleName("CPU 实测转速");
});
