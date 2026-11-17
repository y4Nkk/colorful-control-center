import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { modeGpuFixture } from "./mode-gpu-fixture.mjs";

const modes = page => page.getByRole("region", { name: "预设模式", exact: true });
const gpuPreview = page => modes(page).getByRole("region", { name: "模式GPU联动预览", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
async function setup(page, options = {}) {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(({ fixture, options }) => {
    const data = window.__uiFixture;
    if (options.off) Object.assign(data.gpuPower.devices[0], { enabled: false, problem: 22, drivesDisplay: false, canDisable: true });
    if (options.deferred) {
      fixture.value.clocks.driver = null; fixture.value.clocks.candidate.projected = null;
      fixture.value.clocks.deferredReason = "独显已关闭；驱动范围和实际频率在已确认模式事务的写前检查点核对";
      data.modeApplyFault = "确认后驱动不可用，已恢复原独显节点";
    }
    if (options.driverError) fixture.value.clocks.driverError = "隔离驱动范围已变化";
    data.modeGpuOverrides = { 3: fixture };
  }, { fixture: modeGpuFixture(), options });
  await page.goto("/"); await openWorkspace(page, "性能调校");
  await expect(modes(page).getByRole("radio", { name: /日常均衡/ })).toBeEnabled();
  await modes(page).getByRole("radio", { name: /日常均衡/ }).click();
}
async function apply(page) {
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(0);
  await modes(page).getByRole("button", { name: "确认应用模式", exact: true }).click();
}

test("完整模式GPU左右预览、驱动范围、两步确认和完整初始恢复", async ({ page }) => {
  await setup(page);
  await expect(gpuPreview(page).getByText("当前GPU", { exact: true })).toBeVisible();
  await expect(gpuPreview(page).getByText("确认后的GPU目标", { exact: true })).toBeVisible();
  await gpuPreview(page).getByText("查看驱动范围", { exact: true }).click();
  await expect(gpuPreview(page).getByText(/可编辑/).first()).toBeVisible();
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(0);
  await apply(page);
  const expected = (await calls(page, "apply_oem_power_mode"))[0].args.expected;
  expect(expected.fromMode).toBe(2); expect(expected.revision).toBeTruthy();
  expect(expected.gpu.status).toBe("ready"); expect(expected.gpu.value.clocks.driver.current).toEqual(modeGpuFixture().value.clocks.driver.current);
  await modes(page).getByText("初始模式恢复预览", { exact: true }).click();
  const restore = modes(page).getByRole("region", { name: "GPU初始恢复目标", exact: true });
  await expect(restore.getByText("当前频率", { exact: true })).toBeVisible();
  await expect(restore.getByText("初始频率目标", { exact: true })).toBeVisible();
  await modes(page).getByRole("button", { name: "恢复初始", exact: true }).click();
  expect(await calls(page, "restore_oem_power_mode")).toHaveLength(0);
  await modes(page).getByRole("button", { name: "恢复以上初始模式、选项及关联设置？", exact: true }).click();
  await expect(restore).toHaveCount(0);
});

test("关闭独显的延后检查不伪造频率，失败后保留重试", async ({ page }) => {
  await setup(page, { off: true, deferred: true });
  const steps = gpuPreview(page).getByRole("list", { name: "独显联动步骤", exact: true });
  await expect(steps).toContainText("切换前");
  await expect(steps).toContainText("开启");
  await expect(gpuPreview(page).getByText("开启后核对频率", { exact: true })).toBeVisible();
  await gpuPreview(page).getByText("联动详情", { exact: true }).click();
  await expect(gpuPreview(page).getByText(/驱动范围和实际频率在已确认模式事务/)).toBeVisible();
  await apply(page);
  await expect(modes(page).getByText(/确认后驱动不可用，已恢复原独显节点/)).toBeVisible();
  expect(await page.evaluate(() => window.__uiFixture.gpuPower.devices[0].enabled)).toBe(false);
  expect(await page.evaluate(() => window.__uiFixture.modeInitial)).toBeNull();
  await expect(modes(page).getByRole("button", { name: "确认应用模式", exact: true })).toBeEnabled();
});

test("驱动错误禁用确认并保留已有GPU事实", async ({ page }) => {
  await setup(page, { driverError: true });
  await expect(gpuPreview(page).getByText("隔离驱动范围已变化", { exact: true })).toBeVisible();
  await expect(gpuPreview(page).getByText(/图形锁频：/).first()).toBeVisible();
  await expect(modes(page).getByRole("button", { name: "确认应用模式", exact: true })).toBeDisabled();
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(0);
});

test("确认前核心前态变化拒绝旧完整目标，重读后再确认", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => { window.__uiFixture.acMode = 3; });
  await modes(page).getByRole("button", { name: "确认应用模式", exact: true }).click();
  await expect(modes(page).getByText(/完整模式预览已变化/)).toBeVisible();
  expect(await page.evaluate(() => window.__uiFixture.performanceOptions.currentMode)).toBe(2);
  expect(await page.evaluate(() => window.__uiFixture.modeInitial)).toBeNull();
  await expect(modes(page).getByRole("button", { name: "确认应用模式", exact: true })).toBeEnabled();
});

test("GPU模块宽窄及深浅布局保留独立边框和间距", async ({ page }, testInfo) => {
  await setup(page);
  for (const [width, theme] of [[1440, "light"], [700, "dark"]]) {
    await page.setViewportSize({ width, height: 1100 });
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    await gpuPreview(page).scrollIntoViewIfNeeded();
    const cards = gpuPreview(page).locator(":scope > .ccc-mode-gpu-part");
    expect(await cards.count()).toBe(2);
    const boxes = await cards.evaluateAll(cards => cards.map(card => { const box = card.getBoundingClientRect(), style = getComputedStyle(card); return { x: box.x, y: box.y, width: box.width, border: parseFloat(style.borderTopWidth) }; }));
    expect(boxes.every(box => box.border > 0)).toBeTruthy();
    if (width === 1440) expect(boxes[1].x).toBeGreaterThan(boxes[0].x + boxes[0].width);
    else expect(boxes[1].y).toBeGreaterThan(boxes[0].y);
    await gpuPreview(page).screenshot({ path: testInfo.outputPath(`gpu-mode-${theme}-${width}.png`) });
  }
});
