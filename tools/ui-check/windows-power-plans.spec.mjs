import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openPowerArea } from "./workspace.mjs";

const HIGH = "22222222-2222-2222-2222-222222222222";
const ULTIMATE = "11111111-1111-1111-1111-111111111111";
const OFFICE = "55555555-5555-5555-5555-555555555555";
const section = page => page.getByRole("region", { name: "Windows 电源计划", exact: true });
const picker = page => section(page).getByRole("combobox", { name: "选择计划", exact: true });
const applyButton = page => section(page).getByRole("button", { name: "应用电源计划", exact: true });
const reread = page => section(page).getByRole("button", { name: "重新读取 Windows 电源计划", exact: true }).click();
const writes = page => page.evaluate(() => window.__uiFixture.calls
  .filter(call => call.command === "apply_windows_power_plan").map(call => call.args.request.targetId));

async function choose(page, name) {
  await picker(page).click();
  await page.getByRole("option", { name, exact: true }).click();
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await openPowerArea(page, "电源设置");
  await expect(picker(page)).toHaveText("高性能");
});

test("从系统列表选择、预览并应用计划", async ({ page }) => {
  await picker(page).click();
  for (const name of ["平衡", "卓越性能", "办公自定义"])
    await expect(page.getByRole("option", { name, exact: true })).toBeVisible();
  await expect(page.getByRole("option", { name: /高性能.*当前启用/ })).toHaveAttribute("aria-selected", "true");
  await page.getByRole("option", { name: "办公自定义", exact: true }).click();

  await expect(picker(page)).toHaveText("办公自定义");
  expect(await writes(page)).toEqual([]);
  const usbReads = () => page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "read_usb_policy").length);
  const before = await usbReads();
  await applyButton(page).click();
  await expect(applyButton(page)).toHaveCount(0);
  await expect(picker(page)).toHaveText("办公自定义");
  expect(await writes(page)).toEqual([OFFICE]);
  await expect.poll(usbReads).toBeGreaterThan(before);

  await expect(section(page).getByRole("button", { name: "恢复初始计划", exact: true })).toHaveCount(0);
  await choose(page, "高性能");
  await expect(applyButton(page)).toBeVisible();
  expect(await writes(page)).toEqual([OFFICE]);
  await applyButton(page).click();
  await expect(applyButton(page)).toHaveCount(0);
  await expect(picker(page)).toHaveText("高性能");
  expect(await writes(page)).toEqual([OFFICE, HIGH]);
});

test("窗口重新聚焦后同步外部计划和 USB 读数，过时预览保持失效", async ({ page }) => {
  await page.setViewportSize({ width: 610, height: 700 });
  await choose(page, "办公自定义");
  await expect(applyButton(page)).toBeVisible();
  const usb = page.getByRole("region", { name: "USB 选择性暂停", exact: true });
  await expect(usb.locator("dd")).toHaveText(["开启", "开启"]);
  await page.evaluate(id => {
    window.__uiFixture.windowsPlans.activeId = id;
    window.__uiFixture.usb.activeValues = { ac: false, dc: false };
    window.dispatchEvent(new Event("focus"));
  }, ULTIMATE);
  await expect(picker(page)).toHaveText("卓越性能");
  await expect(usb.locator("dd")).toHaveText(["关闭", "关闭"]);
  await expect(applyButton(page)).toHaveCount(0);
  await expect(section(page).getByText("电源计划已变化，请重新选择。")).toBeVisible();
  await page.evaluate(id => {
    window.__uiFixture.windowsPlans.activeId = id;
    window.dispatchEvent(new Event("focus"));
  }, HIGH);
  await expect(picker(page)).toHaveText("高性能");
  await expect(applyButton(page)).toHaveCount(0);
  expect(await writes(page)).toEqual([]);
});

test("Windows 写权限和电池校准分别限制切换", async ({ page }) => {
  const reason = "Windows 电源计划切换权限检查：组策略阻止了此操作 (1260)";
  await page.evaluate(message => { window.__uiFixture.windowsPlansBlockedReason = message; }, reason);
  await reread(page);
  await expect(picker(page)).toBeDisabled();
  await expect(picker(page)).toHaveText("高性能");
  await expect(section(page).getByText(reason, { exact: true })).toBeVisible();
  await page.evaluate(() => { window.__uiFixture.windowsPlansBlockedReason = null; });
  await reread(page);
  await expect(picker(page)).toBeEnabled();
  await page.evaluate(() => { window.__uiFixture.calibration.workerRunning = true; });
  await reread(page);
  await expect(picker(page)).toBeDisabled();
  await expect(section(page).getByText(/电池校准正在接管/)).toBeVisible();
  expect(await writes(page)).toEqual([]);
});

test("读取失败、单一计划和系统拒绝写入都有实际反馈", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.windowsPlansWriteFault = "Windows 拒绝激活电源计划"; });
  await choose(page, "平衡");
  await applyButton(page).click();
  await expect(section(page).getByRole("alert")).toHaveText("Windows 拒绝激活电源计划");
  await expect(picker(page)).toHaveText("平衡");
  await expect(applyButton(page)).toBeVisible();

  await page.evaluate(() => { window.__uiFixture.windowsPlansReadFault = "暂时无法读取 Windows 电源计划"; });
  await reread(page);
  await expect(section(page).getByRole("alert")).toHaveText("暂时无法读取 Windows 电源计划");
  await expect(picker(page)).toBeDisabled();
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.windowsPlansReadFault = null;
    data.windowsPlans.plans = data.windowsPlans.plans.filter(plan => plan.id === data.windowsPlans.activeId);
  });
  await reread(page);
  await expect(picker(page)).toHaveText("高性能");
  await expect(picker(page)).toBeDisabled();
  await expect(section(page).getByText("Windows 目前只提供一个电源计划。", { exact: true })).toBeVisible();
  await expect(section(page).getByRole("alert")).toHaveCount(0);
});
