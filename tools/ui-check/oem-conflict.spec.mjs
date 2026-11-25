import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { supplyModeFixture } from "./supply-mode-fixture.mjs";
import { openWorkspace, openPowerArea } from "./workspace.mjs";

const owner = { pid: 4301, creationTime: [20, 4] };
const diagnostic = executable => `原厂 ${executable} 正在运行，可能同时修改硬件设置。请退出后重试。`;
const region = (page, name) => page.getByRole("region", { name, exact: true });
const openConflict = card => card.getByRole("button", { name: "关闭 FnKey", exact: true });
const prompt = page => page.getByRole("dialog", { name: "关闭原厂 FnKey", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);

async function setup(page, scenario) {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await page.evaluate(({ owner, issue, scenario, supply }) => {
    const data = window.__uiFixture;
    data.fnKey = ["profile-trial", "automatic-trial"].includes(scenario) ? null : owner;
    if (scenario === "power") {
      data.supplyMode = { ...supply, enabled: true, active: true, canEnable: false, canRestore: true };
      data.supplyModeRevision = 0;
      data.platform.powerDelivery = { revision: "pd-0", encoding: "adapterV1", currentLevel: 1, currentValid: true,
        availableLevels: [1, 2, 4], zeroBased: false, adapterWatts: 100, initial: null, restoreBlockedReason: null, blockedReason: null };
    }
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === "read_fn_key_close_target") {
        data.calls.push({ command, args });
        if (data.fnKeyInspectionFault) throw new Error(data.fnKeyInspectionFault);
        return structuredClone(data.fnKey);
      }
      if (command === "close_fn_key") {
        data.calls.push({ command, args });
        if (data.fnKey && JSON.stringify(args.expected) !== JSON.stringify(data.fnKey))
          throw new Error("原厂 FnKey 已重新启动，请重新核对后确认");
        data.fnKey = null;
        window.__uiEmit("hardware-controls-changed", null);
        return null;
      }
      if (scenario === "write" && command === "enable_automatic_refresh" && data.fnKey) {
        data.calls.push({ command, args });
        throw new Error(`写入前检查：${issue}`);
      }
      if (scenario === "profile-trial" && command === "keep_profile_display" && data.fnKey) {
        data.calls.push({ command, args });
        throw new Error(`档案继续预检：${issue}`);
      }
      if (scenario === "automatic-trial" && command === "keep_display_mode" && data.fnKey) {
        data.calls.push({ command, args });
        throw new Error(`自动刷新率保留预检：${issue}`);
      }
      const result = await invoke(command, args);
      if (scenario === "keyboard" && command === "read_hotkeys") {
        return { ...result, facts: { ...result.facts, blockedReason: issue } };
      }
      if (scenario === "devices" && command === "read_airplane") {
        return { ...result, current: null, revision: null, error: "Windows 未发现原厂飞行模式设备接口" };
      }
      if (!data.fnKey) return result;
      if (scenario === "devices" && ["read_touchpad", "read_microphones"].includes(command)) {
        return { ...result, blockedReason: issue };
      }
      if (["status", "screen"].includes(scenario) && command === "read_automatic_refresh") return { ...result, blockedReason: issue, canStart: false };
      if (scenario === "screen" && ["read_display_modes", "read_airplane"].includes(command)) return { ...result, blockedReason: issue };
      if (scenario === "power" && command === "read_oem_supply_mode") {
        return { ...result, blockedReason: issue, canEnable: false, canResume: false, canRestore: false };
      }
      if (scenario === "power" && command === "read_platform_power") {
        return { ...result, powerDelivery: { ...result.powerDelivery, blockedReason: issue } };
      }
      if (scenario === "power" && command === "read_oem_performance_options") return { ...result, blockedReason: issue };
      if (scenario === "profile" && command === "preview_profile_application") return { ...result, blocked: issue };
      return result;
    };
  }, { owner, issue: diagnostic(scenario === "keyboard" ? "ControlCenter30.exe" : "FnKey.exe"),
    scenario, supply: scenario === "power" ? supplyModeFixture() : null });
}

test("自动刷新率写入冲突打开确认，可取消并重新核验，退出后由用户重试", async ({ page }) => {
  await setup(page, "write");
  await openWorkspace(page, "设备配置");
  const refresh = region(page, "自动刷新率");
  const toggle = refresh.getByRole("switch", { name: "启用自动刷新率" });
  await expect(toggle).toBeEnabled();
  await toggle.click();
  const dialog = prompt(page);
  await expect(dialog.getByRole("heading")).toBeFocused();
  await expect(dialog.getByRole("button", { name: "关闭 FnKey", exact: true })).toBeEnabled();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  expect(await calls(page, "close_fn_key")).toEqual([]);

  await openConflict(refresh).click();
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === "close_fn_key") await new Promise(resolve => { window.__uiFixture.finishExit = resolve; });
      return invoke(command, args);
    };
  });
  await dialog.getByRole("button", { name: "关闭 FnKey", exact: true }).click();
  await expect(dialog.getByRole("button", { name: "正在关闭 FnKey…", exact: true })).toBeDisabled();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeVisible();
  await page.evaluate(() => window.__uiFixture.finishExit());
  await expect(dialog).toHaveCount(0);
  await expect(openConflict(refresh)).toHaveCount(0);
  expect((await calls(page, "close_fn_key")).map(call => call.args)).toEqual([{ expected: owner }]);
  expect(await calls(page, "enable_automatic_refresh")).toHaveLength(1);
  await toggle.click();
  await expect(page.getByRole("dialog", { name: "自动刷新率试用" })).toBeVisible();
});

test("触摸板与麦克风直接显示占用原因，退出后恢复编辑资格，缺失飞行模式接口独立受限", async ({ page }) => {
  await setup(page, "devices");
  await openWorkspace(page, "设备配置");
  const touchpad = region(page, "触摸板"), mic = region(page, "麦克风静音"), airplane = region(page, "飞行模式");
  const touchpadSwitch = touchpad.getByRole("radio", { name: "关闭", exact: true });
  const micSwitch = mic.getByRole("radio", { name: "全部静音", exact: true });
  for (const card of [touchpad, mic]) {
    await expect(card).toContainText(diagnostic("FnKey.exe"));
    await expect(openConflict(card)).toBeEnabled();
  }
  await expect(touchpadSwitch).toBeDisabled();
  await expect(micSwitch).toBeDisabled();
  await expect(prompt(page)).toHaveCount(0);
  const flight = airplane.getByRole("switch", { name: "飞行模式", exact: true });
  await expect(airplane.getByRole("alert")).toHaveText("Windows 未发现原厂飞行模式设备接口");
  await expect(openConflict(airplane)).toHaveCount(0);
  await openConflict(mic).click();
  await prompt(page).getByRole("button", { name: "关闭 FnKey", exact: true }).click();
  await expect(touchpadSwitch).toBeEnabled();
  await expect(micSwitch).toBeEnabled();
  await expect(flight).toBeDisabled();
  expect(await calls(page, "apply_touchpad")).toHaveLength(0);
  expect(await calls(page, "apply_microphones")).toHaveLength(0);
});

test("后台关闭 FnKey 并刷新控制状态后清除旧的写入冲突入口", async ({ page }) => {
  await setup(page, "write");
  await openWorkspace(page, "设备配置");
  const refresh = region(page, "自动刷新率");
  await refresh.getByRole("switch", { name: "启用自动刷新率" }).click();
  await prompt(page).getByRole("button", { name: "取消", exact: true }).click();
  await expect(openConflict(refresh)).toBeEnabled();
  await page.evaluate(() => {
    window.__uiFixture.fnKey = null;
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(openConflict(refresh)).toHaveCount(0);
  expect(await calls(page, "close_fn_key")).toHaveLength(0);
});

test("进程重启后拒绝旧确认，重新打开才获取新进程身份", async ({ page }) => {
  await setup(page, "status");
  await openWorkspace(page, "设备配置");
  const action = openConflict(region(page, "自动刷新率"));
  await action.click();
  const close = prompt(page).getByRole("button", { name: "关闭 FnKey", exact: true });
  await expect(close).toBeEnabled();
  await page.evaluate(() => { window.__uiFixture.fnKey.creationTime = [21, 4]; });
  await close.click();
  await expect(prompt(page).getByRole("alert")).toContainText("已重新启动");
  expect((await calls(page, "close_fn_key"))[0].args.expected).toEqual(owner);
  await expect(close).toHaveCount(0);
  await prompt(page).getByRole("button", { name: "知道了" }).click();
  await action.click();
  await close.click();
  await expect(prompt(page)).toHaveCount(0);
  expect((await calls(page, "close_fn_key"))[1].args.expected.creationTime).toEqual([21, 4]);
});

test("电池校准遇到 BatteryPackUtility 时提供手动处理提示", async ({ page }) => {
  await setup(page, "calibration");
  await page.evaluate(issue => {
    const data = window.__uiFixture;
    data.calibrationSupported = true;
    Object.assign(data.calibration, { canStart: false, unavailableReason: issue });
  }, diagnostic("BatteryPackUtility.exe"));
  await openPowerArea(page, "电源设置");
  const calibration = region(page, "电池校准");
  await expect(calibration).toContainText(diagnostic("BatteryPackUtility.exe"));
  await expect(calibration.getByRole("button", { name: /关闭 FnKey|处理原厂软件冲突/ })).toHaveCount(0);
  await expect(prompt(page)).toHaveCount(0);
  expect(await calls(page, "read_fn_key_close_target")).toEqual([]);
  expect(await calls(page, "close_fn_key")).toEqual([]);
  expect(await calls(page, "start_battery_calibration")).toEqual([]);
});

test("无法核验 FnKey 时保留占用并提示手动退出", async ({ page }) => {
  await setup(page, "status");
  await page.evaluate(() => {
    window.__uiFixture.fnKeyInspectionFault = "无法核验原厂 FnKey 的关闭入口，请从任务管理器退出 FnKey.exe 后重试";
  });
  await openWorkspace(page, "设备配置");
  await openConflict(region(page, "自动刷新率")).click();
  const dialog = prompt(page);
  await expect(dialog.getByRole("alert")).toContainText("请从任务管理器退出 FnKey.exe");
  await expect(dialog.getByRole("button", { name: "关闭 FnKey", exact: true })).toHaveCount(0);
  await dialog.getByRole("button", { name: "知道了" }).click();
  expect(await calls(page, "close_fn_key")).toEqual([]);
});

test("原厂热键直接显示其他原厂进程的退出指引", async ({ page }) => {
  await setup(page, "keyboard");
  await openWorkspace(page, "键盘与灯光");
  const hotkeys = region(page, "原厂热键与屏显");
  await expect(hotkeys.getByText(diagnostic("ControlCenter30.exe"), { exact: true })).toBeVisible();
  await expect(hotkeys.getByRole("button", { name: /关闭 FnKey|处理原厂软件冲突/ })).toHaveCount(0);
});

test("档案退出冲突后重做预览，等待单独确认应用", async ({ page }) => {
  await setup(page, "profile");
  await openPowerArea(page, "配置档案");
  const profile = region(page, "档案预览");
  await expect(profile.getByRole("button", { name: "应用档案", exact: true })).toHaveCount(0);
  await openConflict(profile).click();
  await prompt(page).getByRole("button", { name: "关闭 FnKey", exact: true }).click();
  await expect(profile.getByRole("button", { name: "应用档案", exact: true })).toBeEnabled();
  expect(await calls(page, "apply_profile")).toHaveLength(0);
});

test("档案历史里的原厂占用只显示结果", async ({ page }) => {
  await setup(page, "history");
  const issue = diagnostic("FnKey.exe");
  await page.evaluate(async issue => {
    const data = window.__uiFixture;
    data.fnKey = null;
    data.startupLastRun = issue;
    await window.__TAURI_INTERNALS__.invoke("read_power_profile_policy");
    data.powerPolicy.lastIssue = issue;
    data.profileApplication = { id: 42, revision: 1, origin: "manual", targetsApplied: false,
      name: data.profile.name, steps: [{ step: "档案预检", ok: false, message: issue }], pending: null };
  }, issue);
  await openPowerArea(page, "电源设置");
  for (const card of [region(page, "启动档案"), region(page, "供电与唤醒档案")]) {
    await expect(card).toContainText(issue);
    await expect(openConflict(card)).toHaveCount(0);
  }
  await openPowerArea(page, "配置档案");
  const report = region(page, "最近应用记录");
  await expect(report).toContainText(issue);
  await expect(openConflict(report)).toHaveCount(0);
  expect(await calls(page, "read_fn_key_close_target")).toHaveLength(0);
});

test("设备页读到占用后可直接关闭原厂软件，设置开关保持锁定直至回读", async ({ page }) => {
  await setup(page, "screen");
  await openWorkspace(page, "设备配置");
  const display = region(page, "屏幕显示"), automatic = region(page, "自动刷新率"), airplane = region(page, "飞行模式");
  const controls = [display.getByRole("combobox", { name: "目标刷新率" }),
    automatic.getByRole("switch", { name: "启用自动刷新率" }), airplane.getByRole("switch", { name: "飞行模式", exact: true })];
  for (const [index, card] of [display, automatic, airplane].entries()) {
    await expect(controls[index]).toBeDisabled();
    await expect(openConflict(card)).toBeEnabled();
  }
  await expect(prompt(page)).toHaveCount(0);
  await openConflict(airplane).click();
  await prompt(page).getByRole("button", { name: "关闭 FnKey", exact: true }).click();
  for (const control of controls) await expect(control).toBeEnabled();
  expect(await calls(page, "apply_display_refresh")).toEqual([]);
  expect(await calls(page, "enable_automatic_refresh")).toEqual([]);
  expect(await calls(page, "apply_airplane")).toEqual([]);
});

test("供电占用时仍能暂停，PD 与性能选项复用关闭弹窗", async ({ page }) => {
  await setup(page, "power");
  await openWorkspace(page, "性能与散热");
  await expect(openConflict(region(page, "散热控制"))).toBeEnabled();
  await openPowerArea(page, "电源设置");
  const supply = region(page, "原厂供电模式"), pd = region(page, "平台电源");
  const toggle = supply.getByRole("switch", { name: "启用原厂自动切换" });
  await expect(toggle).toBeChecked();
  await toggle.click();
  await expect(toggle).toBeDisabled();
  expect(await calls(page, "pause_oem_supply_mode")).toHaveLength(1);
  expect(await calls(page, "close_fn_key")).toHaveLength(0);
  await expect(openConflict(supply)).toBeEnabled();
  await expect(pd.getByRole("combobox", { name: "PD 档位" })).toBeDisabled();
  await openConflict(pd).click();
  await prompt(page).getByRole("button", { name: "关闭 FnKey", exact: true }).click();
  await expect(pd.getByRole("combobox", { name: "PD 档位" })).toBeEnabled();
  await expect(toggle).toBeEnabled();
  expect(await calls(page, "resume_oem_supply_mode")).toHaveLength(0);
  expect(await calls(page, "apply_pd_level")).toHaveLength(0);
});

test("从其他页保留档案显示时遇到占用会提示退出，用户可继续取消试用", async ({ page }) => {
  await setup(page, "profile-trial");
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.profile.settings.display = { displays: [{ ...data.display.displays[0].current, frequency: 144 }] };
  });
  await openPowerArea(page, "配置档案");
  await region(page, "档案预览").getByRole("button", { name: "应用档案", exact: true }).click();
  await page.evaluate(owner => { window.__uiFixture.fnKey = owner; }, owner);
  await openWorkspace(page, "设置");
  const trial = region(page, "档案显示确认");
  await trial.getByRole("button", { name: "保留显示并继续档案" }).click();
  await expect(prompt(page)).toBeVisible();
  await prompt(page).getByRole("button", { name: "取消", exact: true }).click();
  await expect(openConflict(trial)).toBeEnabled();
  await trial.getByRole("button", { name: "恢复本次显示并停止" }).click();
  await expect(trial).toHaveCount(0);
  expect(await calls(page, "close_fn_key")).toEqual([]);
  expect(await calls(page, "keep_profile_display")).toHaveLength(1);
});

test("自动刷新率试用期间原厂进程出现时，保留失败打开退出确认", async ({ page }) => {
  await setup(page, "automatic-trial");
  await openWorkspace(page, "设备配置");
  await region(page, "自动刷新率").getByRole("switch", { name: "启用自动刷新率" }).click();
  const trial = page.getByRole("dialog", { name: "自动刷新率试用" });
  await expect(trial).toBeVisible();
  await page.evaluate(owner => { window.__uiFixture.fnKey = owner; }, owner);
  await trial.getByRole("button", { name: "保留刷新率", exact: true }).click();
  await expect(prompt(page)).toBeVisible();
  await prompt(page).getByRole("button", { name: "关闭 FnKey", exact: true }).click();
  await expect(openConflict(trial)).toHaveCount(0);
  expect(await calls(page, "keep_display_mode")).toHaveLength(1);
  await trial.getByRole("button", { name: "恢复本次试用", exact: true }).click();
  await expect(trial).toHaveCount(0);
});
