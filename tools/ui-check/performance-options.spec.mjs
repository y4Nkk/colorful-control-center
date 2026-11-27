import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "性能与散热"); });
const panel = page => page.getByRole("region", { name: "散热控制", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(c => c.command === command).length, command);
const confirmation = page => page.getByRole("dialog", { name: "确认散热策略", exact: true });
const confirm = page => confirmation(page).getByRole("button", { name: "确认应用", exact: true }).click();
const quietMode = p => p.getByRole("radio", { name: "寂静模式", exact: true });
const quietPreference = p => p.getByRole("switch", { name: "娱乐档寂静偏好", exact: true });
const switchTrack = control => control.locator("..").locator(".ccc-switch-track");

async function expectPerformanceStatus(panel, status = "") {
  for (const name of ["Turbo 风扇", "动态调校 DTT"]) {
    await expect(panel.getByRole("switch", { name, exact: true })).toHaveAccessibleDescription(status);
  }
  const captions = panel.locator(".ccc-cooling-option .ccc-control-caption");
  if (status) await expect(captions).toHaveText([status, status]);
  else await expect(captions).toHaveCount(0);
}

test("散热控制统一回读风扇、Turbo 和 DTT", async ({ page }) => {
  const p = panel(page);
  expect(await calls(page, "apply_oem_performance_options")).toBe(0);
  await p.getByRole("switch", { name: "Turbo 风扇", exact: true }).click();
  await confirm(page);
  await expect(p.getByRole("switch", { name: "Turbo 风扇", exact: true })).toBeChecked();
  await p.getByRole("switch", { name: "动态调校 DTT", exact: true }).check();
  await expect(p.getByRole("switch", { name: "动态调校 DTT", exact: true })).toBeChecked();
  await expectPerformanceStatus(p);
  await expect(page.getByRole("region", { name: "性能选项", exact: true })).toHaveCount(0);
  expect(await calls(page, "apply_oem_performance_options")).toBe(2);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
test("娱乐模式独立保存性能偏好，切换到性能模式时预览生效", async ({ page }) => {
  const p = panel(page);
  await page.evaluate(() => {
    const fixture = window.__uiFixture;
    fixture.performanceOptions.currentMode = 3;
    fixture.performanceOptions.dttDriverAvailable = false;
    fixture.fan.currentMode = 1;
  });
  await p.getByRole("button", { name: "重新读取散热控制", exact: true }).click();
  await expect(p).toContainText("此设备可在娱乐档保存 Turbo 风扇和 DTT 偏好，切换到性能档后应用");
  await p.getByRole("switch", { name: "Turbo 风扇", exact: true }).check();
  await p.getByRole("switch", { name: "动态调校 DTT", exact: true }).check();
  await expect(p.getByRole("switch", { name: "Turbo 风扇", exact: true })).toBeChecked();
  await expect(p.getByRole("switch", { name: "动态调校 DTT", exact: true })).toBeChecked();
  await expectPerformanceStatus(p, "尚未生效");
  for (const name of ["Turbo 风扇", "动态调校 DTT"]) {
    await expect(switchTrack(p.getByRole("switch", { name, exact: true }))).toHaveCSS("background-color", "rgb(58, 131, 247)");
  }
  expect(await page.evaluate(() => {
    const f = window.__uiFixture;
    return [f.performanceOptions.currentMode, f.fan.currentMode];
  })).toEqual([3, 1]);
  expect(await calls(page, "apply_oem_performance_options")).toBe(2);
  expect(await calls(page, "apply_oem_power_mode")).toBe(0);

  await page.evaluate(() => window.__uiFixture.performanceOptions.dttDriverAvailable = true);
  await openWorkspace(page, "概览");
  await openWorkspace(page, "性能与散热");
  const performance = page.getByRole("region", { name: "预设模式", exact: true }).getByRole("radio", { name: /^性能/ });
  await performance.click();
  await expect(performance).toBeChecked();
  await expect(p.getByRole("switch", { name: "Turbo 风扇", exact: true })).toBeChecked();
  await expect(p.getByRole("switch", { name: "动态调校 DTT", exact: true })).toBeChecked();
  await expectPerformanceStatus(p);
  expect(await calls(page, "apply_oem_power_mode")).toBe(1);
  expect(await page.evaluate(() => window.__uiFixture.calls.find(call => call.command === "apply_oem_power_mode").args.expected))
    .toMatchObject({ mode: 2, turbo: true, dtt: true });
});

test("已核对的固件在娱乐档确认、应用和关闭两个性能选项", async ({ page }) => {
  const p = panel(page);
  await page.evaluate(() => {
    const f = window.__uiFixture;
    f.entertainmentOptions = true;
    f.performanceOptions.currentMode = 3;
  });
  await p.getByRole("button", { name: "重新读取散热控制", exact: true }).click();
  await expect(p).not.toContainText("切换到性能模式后生效");
  const dialog = confirmation(page);
  const turbo = p.getByRole("switch", { name: "Turbo 风扇", exact: true });
  const dtt = p.getByRole("switch", { name: "动态调校 DTT", exact: true });
  await turbo.click();
  await expect(dialog).toContainText("Turbo 风扇 · 开启");
  await dialog.getByRole("button", { name: "取消", exact: true }).click();
  expect(await calls(page, "apply_oem_performance_options")).toBe(0);
  for (const [option, command] of [[turbo, 67], [dtt, 195], [turbo, 131], [dtt, 3]]) {
    await option.click();
    await dialog.getByRole("button", { name: "确认应用", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.__uiFixture.modeOptionsCommand)).toBe(command);
  }
  await expect(turbo).not.toBeChecked();
  await expect(dtt).not.toBeChecked();
  expect(await calls(page, "apply_oem_power_mode")).toBe(0);
  expect(await page.evaluate(() => window.__uiFixture.performanceOptions.currentMode)).toBe(3);
});

test("娱乐档将缺少能力证据的开关标为未确认支持", async ({ page }) => {
  const p = panel(page);
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.entertainmentOptions = false;
    data.performanceOptions.currentMode = 3;
    data.performanceOptions.current = { turboFan: null, dtt: null };
  });
  await p.getByRole("button", { name: "重新读取散热控制", exact: true }).click();
  await expectPerformanceStatus(p, "未确认支持");
  await expect(p.getByRole("switch", { name: "Turbo 风扇", exact: true })).toHaveAttribute("aria-disabled", "true");
  await expect(p.getByRole("switch", { name: "动态调校 DTT", exact: true })).toHaveAttribute("aria-disabled", "true");
  expect(await calls(page, "apply_oem_performance_options")).toBe(0);
});

test("娱乐档的 DTT 保留自定义曲线，Turbo 经确认交接", async ({ page }) => {
  const p = panel(page);
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.entertainmentOptions = true;
    data.performanceOptions.currentMode = 3;
    data.fan.currentMode = 6;
  });
  await p.getByRole("button", { name: "重新读取散热控制", exact: true }).click();
  const turbo = p.getByRole("switch", { name: "Turbo 风扇", exact: true });
  const dtt = p.getByRole("switch", { name: "动态调校 DTT", exact: true });
  const dialog = confirmation(page);
  await dtt.click();
  await dialog.getByRole("button", { name: "确认应用", exact: true }).click();
  await expect(dtt).toBeChecked();
  expect(await page.evaluate(() => window.__uiFixture.fan.currentMode)).toBe(6);
  await turbo.click();
  await expect(dialog).toContainText("从自定义固件风扇曲线切换到「自动」");
  await dialog.getByRole("button", { name: "取消", exact: true }).click();
  expect(await calls(page, "apply_oem_performance_options")).toBe(1);
  await turbo.click();
  await dialog.getByRole("button", { name: "确认应用", exact: true }).click();
  await expect(turbo).toBeChecked();
  expect(await page.evaluate(() => {
    const data = window.__uiFixture;
    return [data.performanceOptions.currentMode, data.fan.currentMode, data.modeOptionsCommand];
  })).toEqual([3, 0, 195]);
});

test("缺少驱动时禁用性能模式的 DTT，固件未声明时隐藏寂静模式", async ({ page }) => {
  const p = panel(page);
  await page.evaluate(() => window.__uiFixture.performanceOptions.dttDriverAvailable = false);
  await p.getByRole("button", { name: "重新读取散热控制", exact: true }).click();
  await expect(p.getByRole("switch", { name: "动态调校 DTT", exact: true })).toHaveAttribute("aria-disabled", "true");
  await expect(p).toContainText("Dynamic Tuning 驱动未就绪");
  await expect(quietMode(p)).toHaveCount(0);
  expect(await calls(page, "apply_oem_performance_options")).toBe(0);
});

test("性能模式的寂静模式与 Turbo 互斥", async ({ page }) => {
  const p = panel(page);
  const turbo = p.getByRole("switch", { name: "Turbo 风扇", exact: true });
  const quiet = quietMode(p);
  await page.evaluate(() => {
    const f = window.__uiFixture;
    f.performanceFanMode = 0;
    f.fan.availableModes.push(8);
    f.performanceOptions.current.turboFan = true;
  });
  await p.getByRole("button", { name: "重新读取散热控制", exact: true }).click();
  await expect(turbo).toBeChecked();
  await quiet.click();
  await expect(confirmation(page)).toContainText("Turbo 风扇 · 关闭");
  await expect(confirmation(page)).toContainText("风扇切换到「寂静模式」");
  await confirm(page);
  await expect(turbo).not.toBeChecked();
  await expect(quiet).toBeChecked();
  expect(await calls(page, "apply_oem_performance_options")).toBe(1);
  expect(await calls(page, "set_fan_mode")).toBe(0);
  expect(await page.evaluate(() => {
    const f = window.__uiFixture;
    return [f.performanceOptions.currentMode, f.fan.currentMode, f.performanceFanMode];
  })).toEqual([2, 8, 8]);
});

test("娱乐档寂静偏好、即时寂静和性能档记忆各自独立", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light" });
  const p = panel(page);
  const quiet = quietMode(p);
  const preference = quietPreference(p);
  const turbo = p.getByRole("switch", { name: "Turbo 风扇", exact: true });
  const presets = page.getByRole("region", { name: "预设模式", exact: true });
  const entertainment = presets.getByRole("radio", { name: /^娱乐/ });
  const performance = presets.getByRole("radio", { name: /^性能/ });
  await page.evaluate(() => {
    const f = window.__uiFixture;
    f.performanceFanMode = 0;
    f.fan.availableModes.push(8);
    f.performanceOptions.current.turboFan = true;
    window.__uiEmit("hardware-controls-changed", null);
  });
  await entertainment.click();
  await expect(entertainment).toBeChecked();
  await expect(preference).not.toBeChecked();
  await expect(switchTrack(preference)).toHaveCSS("background-color", "rgb(122, 122, 122)");
  await quiet.click();
  await expect(quiet).toBeChecked();
  await expect(preference).not.toBeChecked();
  await preference.click();
  await confirm(page);
  await expect(preference).toBeChecked();
  await expect(switchTrack(preference)).toHaveCSS("background-color", "rgb(58, 131, 247)");
  await expect(preference).toHaveAccessibleDescription("");
  await expect(p.getByText("已保存", { exact: true })).toHaveCount(0);
  await expect(turbo).toBeChecked();
  await expect(turbo).toHaveAccessibleDescription("尚未生效");
  const auto = p.getByRole("radio", { name: "自动", exact: true });
  await expect(quiet).toBeChecked();
  await auto.click();
  await expect(auto).toBeChecked();
  await expect(preference).toBeChecked();
  await expect(p.getByText("选择「自动」时会应用娱乐档寂静偏好。", { exact: true })).toHaveCount(0);
  const full = p.getByRole("radio", { name: "全速", exact: true });
  await full.click();
  await expect(full).toBeChecked();
  await expect(preference).toBeChecked();
  await expect(quiet).not.toBeChecked();

  await performance.click();
  await expect(performance).toBeChecked();
  await expect(quiet).not.toBeChecked();
  await expect(preference).toHaveCount(0);
  await quiet.click();
  await confirm(page);
  await expect(turbo).not.toBeChecked();
  await entertainment.click();
  await expect(entertainment).toBeChecked();
  await expect(turbo).not.toBeChecked();
  await expect(preference).toBeChecked();
  await expect(quiet).toBeChecked();
  await preference.click();
  await confirm(page);
  await expect(switchTrack(preference)).toHaveCSS("background-color", "rgb(122, 122, 122)");
  await expect(p.getByText("已保存", { exact: true })).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => {
    const f = window.__uiFixture;
    return [f.entertainmentQuiet, f.performanceFanMode, f.performanceOptions.current.turboFan];
  })).toEqual([false, 8, false]);
});

test("寂静模式支持键盘操作并能重新读取外部状态", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 820 });
  const p = panel(page);
  const read = p.getByRole("button", { name: "重新读取散热控制", exact: true });
  const turbo = p.getByRole("switch", { name: "Turbo 风扇", exact: true });
  const quiet = quietMode(p);
  await page.evaluate(() => window.__uiFixture.fan.availableModes.push(8));
  await read.click();
  await expect(p).toContainText("切换预设后，此设备会重置性能档的寂静设置");
  await quiet.focus();
  await page.keyboard.press("Space");
  await expect(quiet).toBeChecked();
  await page.evaluate(() => {
    const fixture = window.__uiFixture;
    fixture.fan.currentMode = 0;
    fixture.performanceOptions.current.turboFan = true;
  });
  await read.click();
  await expect(turbo).toBeChecked();
  await expect(quiet).not.toBeChecked();
  expect(await calls(page, "set_fan_mode")).toBe(1);
});
test("自定义风扇只阻止Turbo变更，软件曲线阻止整个选项操作", async ({ page }) => {
  const p = panel(page);
  await page.evaluate(() => window.__uiFixture.fan.currentMode = 6);
  await p.getByRole("button", { name: "重新读取散热控制", exact: true }).click();
  await expect(p.getByRole("switch", { name: "Turbo 风扇", exact: true })).toHaveAttribute("aria-disabled", "true");
  await p.getByRole("switch", { name: "动态调校 DTT", exact: true }).check();
  await page.evaluate(() => window.__uiFixture.fan.software = {});
  await p.getByRole("button", { name: "重新读取散热控制", exact: true }).click();
  await expect(p.getByRole("switch", { name: "动态调校 DTT", exact: true })).toHaveAttribute("aria-disabled", "true");
  await expect(p).toContainText("在「风扇模式」选择「自动」后，可应用这些选项。");
});
test("拒绝过期选项修订并回读实际开关状态", async ({ page }) => {
  const p = panel(page), dtt = p.getByRole("switch", { name: "动态调校 DTT", exact: true });
  await expect(dtt).toBeEnabled();
  await page.evaluate(() => window.__uiFixture.performanceOptions.revision = "external-change");
  await dtt.click();
  await expect(p.getByRole("alert")).toContainText("预览已变化");
  await expect(dtt).not.toBeChecked();
});

test("重新读取显示偏好与当前启停的差异", async ({ page }) => {
  const p = panel(page);
  const turbo = p.getByRole("switch", { name: "Turbo 风扇", exact: true });
  const dtt = p.getByRole("switch", { name: "动态调校 DTT", exact: true });
  const reload = p.getByRole("button", { name: "重新读取散热控制", exact: true });
  await dtt.check();
  await page.evaluate(() => {
    const fixture = window.__uiFixture;
    fixture.entertainmentOptions = true;
    fixture.performanceOptions.currentMode = 3;
    fixture.performanceOptions.current.turboFan = true;
    fixture.modeOptionsCommand = 3 | 128;
  });
  await reload.click();
  await expect(turbo).toBeChecked();
  await expect(turbo).toHaveAccessibleDescription("尚未生效");
  await expect(dtt).toBeChecked();
  await expect(dtt).toHaveAccessibleDescription("");
  await page.evaluate(() => {
    const fixture = window.__uiFixture;
    fixture.performanceOptions.current.turboFan = false;
    fixture.modeOptionsCommand |= 64;
  });
  await reload.click();
  await expect(turbo).not.toBeChecked();
  await expect(turbo).toHaveAccessibleDescription("当前仍开启");
  expect(await calls(page, "apply_oem_performance_options")).toBe(1);
});

for (const [mode, label] of [[3, "娱乐"], [2, "性能"]]) {
  test(`${label}模式可从自定义固件曲线确认切换到寂静模式`, async ({ page }) => {
    const p = panel(page);
    await page.evaluate(mode => {
      const f = window.__uiFixture;
      f.performanceOptions.current = { turboFan: null, dtt: null };
      f.performanceOptions.currentMode = mode;
      f.fan.currentMode = 6;
      f.fan.availableModes.push(8);
      f.entertainmentQuiet = mode === 3;
    }, mode);
    await p.getByRole("button", { name: "重新读取散热控制", exact: true }).click();
    await expectPerformanceStatus(p, "未确认支持");
    await expect(p).not.toContainText("切换到性能模式后生效");
    const quiet = quietMode(p);
    await quiet.click();
    const dialog = confirmation(page);
    await expect(dialog).toContainText("从自定义固件风扇曲线切换到「寂静模式」");
    await dialog.getByRole("button", { name: "取消", exact: true }).click();
    expect(await calls(page, "apply_oem_performance_options")).toBe(0);
    await quiet.click();
    await confirm(page);
    await expect(quiet).toBeChecked();
    expect(await page.evaluate(() => window.__uiFixture.calls.find(c => c.command === "apply_oem_performance_options").args.request))
      .toMatchObject({ quietFan: "current", replaceCustomFan: true });
    expect(await page.evaluate(() => window.__uiFixture.performanceOptions.currentMode)).toBe(mode);
    expect(await calls(page, "apply_oem_power_mode")).toBe(0);
  });
}

test("只修改 DTT 时保留已保存的 Turbo、风扇档位和两路曲线", async ({ page }) => {
  const p = panel(page);
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.entertainmentOptions = true;
    data.performanceOptions.currentMode = 3;
    data.performanceOptions.current.turboFan = true;
    data.fan.currentMode = 6;
    data.fan.fans.push({ ...data.fan.fans[0], id: "gpu1" });
  });
  await p.getByRole("button", { name: "重新读取散热控制" }).click();
  await p.getByRole("button", { name: "编辑曲线", exact: true }).click();
  const gpu = p.getByRole("group", { name: "GPU 1 曲线点" });
  const cpu = p.getByRole("group", { name: "CPU 曲线点" });
  const turbo = p.getByRole("switch", { name: "Turbo 风扇", exact: true });
  await expect(gpu).toBeVisible();
  await expect(turbo).toHaveAccessibleDescription("尚未生效");
  await expect(p.getByRole("button", { name: "应用已保存的性能选项", exact: true })).toHaveCount(0);
  await p.getByRole("switch", { name: "动态调校 DTT", exact: true }).click();
  await expect(confirmation(page)).not.toContainText("Turbo 风扇");
  await confirm(page);
  await expect(p.getByRole("switch", { name: "动态调校 DTT", exact: true })).toBeChecked();
  await expect(turbo).toHaveAccessibleDescription("尚未生效");
  await expect(gpu).toBeVisible();
  await expect(cpu).toBeVisible();
  expect(await page.evaluate(() => [window.__uiFixture.modeOptionsCommand, window.__uiFixture.fan.currentMode])).toEqual([131, 6]);
});
