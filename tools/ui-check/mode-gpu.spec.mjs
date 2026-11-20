import { availableAction } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { modeGpuFixture } from "./mode-gpu-fixture.mjs";

const modes = page => page.getByRole("region", { name: "预设模式", exact: true });
const entertainment = page => modes(page).getByRole("radio", { name: /^娱乐/ });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
async function setup(page, options = {}) {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(({ fixture, options }) => {
    const data = window.__uiFixture;
    if (options.off) Object.assign(data.gpuPower.devices[0], { enabled: false, problem: 22, drivesDisplay: false, canDisable: true });
    if (options.deferred) {
      fixture.value.clocks.driver = null; fixture.value.clocks.candidate.projected = null;
      fixture.value.clocks.deferredReason = "独显已关闭；驱动范围和实际频率在已确认模式事务的写前检查点核对";
      data.modeApplyFault = "切换时驱动不可用，已恢复原独显节点";
    }
    if (options.driverError) fixture.value.clocks.driverError = "隔离驱动范围已变化";
    data.modeGpuOverrides = { 3: fixture };
  }, { fixture: modeGpuFixture(), options });
  await page.goto("/"); await openWorkspace(page, "性能与散热");
  await expect(entertainment(page)).toBeEnabled();
}

test("直接切换发送完整GPU目标与修订", async ({ page }) => {
  await setup(page);
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(0);
  await entertainment(page).click();
  await expect(entertainment(page)).toBeChecked();
  const requests = await calls(page, "apply_oem_power_mode");
  expect(requests).toHaveLength(1);
  const expected = requests[0].args.expected;
  expect(expected.fromMode).toBe(2); expect(expected.revision).toBeTruthy();
  expect(expected.gpu.status).toBe("ready"); expect(expected.gpu.value.clocks.driver.current).toEqual(modeGpuFixture().value.clocks.driver.current);
});

test("关闭独显的延后检查不伪造频率，失败后保留重试", async ({ page }) => {
  await setup(page, { off: true, deferred: true });
  await entertainment(page).click();
  await expect(modes(page).getByRole("alert")).toContainText("切换时驱动不可用，已恢复原独显节点");
  const { gpu } = (await calls(page, "apply_oem_power_mode"))[0].args.expected;
  expect(gpu.value.action).toMatchObject({ stage: "beforeMode", target: { enabled: true } });
  expect(gpu.value.clocks.driver).toBeNull();
  expect(await page.evaluate(() => window.__uiFixture.gpuPower.devices[0].enabled)).toBe(false);
  expect(await page.evaluate(() => window.__uiFixture.modeInitial)).toBeNull();
  await expect(entertainment(page)).toBeEnabled();
});

test("已知驱动错误时点卡片显示原因，未发出写入", async ({ page }) => {
  await setup(page, { driverError: true });
  await entertainment(page).click();
  await expect(modes(page).getByRole("alert")).toContainText("隔离驱动范围已变化");
  await expect(availableAction(page, "预设模式")).toHaveCount(0);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(0);
});

test("联动条件突变拒绝旧目标，回读后可再点卡片", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => { window.__uiFixture.acMode = 3; });
  await entertainment(page).click();
  await expect(modes(page).getByRole("alert")).toContainText("模式联动条件已变化");
  expect(await page.evaluate(() => window.__uiFixture.performanceOptions.currentMode)).toBe(2);
  expect(await page.evaluate(() => window.__uiFixture.modeInitial)).toBeNull();
  await entertainment(page).click();
  await expect(entertainment(page)).toBeChecked();
  await expect(modes(page).getByRole("alert")).toHaveCount(0);
});
