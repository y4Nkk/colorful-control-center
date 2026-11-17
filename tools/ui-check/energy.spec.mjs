import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "电池与节能"); });
const panelOf = page => page.getByRole("region", { name: "电池节能计划", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => c.command === "apply_energy_policy").length);
const choose = async (page, panel, label, value) => {
  await panel.getByRole("combobox", { name: label, exact: true }).click();
  await page.getByRole("option", { name: value, exact: true }).click();
};
const apply = async panel => {
  await panel.getByRole("button", { name: "应用节能计划", exact: true }).click();
  await panel.getByRole("button", { name: "确认发送并保存节能计划？", exact: true }).click();
};

test("星期与时段仅修改草稿，确认绑定当前计划", async ({ page }) => {
  const panel = panelOf(page);
  await panel.getByRole("switch", { name: "启用节能计划", exact: true }).check();
  await panel.getByRole("switch", { name: "每天执行节能时段", exact: true }).uncheck();
  await panel.getByRole("checkbox", { name: "节能星期一", exact: true }).check();
  expect(await writes(page)).toBe(0);
  await panel.getByRole("button", { name: "应用节能计划", exact: true }).click();
  await choose(page, panel, "时段 1 开始", "09:00");
  await expect(panel.getByRole("button", { name: "应用节能计划", exact: true })).toBeVisible();
  expect(await writes(page)).toBe(0);
  await apply(panel);
  await expect(panel.getByRole("button", { name: "重新发送保存计划", exact: true })).toBeVisible();
  const saved = await page.evaluate(() => window.__uiFixture.energy.saved);
  expect(saved.weekdays).toBe(2); expect(saved.everyDay).toBe(false); expect(saved.slots[0]).toEqual({ start: 9, end: 18 });
  await expect(page.getByRole("region", { name: "电池与充电", exact: true }).getByRole("switch", { name: "限制充电范围", exact: true })).toHaveAttribute("aria-disabled", "true");
  await page.setViewportSize({ width: 1000, height: 1100 });
  await panel.screenshot({ path: "target/ui-check/energy-policy.png" });
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("重叠时段拒绝，停止与初始恢复均需确认", async ({ page }) => {
  const panel = panelOf(page);
  await panel.getByRole("switch", { name: "启用节能计划", exact: true }).check();
  await panel.getByRole("switch", { name: "启用节能时段 2", exact: true }).check();
  await choose(page, panel, "时段 2 开始", "17:00");
  await expect(panel).toContainText("两个节能时段不能重叠");
  await expect(panel.getByRole("button", { name: "应用节能计划", exact: true })).toBeDisabled();
  await choose(page, panel, "时段 2 开始", "20:00");
  await apply(panel);
  await choose(page, panel, "时段 1 开始", "09:00");
  await panel.getByRole("button", { name: "停止节能", exact: true }).click();
  expect(await writes(page)).toBe(1);
  await panel.getByRole("button", { name: "停止节能时段并恢复原充电策略？", exact: true }).click();
  await expect(panel.getByRole("switch", { name: "启用节能计划", exact: true })).not.toBeChecked();
  await expect(panel.getByRole("combobox", { name: "时段 1 开始", exact: true })).toHaveText("09:00");
  expect(await page.evaluate(() => window.__uiFixture.energy.saved.slots[0].start)).toBe(8);
  expect(await page.evaluate(() => window.__uiFixture.battery.charge.current)).toEqual({ enabled: false, start: 95, stop: 100 });
  await panel.getByRole("button", { name: "恢复节能初始", exact: true }).click();
  await panel.getByRole("button", { name: "恢复首次接管前的节能与充电策略？", exact: true }).click();
  await expect(panel.getByRole("switch", { name: "启用节能时段 2", exact: true })).not.toBeChecked();
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("失败保留草稿，补偿未完成时只开放恢复", async ({ page }) => {
  const panel = panelOf(page);
  await panel.getByRole("switch", { name: "启用节能计划", exact: true }).check();
  await page.evaluate(() => { window.__uiFixture.energyFault = "隔离计划发送失败，充电恢复待重试"; window.__uiFixture.energyRecoveryFault = true; });
  await apply(panel);
  await expect(panel.getByRole("alert")).toContainText("隔离计划发送失败");
  await expect(panel.getByRole("switch", { name: "启用节能计划", exact: true })).toBeChecked();
  await expect(panel.getByRole("button", { name: "应用节能计划", exact: true })).toBeDisabled();
  await expect(panel.getByRole("button", { name: "恢复节能初始", exact: true })).toBeEnabled();
  await panel.getByRole("button", { name: "恢复节能初始", exact: true }).click();
  await panel.getByRole("button", { name: "恢复首次接管前的节能与充电策略？", exact: true }).click();
  await expect(panel.getByRole("alert")).toHaveCount(0);
});

test("手动阈值与窄屏保留有效语义，无电池不能发送", async ({ page }) => {
  const panel = panelOf(page);
  await panel.getByRole("switch", { name: "启用节能计划", exact: true }).check();
  await choose(page, panel, "节能阈值方式", "自定义阈值");
  await choose(page, panel, "节能放电下限", "70%");
  await choose(page, panel, "节能充电上限", "90%");
  await page.setViewportSize({ width: 360, height: 720 });
  expect(await panel.evaluate(el => el.getBoundingClientRect().right <= innerWidth)).toBe(true);
  await panel.screenshot({ path: "target/ui-check/energy-policy-narrow.png" });
  await page.evaluate(() => { window.__uiFixture.battery.power.present = false; });
  await panel.getByRole("button", { name: "重新读取节能计划", exact: true }).click();
  await expect(panel.getByRole("button", { name: "重新发送保存计划", exact: true })).toBeDisabled();
  expect(await writes(page)).toBe(0);
});

test("首次使用不伪造已有计划，恢复明确清除本应用时段", async ({ page }) => {
  const panel = panelOf(page);
  await page.evaluate(() => { Object.assign(window.__uiFixture.energy, { saved: null, source: "unconfigured", hasInitialPlan: false }); });
  await panel.getByRole("button", { name: "重新读取节能计划", exact: true }).click();
  await expect(panel).toContainText("保存计划：未保存计划");
  await panel.getByText("计划来源与恢复目标", { exact: true }).click();
  await expect(panel).toContainText("清除本应用时段");
  await panel.getByRole("switch", { name: "启用节能计划", exact: true }).check();
  await panel.getByRole("switch", { name: "启用节能时段 1", exact: true }).check();
  await apply(panel);
  await panel.getByRole("button", { name: "恢复节能初始", exact: true }).click();
  await panel.getByRole("button", { name: "清除本应用时段并恢复初始充电策略？", exact: true }).click();
  await expect(panel).toContainText("保存计划：未保存计划");
  expect(await page.evaluate(() => window.__uiFixture.energy.saved)).toBeNull();
});

test("未声明节能能力时不显示可操作入口", async ({ page }) => {
  const panel = panelOf(page);
  await expect(panel.getByRole("button", { name: "重新发送保存计划", exact: true })).toBeVisible();
  await expect(panel.getByRole("button", { name: "重新读取节能计划", exact: true })).toBeEnabled();
  await page.evaluate(() => { window.__uiFixture.energySupported = false; });
  await panel.getByRole("button", { name: "重新读取节能计划", exact: true }).click();
  await expect(panel).toHaveCount(0);
  await expect(page.getByRole("region", { name: "电池与充电", exact: true })).toBeVisible();
  expect(await writes(page)).toBe(0);
});
