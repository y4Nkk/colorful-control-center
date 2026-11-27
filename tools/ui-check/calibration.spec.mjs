import { openWorkspace, openPowerArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => { window.__uiFixture.calibrationSupported = true; });
  await page.goto("/"); await openPowerArea(page, "电源设置");
});
const panelOf = page => page.getByRole("region", { name: "电池校准", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => /^(start|stop|restore)_battery_calibration$/.test(c.command)));
const openRestore = async page => {
  await panelOf(page).getByRole("button", { name: "恢复电池校准", exact: true }).click();
  return page.getByRole("dialog", { name: "恢复电池校准", exact: true });
};
const start = async panel => {
  await panel.getByRole("button", { name: "开始电池校准", exact: true }).click();
  await panel.getByRole("button", { name: "开始校准", exact: true }).click();
};

test("开始前预览确认，运行期间充电和节能互斥，切页保留任务", async ({ page }) => {
  const panel = panelOf(page);
  await panel.getByRole("button", { name: "开始电池校准", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "开始电池校准", exact: true })).toContainText("两轮充放电");
  expect(await writes(page)).toHaveLength(0);
  await panel.getByRole("button", { name: "开始校准", exact: true }).click();
  await expect(panel.getByRole("status")).toHaveText("正在校准");
  const battery = page.getByRole("region", { name: "电池与充电", exact: true });
  await expect(battery.getByRole("switch", { name: "限制充电范围", exact: true })).toHaveAttribute("aria-disabled", "true");
  await expect(page.getByRole("switch", { name: "启用节能计划", exact: true })).toHaveAttribute("aria-disabled", "true");
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await page.getByRole("button", { name: "电源与档案", exact: true }).click();
  await expect(panel.getByRole("status")).toHaveText("正在校准");
  expect((await writes(page)).map(c => c.command)).toEqual(["start_battery_calibration"]);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("真实暂停与耗时独立显示，停止仍需确认", async ({ page }) => {
  const panel = panelOf(page);
  await start(panel);
  await page.evaluate(() => {
    const task = window.__uiFixture.calibration.task;
    task.status = "paused"; task.phase = "charge1"; task.elapsedMs = 3_661_000; task.phaseElapsedMs = 61_000;
  });
  await panel.getByRole("button", { name: "重新读取校准", exact: true }).click();
  await expect(panel.getByRole("status")).toHaveText("已暂停，等待接入电源");
  await expect(panel).toContainText("01:01:01");
  await panel.getByRole("button", { name: "停止校准", exact: true }).click();
  expect(await writes(page)).toHaveLength(1);
  await panel.getByRole("button", { name: "停止并恢复", exact: true }).click();
  await expect(panel.getByRole("status")).toHaveText("已停止，已恢复策略");
  await expect(panel.getByRole("button", { name: "开始电池校准", exact: true })).toBeEnabled();
});

test("失败保留诊断和恢复入口，恢复需再次确认", async ({ page }) => {
  const panel = panelOf(page);
  await page.evaluate(() => { window.__uiFixture.calibrationFault = "隔离校准写入失败，原策略尚需恢复"; });
  await start(panel);
  await expect(panel).toContainText("上次校准尚需恢复");
  await expect(panel.getByRole("button", { name: "开始电池校准", exact: true })).toBeDisabled();
  const restore = await openRestore(page);
  expect(await writes(page)).toHaveLength(1);
  await expect(restore).toContainText("恢复开始前计划");
  await restore.getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(panel.getByRole("button", { name: "开始电池校准", exact: true })).toBeEnabled();
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("旧预览刷新后撤销确认，窄屏卡片不溢出", async ({ page }) => {
  const panel = panelOf(page);
  await panel.getByRole("button", { name: "开始电池校准", exact: true }).click();
  await page.evaluate(() => {
    const cal = window.__uiFixture.calibration;
    cal.revision = "calibration-new"; cal.knownEnergyPlan = false; cal.restoreEnergyEnabled = null;
  });
  await expect(page.getByRole("dialog", { name: "开始电池校准", exact: true })).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "开始电池校准", exact: true })).toBeVisible();
  expect(await writes(page)).toHaveLength(0);
  await page.setViewportSize({ width: 760, height: 1100 });
  await panel.screenshot({ path: "target/ui-check/battery-calibration.png" });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test("未满足开始条件和不支持机型保留明确边界", async ({ page }) => {
  const panel = panelOf(page);
  await expect(panel.getByRole("button", { name: "开始电池校准", exact: true })).toBeEnabled();
  await page.evaluate(() => { const cal = window.__uiFixture.calibration; cal.canStart = false; cal.unavailableReason = "请接入电源后重新预览校准"; });
  await panel.getByRole("button", { name: "重新读取校准", exact: true }).click();
  await expect(panel).toContainText("请接入电源");
  await expect(panel.getByRole("button", { name: "开始电池校准", exact: true })).toBeDisabled();
  await page.evaluate(() => { window.__uiFixture.calibrationSupported = false; });
  await panel.getByRole("button", { name: "重新读取校准", exact: true }).click();
  await expect(panel.getByRole("status")).toHaveText("当前设备未提供电池校准。");
  await expect(panel.getByRole("button", { name: "开始电池校准", exact: true })).toHaveCount(0);
  await expect(page.getByRole("switch", { name: "限制充电范围", exact: true })).not.toHaveAttribute("aria-disabled", "true");
  expect(await writes(page)).toHaveLength(0);
});

test("未接管的中断只解除准备记录，确认不声称硬件恢复", async ({ page }) => {
  const panel = panelOf(page);
  await expect(panel.getByRole("button", { name: "开始电池校准", exact: true })).toBeEnabled();
  await page.evaluate(() => {
    const cal = window.__uiFixture.calibration;
    cal.canStart = false; cal.recoveryRequired = true; cal.controlAcquired = false;
  });
  await panel.getByRole("button", { name: "重新读取校准", exact: true }).click();
  await expect(panel.getByRole("status")).toHaveText("需要解除准备记录");
  const restore = await openRestore(page);
  await expect(restore).toContainText("解除未接管的校准准备记录");
  expect(await writes(page)).toHaveLength(0);
  await restore.getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(panel.getByRole("button", { name: "开始电池校准", exact: true })).toBeEnabled();
  expect((await writes(page)).map(c => c.command)).toEqual(["restore_battery_calibration"]);
});

test("校准取得和释放控制时，同步锁定模式与性能选项", async ({ page }) => {
  const panel = panelOf(page), modes = page.getByRole("region", { name: "预设模式", exact: true });
  const options = page.getByRole("region", { name: "散热控制", exact: true });
  await start(panel);
  await openWorkspace(page, "性能与散热");
  await expect(modes).toContainText("电池校准正在接管或尚需恢复");
  await expect(modes.getByRole("radio").first()).toBeDisabled();
  await expect(options.getByRole("switch", { name: "Turbo 风扇", exact: true })).toHaveAttribute("aria-disabled", "true");
  await openPowerArea(page, "电源设置");
  await panel.getByRole("button", { name: "停止校准", exact: true }).click();
  await panel.getByRole("button", { name: "停止并恢复", exact: true }).click();
  await openWorkspace(page, "性能与散热");
  await expect(modes.getByRole("radio").first()).toBeEnabled();
  await expect(options.getByRole("switch", { name: "Turbo 风扇", exact: true })).not.toHaveAttribute("aria-disabled", "true");
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
