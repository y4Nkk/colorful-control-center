import { availableAction } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const modes = page => page.getByRole("region", { name: "预设模式", exact: true });
const options = page => page.getByRole("region", { name: "散热控制", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
async function setup(page, mode = 2, ac = 3) {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(({ mode, ac }) => {
    window.__uiFixture.performanceOptions.currentMode = mode;
    window.__uiFixture.acMode = ac;
    window.__uiFixture.battery.power.acOnline = false;
  }, { mode, ac });
  await page.goto("/"); await openWorkspace(page, "性能与散热");
  await expect(modes(page).getByRole("radio", { name: /性能/ })).toBeEnabled();
  await expect(availableAction(page, "预设模式")).toHaveCount(0);
}
async function modeDetails(page) {
  await modes(page).getByRole("button", { name: "模式说明", exact: true }).click();
  return page.getByRole("dialog", { name: "模式说明", exact: true });
}
async function selectMode(page, name) {
  const radio = modes(page).getByRole("radio", { name, exact: true });
  await radio.click();
  await expect(radio).toBeChecked();
  await expect(modes(page).getByRole("radiogroup")).toHaveAttribute("aria-busy", "false");
  await expect(availableAction(page, "预设模式")).toHaveCount(0);
}

test("电池场景读取不写入，点击当前模式即保存插电偏好", async ({ page }) => {
  await setup(page, 0, 3);
  expect(await calls(page, "apply_oem_power_mode")).toEqual([]);
  await selectMode(page, "安静");
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(1);
  expect((await calls(page, "apply_oem_power_mode"))[0].args.expected.acMode).toBe(0);
  expect(await page.evaluate(() => window.__uiFixture.acMode)).toBe(0);
  await expect(options(page)).toContainText("在娱乐或性能模式下可修改 Turbo 风扇和动态调校 DTT");
  await expect(await modeDetails(page)).toContainText("插电偏好 · 安静");
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
  const proceeding = dialog.getByRole("button", { name: "正在关闭并切换…", exact: true });
  const cancel = dialog.getByRole("button", { name: "取消", exact: true });
  await expect(proceeding).toBeDisabled();
  await expect(proceeding).toHaveCSS("cursor", "progress");
  await expect(cancel).toBeDisabled();
  await expect(cancel).toHaveCSS("cursor", "not-allowed");
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
  await openWorkspace(page, "概览");
  await openWorkspace(page, "性能与散热");
  await page.evaluate(() => window.__uiFixture.returnFnKeyConflict());
  await expect(modes(page).getByRole("radiogroup")).toHaveAttribute("aria-busy", "false");
  await expect(page.getByRole("dialog", { name: "关闭原厂 FnKey" })).toHaveCount(0);
  expect((await calls(page, "apply_oem_power_mode")).map(call => call.args.confirmedFnKey)).toEqual([null]);
});

test("等待写入和回读时保留当前插画配色，并阻止重复提交", async ({ page }) => {
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
  const artwork = panel.locator(".ccc-preset-artwork");
  const background = panel.locator(".ccc-preset-backdrop");
  await quiet.click();
  await expect(panel.getByRole("status")).toHaveText("正在切换到安静…");
  await expect(quiet.locator("..")).toHaveAttribute("data-pending", "true");
  await expect(quiet).toHaveCSS("cursor", "progress");
  await expect(artwork).toHaveAttribute("data-preset-palette", "performance");
  await expect(background).toHaveAttribute("data-preset-palette", "performance");
  await expect(panel.locator(".ccc-preset-current")).toHaveText("性能模式");
  await expect(panel.getByRole("radio", { name: "性能", exact: true })).toBeChecked();
  const other = panel.getByRole("radio", { name: "娱乐", exact: true });
  await expect(other).toBeDisabled();
  await expect(other).toHaveCSS("cursor", "not-allowed");
  await other.evaluate(input => input.click());
  await page.evaluate(() => window.__uiFixture.finishModeApply());
  await expect.poll(() => page.evaluate(() => typeof window.__uiFixture.finishModeRead)).toBe("function");
  await expect(artwork).toHaveAttribute("data-preset-palette", "performance");
  await expect(background).toHaveAttribute("data-preset-palette", "performance");
  await expect(quiet).not.toBeChecked();
  await expect(quiet).toHaveCSS("cursor", "progress");
  await page.evaluate(() => window.__uiFixture.finishModeRead());
  await expect(quiet).toBeChecked();
  await expect(quiet.locator("..")).not.toHaveAttribute("data-pending", "true");
  await expect(quiet).toHaveCSS("cursor", "pointer");
  await expect(artwork).toHaveAttribute("data-preset-palette", "quiet");
  await expect(background).toHaveAttribute("data-preset-palette", "quiet");
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
  await expect(panel.locator(".ccc-preset-artwork")).toHaveAttribute("data-preset-palette", "performance");
  await panel.getByRole("button", { name: "重新读取", exact: true }).click();
  await expect(panel.getByRole("alert")).toHaveCount(0);
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(1);
});

test("共享模式读取失败显示原因并清空模式状态", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === "read_oem_power_mode") throw new Error("隔离读取失败");
      return invoke(command, args);
    };
  });
  await openWorkspace(page, "概览");
  await openWorkspace(page, "性能与散热");
  await expect(modes(page).getByRole("alert")).toContainText("隔离读取失败");
  await expect(modes(page).locator(".ccc-preset-artwork")).toHaveAttribute("data-preset-palette", "neutral");
  await expect(modes(page).getByText("当前模式未知", { exact: true })).toBeVisible();
  await expect(modes(page).getByRole("radio", { checked: true })).toHaveCount(0);
});
