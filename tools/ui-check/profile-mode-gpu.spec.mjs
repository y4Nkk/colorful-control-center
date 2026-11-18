import { openProfileResults } from "./profile-review.mjs";
import { configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { modeGpuFixture } from "./mode-gpu-fixture.mjs";

const panel = page => page.getByRole("region", { name: "档案预览", exact: true });
const gpu = page => panel(page).getByRole("region", { name: "模式GPU联动预览", exact: true });
const calls = page => page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "apply_profile"));

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(fixture => {
    const data = window.__uiFixture, node = data.gpuPower.devices[0];
    Object.assign(node, { enabled: false, automaticPowerOff: true, problem: 22, drivesDisplay: false, canDisable: true });
    fixture.value.action = { stage: "afterMode", target: { enabled: false, automaticPowerOff: true } };
    fixture.value.clocks.candidate.decision.target.lockMhz = 900;
    fixture.value.clocks.candidate.projected.locks[0].value = 900000;
    data.profileModeAfterEnable = structuredClone({ driver: fixture.value.clocks.driver, candidate: fixture.value.clocks.candidate });
    fixture.value.clocks.driver = null;
    fixture.value.clocks.deferredReason = "独显已关闭，开启后核对GPU驱动和原始频率";
    fixture.value.clocks.candidate.projected = null;
    data.modeGpuOverrides = { 1: fixture };
    data.profile.settings.oemPowerMode = 1;
    data.profile.settings.gpuPower = { id: node.id, name: node.name, values: { enabled: true, automaticPowerOff: false } };
    data.profile.settings.gpu = { coreOffsetMhz: 60, memOffsetMhz: null };
  }, modeGpuFixture());
  await page.goto("/"); await openWorkspace(page, "性能调校");
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: "性能档案", exact: true }).click();
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
});

test("档案预览展示三阶段节点与延后频率，两次确认携带完整目标", async ({ page }) => {
  await expect(gpu(page).getByText("开启后核对频率", { exact: true })).toBeVisible();
  const steps = gpu(page).getByRole("list", { name: "独显联动步骤", exact: true }).getByRole("listitem");
  await expect(steps).toHaveCount(3);
  await expect(steps.nth(0)).toContainText("准备开启");
  await expect(steps.nth(1)).toContainText("切换后关闭");
  await expect(steps.nth(2)).toContainText("最终开启");
  expect(await calls(page)).toHaveLength(0);
  await openConfigurationReview(page);
  expect(await calls(page)).toHaveLength(0);
  await configurationDialog(page).getByRole("button", { name: "应用档案", exact: true }).click();
  const results = await openProfileResults(page);
  await expect(results).toContainText("GPU 偏移");
  const [call] = await calls(page), graph = call.args.expectedMode.gpu.value.stages;
  expect([graph.preparation.target.enabled, graph.afterMode.target.enabled, graph.finish.target.enabled]).toEqual([true, false, true]);
  expect(graph.preparation.target.automaticPowerOff).toBe(true);
  expect(graph.finish.target.automaticPowerOff).toBe(false);
  const state = await page.evaluate(() => ({ authorizations: window.__uiFixture.profileGpuAuthorizations, node: window.__uiFixture.gpuPower.devices[0], core: window.__uiFixture.gpu.coreOffsetMhz }));
  expect(state.authorizations).toBe(1);
  expect(state.node.enabled).toBe(true); expect(state.node.automaticPowerOff).toBe(false); expect(state.core).toBe(60);
});

test("确认前模式前态变化拒绝整个旧档案目标", async ({ page }) => {
  await expect(gpu(page)).toBeVisible();
  await openConfigurationReview(page);
  await page.evaluate(() => { window.__uiFixture.performanceOptions.currentMode = 3; });
  await configurationDialog(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(page.getByRole("alert").filter({ hasText: "模式联动已变化" })).toBeVisible();
  expect(await page.evaluate(() => window.__uiFixture.profileGpuAuthorizations ?? 0)).toBe(0);
  expect(await page.evaluate(() => window.__uiFixture.gpuPower.devices[0].enabled)).toBe(false);
});

test("档案三阶段GPU图示在深浅与窄窗口保留模块边框", async ({ page }, testInfo) => {
  for (const [width, theme] of [[1440, "light"], [700, "dark"]]) {
    await page.setViewportSize({ width, height: 1100 });
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    await gpu(page).scrollIntoViewIfNeeded();
    const cards = gpu(page).locator(":scope > .ccc-mode-gpu-part");
    await expect(cards).toHaveCount(2);
    expect(await cards.evaluateAll(cards => cards.every(card => parseFloat(getComputedStyle(card).borderTopWidth) > 0))).toBeTruthy();
    await gpu(page).screenshot({ path: testInfo.outputPath(`profile-mode-gpu-${theme}-${width}.png`) });
  }
});
