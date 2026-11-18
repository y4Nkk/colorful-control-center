import { availableAction, configurationDialog, confirmConfiguration, openConfigurationReview } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { modeGpuFixture } from "./mode-gpu-fixture.mjs";
import { modeCycleFixture } from "./mode-cycle-fixture.mjs";

const startup = page => page.getByRole("region", { name: "启动档案", exact: true });
const policy = page => page.getByRole("region", { name: "供电与唤醒档案", exact: true });
const approval = region => region.getByRole("region", { name: "后台模式GPU联动", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
async function choose(page, region, label) {
  await region.getByRole("combobox", { name: label, exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
}
async function saveStartup(page) {
  await confirmConfiguration(await openConfigurationReview(page, "启动档案"));
  await expect(availableAction(page, "启动档案")).toHaveCount(0);
}
async function openPolicyDetails(page) {
  await policy(page).getByRole("button", { name: "详情", exact: true }).click();
  return page.getByRole("dialog", { name: "供电档案状态与记录", exact: true });
}
async function noHardwareCalls(page) {
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|restore_)/.test(call.command)))).toEqual([]);
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(({ fixture, cycle }) => {
    const data = window.__uiFixture, node = data.gpuPower.devices[0];
    Object.assign(node, { enabled: true, automaticPowerOff: false, problem: 0, drivesDisplay: false, canDisable: true });
    fixture.value.clocks.candidate = structuredClone(cycle.clocks.choices.find(choice => choice.mode === 2 && choice.onAc).candidate);
    data.modeGpuOverrides = { 2: fixture };
    data.profileModeChoices = cycle.clocks.choices;
    data.profile.settings.oemPowerMode = 2;
  }, { fixture: modeGpuFixture(), cycle: modeCycleFixture() });
});

test("启动确认展示两种供电目标，只保存稳定授权而不执行设备", async ({ page }) => {
  await page.goto("/"); await openWorkspace(page, "自动策略");
  await choose(page, startup(page), "启动档案");
  await expect(approval(startup(page))).toHaveCount(0);
  const review = await openConfigurationReview(page, "启动档案");
  const table = approval(review).getByRole("table", { name: "供电频率目标" });
  await expect(table.getByRole("row").filter({ hasText: "插电" })).toContainText("1700");
  await expect(table.getByRole("row").filter({ hasText: "电池" })).toContainText("保持");
  await expect(approval(review).locator(".ccc-gpu-power-visual svg")).toBeVisible();
  expect(await calls(page, "set_startup_profile")).toHaveLength(0);
  await confirmConfiguration(review);
  await expect(availableAction(page, "启动档案")).toHaveCount(0);
  const [call] = await calls(page, "set_startup_profile");
  expect(call.args.expectedMode.clocks.choices.map(choice => choice.onAc)).toEqual([true, false]);
  const binding = await page.evaluate(() => window.__uiFixture.startupSelection);
  expect(Object.keys(binding.mode).sort()).toEqual(["automaticPowerOff", "driver", "driverDeferred", "identity"]);
  expect(binding.mode.driver).toBe("a".repeat(64));
  await noHardwareCalls(page);
});

test("供电规则确认完整未来目标，启用只记录版本二授权", async ({ page }) => {
  await page.goto("/"); await openWorkspace(page, "自动策略");
  await choose(page, policy(page), "插电时");
  await policy(page).getByRole("switch", { name: "启用自动档案", exact: true }).check();
  const review = await openConfigurationReview(page, "供电与唤醒档案");
  await expect(approval(review)).toBeVisible();
  expect(await calls(page, "save_power_profile_policy")).toHaveLength(0);
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  await expect(policy(page).getByText("已启用", { exact: true })).toBeVisible();
  const request = (await calls(page, "save_power_profile_policy"))[0].args.request;
  expect(request.ac.mode.clocks.choices).toHaveLength(2);
  expect(request.ac.mode.allowedAutomaticPowerOff).toEqual([false]);
  const record = await page.evaluate(() => window.__uiFixture.powerPolicy.record);
  expect(record.version).toBe(2); expect(record.ac.mode.identity).toBe(request.ac.mode.identity);
  expect(record.ac.mode).not.toHaveProperty("target");
  await noHardwareCalls(page);
});

test("未来GPU目标读取失败时可查看原因并阻止保存", async ({ page }) => {
  await page.addInitScript(() => { window.__uiFixture.profileAuthorizationFault = "未来电池目标超过驱动范围"; });
  await page.goto("/"); await openWorkspace(page, "自动策略");
  await choose(page, startup(page), "启动档案");
  await expect(startup(page).getByRole("alert").filter({ hasText: "未来电池目标超过驱动范围" })).toBeVisible();
  await expect(availableAction(page, "启动档案")).toHaveCount(0);
  await choose(page, policy(page), "插电时");
  await policy(page).getByRole("switch", { name: "启用自动档案", exact: true }).check();
  await policy(page).getByRole("button", { name: "插电时 · 档案不可用", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "插电时 · 档案不可用", exact: true })).toContainText("未来电池目标超过驱动范围");
  await page.keyboard.press("Escape");
  await expect(availableAction(page, "供电与唤醒档案")).toHaveCount(0);
  expect(await calls(page, "set_startup_profile")).toHaveLength(0);
  expect(await calls(page, "save_power_profile_policy")).toHaveLength(0);
});

test("确认前未来目标变化拒绝旧供电规则且不触发设备执行", async ({ page }) => {
  await page.goto("/"); await openWorkspace(page, "自动策略");
  await choose(page, policy(page), "插电时");
  await policy(page).getByRole("switch", { name: "启用自动档案", exact: true }).check();
  await openConfigurationReview(page, "供电与唤醒档案");
  await page.evaluate(() => { window.__uiFixture.profileModeChoices.find(choice => choice.mode === 2 && !choice.onAc).candidate.decision.target.lockMhz = 800; });
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  await expect(await openPolicyDetails(page)).toContainText("模式联动已变化");
  expect(await page.evaluate(() => window.__uiFixture.powerPolicy.record.enabled)).toBe(false);
  await noHardwareCalls(page);
});

test("不可用旧规则按文件修订两次确认清除，变化时拒绝旧许可", async ({ page }) => {
  await page.addInitScript(() => {
    const data = window.__uiFixture;
    data.powerPolicy = { record: null, initial: null, paused: true, active: false, source: "ac", sleeping: false, pending: null,
      route: { modernStandby: false, displayEvents: false, warnings: [] }, configurationError: "自动档案版本已过期", resetRevision: "a".repeat(64), listenerError: null, lastIssue: null };
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === "reset_power_profile_policy" && !data.resetChanged) { data.resetChanged = true; data.powerPolicy.resetRevision = "b".repeat(64); }
      return invoke(command, args);
    };
  });
  await page.goto("/"); await openWorkspace(page, "自动策略");
  await policy(page).getByRole("button", { name: "清除不可用规则", exact: true }).click();
  expect(await calls(page, "reset_power_profile_policy")).toHaveLength(0);
  const reset = page.getByRole("dialog", { name: "清除不可用规则", exact: true });
  await confirmConfiguration(reset, "清除规则并停用");
  await expect(await openPolicyDetails(page)).toContainText("规则已变化");
  await page.keyboard.press("Escape");
  await policy(page).getByRole("button", { name: "清除不可用规则", exact: true }).click();
  await confirmConfiguration(reset, "清除规则并停用");
  await expect(policy(page).getByRole("button", { name: "清除不可用规则", exact: true })).toHaveCount(0);
  const requests = await calls(page, "reset_power_profile_policy");
  expect(requests.map(call => call.args.expectedRevision)).toEqual(["a".repeat(64), "b".repeat(64)]);
  const state = await page.evaluate(() => window.__uiFixture.powerPolicy);
  expect(state.record).toEqual({ version: 2, enabled: false, ac: null, battery: null, resume: null });
  expect(state.paused).toBe(true); expect(state.active).toBe(false);
  await noHardwareCalls(page);
});

test("同名启动档案修改后重新预览可以重新确认", async ({ page }) => {
  await page.goto("/"); await openWorkspace(page, "自动策略");
  await choose(page, startup(page), "启动档案");
  await saveStartup(page);
  await page.evaluate(() => { window.__uiFixture.profile.settings.pl1W = 88; });
  await startup(page).getByRole("button", { name: "重新读取启动档案", exact: true }).click();
  await expect(startup(page).getByRole("alert").filter({ hasText: "档案内容已变化" })).toBeVisible();
  await expect(availableAction(page, "启动档案")).toBeEnabled();
  await saveStartup(page);
  expect((await calls(page, "set_startup_profile"))[1].args.expected.settings.pl1W).toBe(88);
  await expect(startup(page).getByRole("alert")).toHaveCount(0);
  await noHardwareCalls(page);
});

test("启动元数据读取失败保留已选草稿和完整目标", async ({ page }) => {
  await page.goto("/"); await openWorkspace(page, "自动策略");
  await choose(page, startup(page), "启动档案");
  await page.evaluate(() => { window.__uiFixture.startupReadFault = "隔离启动文件被占用"; });
  await startup(page).getByRole("button", { name: "重新读取启动档案", exact: true }).click();
  await expect(startup(page).getByRole("alert")).toContainText("隔离启动文件被占用");
  await expect(startup(page).getByRole("combobox", { name: "启动档案", exact: true })).toContainText("隔离档案");
  await expect(availableAction(page, "启动档案")).toBeEnabled();
  await expect(approval(await openConfigurationReview(page, "启动档案"))).toBeVisible();
  expect(await calls(page, "set_startup_profile")).toHaveLength(0);
});

test("后台联动的SVG与供电表在深浅主题和窄窗口保持模块间距", async ({ page }, testInfo) => {
  await page.goto("/"); await openWorkspace(page, "自动策略");
  await choose(page, startup(page), "启动档案");
  const review = await openConfigurationReview(page, "启动档案");
  for (const [width, theme] of [[1440, "light"], [700, "dark"]]) {
    await page.setViewportSize({ width, height: 1100 });
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    const region = approval(review);
    await region.scrollIntoViewIfNeeded();
    const geometry = await region.evaluate(element => {
      const columns = element.querySelector(".ccc-mode-gpu-columns"), parts = [...columns.children];
      return { gap: parseFloat(getComputedStyle(columns).gap), borders: parts.every(part => parseFloat(getComputedStyle(part).borderTopWidth) > 0), overflow: document.documentElement.scrollWidth > innerWidth };
    });
    expect(geometry.gap).toBeGreaterThan(0); expect(geometry.borders).toBe(true); expect(geometry.overflow).toBe(false);
    await region.screenshot({ path: testInfo.outputPath(`background-grant-${theme}-${width}.png`) });
  }
});
