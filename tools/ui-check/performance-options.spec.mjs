import { openSystemArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openSystemArea(page, "性能调校"); });
const panel = page => page.getByRole("region", { name: "性能选项", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(c => c.command === command).length, command);
async function openRestore(page) {
  const name = "恢复模式与性能选项";
  await page.getByRole("button", { name, exact: true }).click();
  return page.getByRole("dialog", { name, exact: true });
}
test("性能选项独立回读并共享模式恢复", async ({ page }) => {
  const p = panel(page);
  expect(await calls(page, "apply_oem_performance_options")).toBe(0);
  await p.getByRole("switch", { name: "Turbo 风扇", exact: true }).check();
  await p.getByRole("switch", { name: "动态调校 DTT", exact: true }).check();
  await expect(p.getByRole("switch", { name: "动态调校 DTT", exact: true })).toBeChecked();
  await expect(p.getByText("当前开启", { exact: true })).toHaveCount(0);
  expect(await calls(page, "apply_oem_performance_options")).toBe(2);
  const recovery = await openRestore(page);
  await expect(recovery.getByRole("region", { name: "初始模式恢复目标", exact: true })).toContainText("保持当前偏好");
  await recovery.getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(p.getByRole("switch", { name: "Turbo 风扇", exact: true })).not.toBeChecked();
  await expect(p.getByRole("switch", { name: "动态调校 DTT", exact: true })).not.toBeChecked();
  expect(await calls(page, "restore_oem_power_mode")).toBe(1); expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
test("娱乐模式独立保存性能偏好，切换到性能模式时预览生效", async ({ page }) => {
  const p = panel(page);
  await page.evaluate(() => {
    const fixture = window.__uiFixture;
    fixture.performanceOptions.currentMode = 3;
    fixture.performanceOptions.dttDriverAvailable = false;
    fixture.fan.currentMode = 1;
  });
  await p.getByRole("button", { name: "重新读取性能选项", exact: true }).click();
  await expect(p).toContainText("Turbo 风扇、动态调校 DTT 的偏好可在娱乐模式保存，切换到性能模式后生效");
  await p.getByRole("switch", { name: "Turbo 风扇", exact: true }).check();
  await p.getByRole("switch", { name: "动态调校 DTT", exact: true }).check();
  await expect(p.getByRole("switch", { name: "Turbo 风扇", exact: true })).toBeChecked();
  await expect(p.getByRole("switch", { name: "动态调校 DTT", exact: true })).toBeChecked();
  await expect(p.getByText("已保存开启", { exact: true })).toHaveCount(0);
  expect(await page.evaluate(() => {
    const f = window.__uiFixture;
    return [f.performanceOptions.currentMode, f.fan.currentMode];
  })).toEqual([3, 1]);
  expect(await calls(page, "apply_oem_performance_options")).toBe(2);
  expect(await calls(page, "apply_oem_power_mode")).toBe(0);

  await page.evaluate(() => window.__uiFixture.performanceOptions.dttDriverAvailable = true);
  await openSystemArea(page, "概览");
  await openSystemArea(page, "性能调校");
  const performance = page.getByRole("region", { name: "预设模式", exact: true }).getByRole("radio", { name: /^性能/ });
  await performance.click();
  await expect(performance).toBeChecked();
  await expect(p.getByRole("switch", { name: "Turbo 风扇", exact: true })).toBeChecked();
  await expect(p.getByRole("switch", { name: "动态调校 DTT", exact: true })).toBeChecked();
  await expect(p.getByText("当前开启", { exact: true })).toHaveCount(0);
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
  await p.getByRole("button", { name: "重新读取性能选项", exact: true }).click();
  await expect(p).not.toContainText("切换到性能模式后生效");
  const dialog = page.getByRole("dialog", { name: "应用娱乐模式性能选项", exact: true });
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
  await p.getByRole("button", { name: "重新读取性能选项", exact: true }).click();
  await expect(p.getByText("未确认支持", { exact: true })).toHaveCount(2);
  await expect(p.getByRole("switch", { name: "Turbo 风扇", exact: true })).toHaveAttribute("aria-disabled", "true");
  await expect(p.getByRole("switch", { name: "动态调校 DTT", exact: true })).toHaveAttribute("aria-disabled", "true");
  expect(await calls(page, "apply_oem_performance_options")).toBe(0);
});

test("娱乐档的 DTT 保留自定义曲线，Turbo 经确认交接并可恢复", async ({ page }) => {
  const p = panel(page);
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.entertainmentOptions = true;
    data.performanceOptions.currentMode = 3;
    data.performanceOptions.customFan = true;
    data.fan.currentMode = 6;
  });
  await p.getByRole("button", { name: "重新读取性能选项", exact: true }).click();
  const turbo = p.getByRole("switch", { name: "Turbo 风扇", exact: true });
  const dtt = p.getByRole("switch", { name: "动态调校 DTT", exact: true });
  const dialog = page.getByRole("dialog", { name: "应用娱乐模式性能选项", exact: true });
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
  const restore = await openRestore(page);
  await restore.getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(turbo).not.toBeChecked();
  expect(await page.evaluate(() => window.__uiFixture.fan.currentMode)).toBe(6);
});

test("缺少驱动时禁用性能模式的 DTT，固件未声明时隐藏寂静模式", async ({ page }) => {
  const p = panel(page);
  await page.evaluate(() => window.__uiFixture.performanceOptions.dttDriverAvailable = false);
  await p.getByRole("button", { name: "重新读取性能选项", exact: true }).click();
  await expect(p.getByRole("switch", { name: "动态调校 DTT", exact: true })).toHaveAttribute("aria-disabled", "true");
  await expect(p).toContainText("Dynamic Tuning 驱动未就绪");
  await expect(p.getByRole("switch", { name: "寂静模式", exact: true })).toHaveCount(0);
  expect(await calls(page, "apply_oem_performance_options")).toBe(0);
});

test("性能模式的寂静模式与 Turbo 互斥并可恢复", async ({ page }) => {
  const p = panel(page);
  const turbo = p.getByRole("switch", { name: "Turbo 风扇", exact: true });
  const quiet = p.getByRole("switch", { name: "寂静模式", exact: true });
  await page.evaluate(() => {
    const f = window.__uiFixture;
    f.performanceFanMode = 0;
    f.fan.availableModes.push(8);
    f.performanceOptions.current.turboFan = true;
  });
  await p.getByRole("button", { name: "重新读取性能选项", exact: true }).click();
  await expect(turbo).toBeChecked();
  await quiet.check();
  await expect(turbo).not.toBeChecked();
  await expect(quiet).toBeChecked();
  expect(await calls(page, "apply_oem_performance_options")).toBe(1);
  expect(await page.evaluate(() => {
    const f = window.__uiFixture;
    return [f.performanceOptions.currentMode, f.fan.currentMode, f.performanceFanMode];
  })).toEqual([2, 8, 8]);

  const recovery = await openRestore(page);
  await expect(recovery.getByRole("region", { name: "风扇恢复目标" })).toContainText("自动");
  await recovery.getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(turbo).toBeChecked();
  await expect(quiet).not.toBeChecked();
  expect(await page.evaluate(() => window.__uiFixture.performanceFanMode)).toBe(0);
});

test("娱乐与性能独立开关寂静模式，换档读取对应状态", async ({ page }) => {
  const p = panel(page);
  const quiet = p.getByRole("switch", { name: "寂静模式", exact: true });
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
  await expect(quiet).not.toBeChecked();
  await quiet.check();
  await expect(turbo).toBeChecked();
  expect(await calls(page, "apply_oem_performance_options")).toBe(1);
  await expect(p.getByText("已保存开启", { exact: true })).toHaveCount(0);

  await performance.click();
  await expect(performance).toBeChecked();
  await expect(quiet).not.toBeChecked();
  await quiet.check();
  await expect(turbo).not.toBeChecked();
  await entertainment.click();
  await expect(entertainment).toBeChecked();
  await expect(turbo).not.toBeChecked();
  await expect(quiet).toBeChecked();
  await quiet.uncheck();
  await expect.poll(() => page.evaluate(() => {
    const f = window.__uiFixture;
    return [f.entertainmentQuiet, f.performanceFanMode, f.performanceOptions.current.turboFan];
  })).toEqual([false, 8, false]);

  const recovery = await openRestore(page);
  await expect(recovery.getByRole("region", { name: "模式与性能恢复目标" })).toContainText("娱乐 · 寂静模式");
});

test("寂静模式支持键盘操作并能重新读取外部状态", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 820 });
  const p = panel(page);
  const read = p.getByRole("button", { name: "重新读取性能选项", exact: true });
  const turbo = p.getByRole("switch", { name: "Turbo 风扇", exact: true });
  const quiet = p.getByRole("switch", { name: "寂静模式", exact: true });
  await page.evaluate(() => window.__uiFixture.fan.availableModes.push(8));
  await read.click();
  await expect(p).toContainText("切换模式后，此设备会重置当前模式的寂静模式设置");
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
  expect(await calls(page, "apply_oem_performance_options")).toBe(1);
});
test("自定义风扇只阻止Turbo变更，软件曲线阻止整个选项操作", async ({ page }) => {
  const p = panel(page);
  await page.evaluate(() => window.__uiFixture.performanceOptions.customFan = true);
  await p.getByRole("button", { name: "重新读取性能选项", exact: true }).click();
  await expect(p.getByRole("switch", { name: "Turbo 风扇", exact: true })).toHaveAttribute("aria-disabled", "true");
  await p.getByRole("switch", { name: "动态调校 DTT", exact: true }).check();
  await page.evaluate(() => window.__uiFixture.performanceOptions.softwareActive = true);
  await p.getByRole("button", { name: "重新读取性能选项", exact: true }).click();
  await expect(p.getByRole("switch", { name: "动态调校 DTT", exact: true })).toHaveAttribute("aria-disabled", "true");
  await expect(p).toContainText("退出软件风扇曲线后可修改");
});
test("拒绝过期选项修订并回读实际开关状态", async ({ page }) => {
  const p = panel(page), dtt = p.getByRole("switch", { name: "动态调校 DTT", exact: true });
  await expect(dtt).toBeEnabled();
  await page.evaluate(() => window.__uiFixture.performanceOptions.revision = "external-change");
  await dtt.click();
  await expect(p.getByRole("alert")).toContainText("预览已变化");
  await expect(dtt).not.toBeChecked();
});

test("重新读取显示已保存选项和外部变更", async ({ page }) => {
  const p = panel(page);
  await p.getByRole("switch", { name: "动态调校 DTT", exact: true }).check();
  await page.evaluate(() => { window.__uiFixture.performanceOptions.current.turboFan = true; });
  await p.getByRole("button", { name: "重新读取性能选项", exact: true }).click();
  await expect(p.getByRole("switch", { name: "Turbo 风扇", exact: true })).toBeChecked();
  await expect(p.getByRole("switch", { name: "动态调校 DTT", exact: true })).toBeChecked();
  expect(await calls(page, "apply_oem_performance_options")).toBe(1);
});

for (const [mode, label] of [[3, "娱乐"], [2, "性能"]]) {
  test(`${label}模式可从自定义固件曲线确认切换到寂静模式`, async ({ page }) => {
    const p = panel(page);
    await page.evaluate(mode => {
      const f = window.__uiFixture;
      f.performanceOptions.current = { turboFan: null, dtt: null };
      f.performanceOptions.currentMode = mode;
      f.performanceOptions.customFan = true;
      f.fan.currentMode = 6;
      f.fan.availableModes.push(8);
      f.entertainmentQuiet = mode === 3;
    }, mode);
    await p.getByRole("button", { name: "重新读取性能选项", exact: true }).click();
    await expect(p.getByText(`${label}模式`, { exact: true })).toBeVisible();
    await expect(p.getByText("未确认支持", { exact: true })).toHaveCount(2);
    await expect(p).not.toContainText("切换到性能模式后生效");
    const quiet = p.getByRole("switch", { name: "寂静模式", exact: true });
    const trigger = mode === 3 ? p.getByRole("button", { name: "应用已保存的寂静模式", exact: true }) : quiet;
    await trigger.click();
    const dialog = page.getByRole("dialog", { name: "切换风扇策略", exact: true });
    await expect(dialog).toContainText(`${label}模式将从自定义固件风扇曲线切换到「寂静模式」`);
    await dialog.getByRole("button", { name: "取消", exact: true }).click();
    expect(await calls(page, "apply_oem_performance_options")).toBe(0);
    await trigger.click();
    await dialog.getByRole("button", { name: "确认切换", exact: true }).click();
    await expect(quiet).toBeChecked();
    await expect(p.getByRole("button", { name: "应用已保存的寂静模式", exact: true })).toHaveCount(0);
    expect(await page.evaluate(() => window.__uiFixture.calls.find(c => c.command === "apply_oem_performance_options").args.request))
      .toMatchObject({ quietFan: true, replaceCustomFan: true });
    await quiet.uncheck();
    expect(await page.evaluate(() => window.__uiFixture.performanceOptions.currentMode)).toBe(mode);
    expect(await calls(page, "apply_oem_power_mode")).toBe(0);
  });
}
