import { availableAction, configurationView, submitConfiguration, previewConfiguration } from "./configuration-review.mjs";
import { openSystemArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const modes = page => page.getByRole("region", { name: "预设模式", exact: true });
const options = page => page.getByRole("region", { name: "性能选项", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
async function setup(page, mode = 2, ac = 3) {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(({ mode, ac }) => {
    window.__uiFixture.performanceOptions.currentMode = mode;
    window.__uiFixture.acMode = ac;
    window.__uiFixture.battery.power.acOnline = false;
  }, { mode, ac });
  await page.goto("/"); await openSystemArea(page, "性能调校");
  await expect(modes(page).getByRole("radio", { name: /性能/ })).toBeEnabled();
  await expect(availableAction(page, "预设模式")).toHaveCount(0);
}
async function modeDetails(page) {
  await modes(page).getByRole("button", { name: "详情", exact: true }).click();
  return page.getByRole("dialog", { name: "模式说明", exact: true });
}
async function selectMode(page, name) {
  const radio = modes(page).getByRole("radio", { name, exact: true });
  await radio.click();
  await expect(radio).toBeChecked();
  await expect(modes(page).getByRole("radiogroup")).toHaveAttribute("aria-busy", "false");
  await expect(availableAction(page, "预设模式")).toHaveCount(0);
}
async function enableTurbo(page) {
  const p = options(page);
  await p.getByRole("switch", { name: "Turbo 风扇", exact: true }).click();
  await expect(p.getByRole("switch", { name: "Turbo 风扇", exact: true })).toBeChecked();
  await expect(p.getByText("当前开启", { exact: true })).toHaveCount(0);
}
async function restore(page) {
  const review = await previewConfiguration(page, "模式与性能选项", true);
  expect(await calls(page, "restore_oem_power_mode")).toHaveLength(0);
  await submitConfiguration(review, "确认恢复");
}

test("电池场景读取不写入，点击当前模式即保存插电偏好", async ({ page }) => {
  await setup(page, 0, 3);
  expect(await calls(page, "apply_oem_power_mode")).toEqual([]);
  await selectMode(page, "安静");
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(1);
  expect((await calls(page, "apply_oem_power_mode"))[0].args.expected.acMode).toBe(0);
  expect(await page.evaluate(() => window.__uiFixture.acMode)).toBe(0);
  await expect(options(page)).toContainText("在娱乐或性能模式下可修改性能选项");
  await restore(page);
  await expect(await modeDetails(page)).toContainText("插电偏好 · 娱乐");
  const restored = (await calls(page, "restore_oem_power_mode"))[0];
  expect(typeof restored.args.expectedRevision).toBe("string");
  expect(restored.args.expectedRevision.length).toBeGreaterThan(10);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("打开页面只读，明确重选当前模式只提交一次联动", async ({ page }) => {
  await setup(page, 2, 2);
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(0);
  await selectMode(page, "性能");
  const requests = await calls(page, "apply_oem_power_mode");
  expect(requests).toHaveLength(1);
  expect(requests[0].args.expected.mode).toBe(2);
});

async function startFnKeyPrompt(page) {
  await setup(page, 3, 3);
  await page.evaluate(() => { window.__uiFixture.fnKey = { pid: 4321, creationTime: [120, 7] }; });
  await modes(page).getByRole("radio", { name: "性能", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "关闭原厂 FnKey", exact: true });
  await expect(dialog).toBeVisible();
  return dialog;
}

test("FnKey 弹窗说明目标和热键影响，按 Escape 取消不退出或切换", async ({ page }) => {
  const dialog = await startFnKeyPrompt(page);
  await expect(dialog).toHaveCSS("width", "440px");
  await expect(dialog).toContainText("切换到「性能」");
  await expect(dialog).toContainText("Fn hotkeys and OSD");
  await expect(dialog.getByRole("heading")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.fnKey.pid)).toBe(4321);
  expect((await calls(page, "apply_oem_power_mode")).map(call => call.args.confirmedFnKey)).toEqual([null]);
  await expect(modes(page).getByRole("radio", { name: "娱乐", exact: true })).toBeChecked();
});

test("关闭并切换绑定原进程和目标，等待退出时锁定重复点击", async ({ page }, info) => {
  const dialog = await startFnKeyPrompt(page);
  await dialog.screenshot({ path: info.outputPath("fnkey-confirmation.png") });
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === "apply_oem_power_mode" && args.confirmedFnKey) {
        await new Promise(resolve => { window.__uiFixture.finishFnKeyExit = resolve; });
      }
      return invoke(command, args);
    };
  });
  await dialog.getByRole("button", { name: "关闭并切换", exact: true }).click();
  await expect(dialog.getByRole("button", { name: "正在关闭并切换…", exact: true })).toBeDisabled();
  await expect(dialog.getByRole("button", { name: "取消", exact: true })).toBeDisabled();
  await expect(dialog.getByRole("button", { name: "关闭弹窗", exact: true })).toBeDisabled();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeVisible();
  expect(await page.evaluate(() => window.__uiFixture.performanceOptions.currentMode)).toBe(3);
  await page.evaluate(() => window.__uiFixture.finishFnKeyExit());
  await expect(dialog).toHaveCount(0);
  await expect(modes(page).getByRole("radio", { name: "性能", exact: true })).toBeChecked();
  const requests = await calls(page, "apply_oem_power_mode");
  expect(requests).toHaveLength(2);
  expect(requests[1].args).toEqual({ expected: requests[0].args.expected,
    confirmedFnKey: { identity: { pid: 4321, creationTime: [120, 7] }, modeRevision: requests[0].args.expected.revision } });
  expect(await page.evaluate(() => window.__uiFixture.fnKey)).toBeNull();
});

for (const state of ["进程重启", "退出超时"]) {
  test(`FnKey ${state}时保留原模式并显示原因`, async ({ page }) => {
    const dialog = await startFnKeyPrompt(page);
    await page.evaluate(state => {
      if (state === "进程重启") window.__uiFixture.fnKey.creationTime = [121, 7];
      else window.__uiFixture.fnKeyExitFault = "等待原厂 FnKey 退出超时，请从任务管理器退出后重试";
    }, state);
    await dialog.getByRole("button", { name: "关闭并切换", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await expect(modes(page).getByRole("alert")).toContainText(state === "进程重启" ? "已重新启动" : "退出超时");
    await expect(modes(page).getByRole("radio", { name: "娱乐", exact: true })).toBeChecked();
    expect(await page.evaluate(() => window.__uiFixture.fnKey.pid)).toBe(4321);
  });
}

test("确认期间模式前态已变时不关闭 FnKey", async ({ page }) => {
  const dialog = await startFnKeyPrompt(page);
  await page.evaluate(() => { window.__uiFixture.performanceOptions.currentMode = 0; });
  await dialog.getByRole("button", { name: "关闭并切换", exact: true }).click();
  await expect(modes(page).getByRole("alert")).toContainText("模式联动条件已变化");
  await expect(modes(page).getByRole("radio", { name: "安静", exact: true })).toBeChecked();
  expect(await page.evaluate(() => window.__uiFixture.fnKey.pid)).toBe(4321);
});

test("离开预设模式区域后丢弃迟到的 FnKey 确认", async ({ page }) => {
  await setup(page, 3, 3);
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.fnKey = { pid: 4321, creationTime: [120, 7] };
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === "apply_oem_power_mode") await new Promise(resolve => { data.returnFnKeyConflict = resolve; });
      return invoke(command, args);
    };
  });
  await modes(page).getByRole("radio", { name: "性能", exact: true }).click();
  await expect.poll(() => page.evaluate(() => typeof window.__uiFixture.returnFnKeyConflict)).toBe("function");
  await openSystemArea(page, "概览");
  await openSystemArea(page, "性能调校");
  await page.evaluate(() => window.__uiFixture.returnFnKeyConflict());
  await expect(modes(page).getByRole("radiogroup")).toHaveAttribute("aria-busy", "false");
  await expect(page.getByRole("dialog", { name: "关闭原厂 FnKey" })).toHaveCount(0);
  expect((await calls(page, "apply_oem_power_mode")).map(call => call.args.confirmedFnKey)).toEqual([null]);
});

test("等待写入和回读时保留当前芯片配色，并阻止重复提交", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => {
    const fixture = window.__uiFixture;
    const invoke = window.__TAURI_INTERNALS__.invoke;
    let waitingForReadback = false;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === "apply_oem_power_mode") {
        await new Promise(resolve => { fixture.finishModeApply = resolve; });
        const result = await invoke(command, args);
        waitingForReadback = true;
        return result;
      }
      if (command === "read_oem_power_mode" && waitingForReadback) {
        waitingForReadback = false;
        await new Promise(resolve => { fixture.finishModeRead = resolve; });
      }
      return invoke(command, args);
    };
  });
  const panel = modes(page);
  const quiet = panel.getByRole("radio", { name: "安静", exact: true });
  const artwork = panel.locator(".ccc-preset-chip");
  await quiet.click();
  await expect(panel.getByRole("status")).toHaveText("正在切换到安静…");
  await expect(quiet.locator("..")).toHaveAttribute("data-pending", "true");
  await expect(artwork).toHaveAttribute("data-preset-palette", "performance");
  await expect(panel.locator(".ccc-preset-current")).toHaveText("性能模式");
  await expect(panel.getByRole("radio", { name: "性能", exact: true })).toBeChecked();
  const other = panel.getByRole("radio", { name: "娱乐", exact: true });
  await expect(other).toBeDisabled();
  await other.evaluate(input => input.click());
  await page.evaluate(() => window.__uiFixture.finishModeApply());
  await expect.poll(() => page.evaluate(() => typeof window.__uiFixture.finishModeRead)).toBe("function");
  await expect(artwork).toHaveAttribute("data-preset-palette", "performance");
  await expect(quiet).not.toBeChecked();
  await page.evaluate(() => window.__uiFixture.finishModeRead());
  await expect(quiet).toBeChecked();
  await expect(quiet.locator("..")).not.toHaveAttribute("data-pending", "true");
  await expect(artwork).toHaveAttribute("data-preset-palette", "quiet");
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(1);
});

test("写入返回成功后，显示设备实际回读的模式", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      const result = await invoke(command, args);
      if (command === "apply_oem_power_mode" && result.ok) window.__uiFixture.performanceOptions.currentMode = 2;
      return result;
    };
  });
  const panel = modes(page);
  await panel.getByRole("radio", { name: "安静", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("设备回读的模式与所选模式不同");
  await expect(panel.getByRole("radio", { name: "性能", exact: true })).toBeChecked();
  await expect(panel.locator(".ccc-preset-chip")).toHaveAttribute("data-preset-palette", "performance");
  await panel.getByRole("button", { name: "重新读取", exact: true }).click();
  await expect(panel.getByRole("alert")).toHaveCount(0);
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(1);
});

test("仅选项不接管插电偏好，恢复时也保留后来的偏好", async ({ page }) => {
  await setup(page, 2, 1); await enableTurbo(page);
  expect(await page.evaluate(() => window.__uiFixture.acMode)).toBe(1);
  const review = await previewConfiguration(page, "模式与性能选项", true);
  await expect(review.getByRole("region", { name: "初始模式恢复目标", exact: true })).toContainText("保持当前偏好");
  await page.evaluate(() => { window.__uiFixture.acMode = 3; });
  await configurationView(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(options(page).getByRole("switch", { name: "Turbo 风扇", exact: true })).not.toBeChecked();
  expect(await page.evaluate(() => window.__uiFixture.acMode)).toBe(3);
});

test("调校页启用选项后再切换模式，共享恢复目标扩充且使用最新确认", async ({ page }) => {
  await setup(page); await enableTurbo(page);
  const early = await previewConfiguration(page, "模式与性能选项", true);
  await expect(early).toContainText("保持当前偏好");
  await page.keyboard.press("Escape");
  await selectMode(page, "娱乐");
  await expect(availableAction(page, "模式与性能选项", true)).toBeVisible();
  await expect(options(page)).toContainText(/偏好.*性能模式.*生效/);
  expect(await calls(page, "restore_oem_power_mode")).toEqual([]);
  expect(await page.evaluate(() => window.__uiFixture.modeInitial.acMode)).toBe(3);
  await previewConfiguration(page, "模式与性能选项", true);
  await configurationView(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(modes(page).getByRole("radio", { name: "性能", exact: true })).toBeChecked();
  await expect(options(page).getByRole("switch", { name: "Turbo 风扇", exact: true })).not.toBeChecked();
});

test("后端拒绝已经改变的初始快照摘要，不执行未预览恢复", async ({ page }) => {
  await setup(page); await selectMode(page, "省电");
  await previewConfiguration(page, "模式与性能选项", true);
  await page.evaluate(() => { window.__uiFixture.modeInitial.acMode = 0; });
  await configurationView(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(modes(page).getByRole("alert")).toContainText("恢复目标已变化");
  expect(await page.evaluate(() => window.__uiFixture.performanceOptions.currentMode)).toBe(1);
  expect(await page.evaluate(() => window.__uiFixture.acMode)).toBe(1);
});

test("共享模式读取失败会清除旧的可执行恢复快照", async ({ page }) => {
  await setup(page); await enableTurbo(page);
  await expect(availableAction(page, "模式与性能选项", true)).toBeVisible();
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === "read_oem_power_mode") throw new Error("隔离读取失败");
      return invoke(command, args);
    };
  });
  await openSystemArea(page, "风扇散热");
  await openSystemArea(page, "性能调校");
  await expect(modes(page).getByRole("alert")).toContainText("隔离读取失败");
  await expect(modes(page).locator(".ccc-preset-chip")).toHaveAttribute("data-preset-palette", "neutral");
  await expect(modes(page).getByText("当前模式未知", { exact: true })).toBeVisible();
  await expect(modes(page).getByRole("radio", { checked: true })).toHaveCount(0);
  await expect(availableAction(page, "模式与性能选项", true)).toHaveCount(0);
  expect(await calls(page, "restore_oem_power_mode")).toEqual([]);
});

test("初始恢复完整目标适配宽窄和深浅主题，折叠原始风扇参数", async ({ page }) => {
  await setup(page); await selectMode(page, "安静");
  const recovery = await previewConfiguration(page, "模式与性能选项", true);
  const target = recovery.getByRole("region", { name: "初始模式恢复目标", exact: true });
  for (const theme of ["light", "dark"]) {
    await page.evaluate(value => { document.documentElement.dataset.theme = value; }, theme);
    for (const width of [360, 1000, 1440]) {
      await page.setViewportSize({ width, height: 950 }); await target.scrollIntoViewIfNeeded();
      const geometry = await target.evaluate(element => {
        const panel = element.closest(".ccc-scroll-viewport"), style = getComputedStyle(panel);
        return { width: element.clientWidth, scroll: element.scrollWidth,
          available: panel.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight) };
      });
      expect(geometry.scroll).toBeLessThanOrEqual(geometry.width + 1);
      expect(geometry.width).toBeGreaterThan(geometry.available * 0.94);
      await expect(target.getByRole("heading", { name: "模式与性能", exact: true })).toBeVisible();
      await expect(target.getByRole("heading", { name: "散热策略", exact: true })).toBeVisible();
      expect(await target.locator('.ccc-facts[data-layout="tiles"] .ccc-fact-row').evaluateAll(rows => rows.every(row => parseFloat(getComputedStyle(row).borderTopWidth) === 0))).toBe(true);
      await expect(target).toContainText("Turbo 风扇偏好"); await expect(target).toContainText("动态调校 DTT 偏好");
      if (width !== 1000) await page.screenshot({ path: `target/ui-check/mode-restore-${theme}-${width}.png` });
    }
  }
  await target.getByRole("button", { name: "查看完整风扇恢复参数", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "查看完整风扇恢复参数" }).locator("pre")).toContainText('"fields"');
  expect(await calls(page, "restore_oem_power_mode")).toEqual([]);
});
