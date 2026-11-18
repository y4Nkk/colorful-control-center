import { availableAction, configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panelOf = page => page.getByRole("region", { name: "风扇除尘", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => /^(start_fan_dust|apply_fan_dust_schedule|restore_fan_dust_schedule)$/.test(c.command)));
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    window.__uiFixture.dust = { revision: "dust-0", running: false, current: null, initial: null, softwareActive: false,
      lastStart: null, startBlockedReason: null, scheduleBlockedReason: null };
  });
  await page.goto("/"); await openWorkspace(page, "风扇散热");
});
test("除尘启动预览确认，只报告请求和固件状态，不伪造停止或进度", async ({ page }) => {
  const panel = panelOf(page);
  await expect(panel).toContainText("当前未运行"); await expect(panel).toContainText("原厂未保存计划");
  await panel.getByRole("button", { name: "立即除尘", exact: true }).click(); expect(await writes(page)).toHaveLength(0);
  await panel.getByRole("button", { name: "确认开始一次除尘？", exact: true }).click();
  expect(await writes(page)).toHaveLength(1); await expect(panel).toContainText("当前尚未报告运行");
  await page.evaluate(() => { window.__uiFixture.dust.running = true; window.__uiFixture.dust.lastStart = "running"; });
  await panel.getByRole("button", { name: "重新读取除尘状态" }).click();
  await expect(panel.getByRole("button", { name: "立即除尘", exact: true })).toBeDisabled();
  const fan = page.getByRole("region", { name: "风扇曲线", exact: true });
  await expect(fan).toContainText("固件正在除尘");
  await expect(availableAction(page, "风扇设置", true)).toHaveCount(0);
  await expect(panel.getByRole("button", { name: /停止|取消除尘/ })).toHaveCount(0);
  await expect(panel.getByRole("progressbar")).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.dust.running = false; window.__uiFixture.dust.lastStart = "completed"; });
  await panel.getByRole("button", { name: "重新读取除尘状态" }).click();
  await expect(panel).toContainText("已观察到本次除尘结束");
});
test("星期整点与唤醒计划正确编码，确认后保存并恢复未报告初态", async ({ page }) => {
  const panel = panelOf(page);
  await panel.getByRole("switch", { name: "按星期除尘", exact: true }).click();
  await panel.getByRole("checkbox", { name: "周日", exact: true }).uncheck();
  await panel.getByRole("switch", { name: "系统唤醒后除尘", exact: true }).click();
  await panel.getByRole("combobox", { name: "除尘执行时间" }).click();
  await page.getByRole("option", { name: "23:00", exact: true }).click();
  await openConfigurationReview(page, "除尘计划"); expect(await writes(page)).toHaveLength(0);
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  expect((await writes(page))[0].args.request.schedule).toEqual({ weekdays: 126, hour: 23, onResume: true });
  await expect(panel).toContainText("恢复将关闭本次除尘触发");
  await openConfigurationReview(page, "除尘计划", true); expect(await writes(page)).toHaveLength(1);
  await configurationDialog(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(panel.getByRole("switch", { name: "按星期除尘", exact: true })).not.toBeChecked();
  expect(await writes(page)).toHaveLength(2);
});
test("草稿编辑与外部事实变化均撤销确认，后台刷新保留草稿", async ({ page }) => {
  const panel = panelOf(page);
  await panel.getByRole("switch", { name: "按星期除尘", exact: true }).click();
  await openConfigurationReview(page, "除尘计划");
  await panel.getByRole("checkbox", { name: "周一", exact: true }).uncheck();
  await expect(availableAction(page, "除尘计划")).toBeVisible();
  await openConfigurationReview(page, "除尘计划");
  await page.evaluate(() => { window.__uiFixture.dust.revision = "changed"; });
  await panel.getByRole("button", { name: "重新读取除尘状态" }).click();
  await expect(availableAction(page, "除尘计划")).toBeVisible();
  await expect(panel.getByRole("checkbox", { name: "周一", exact: true })).not.toBeChecked();
  expect(await writes(page)).toHaveLength(0);
});
test("高占空比、软件曲线及校准互斥在界面明确显示", async ({ page }) => {
  const panel = panelOf(page);
  await page.evaluate(() => { window.__uiFixture.dust.startBlockedReason = "风扇占空比超过50%"; });
  await panel.getByRole("button", { name: "重新读取除尘状态" }).click();
  await expect(panel.getByRole("button", { name: "立即除尘", exact: true })).toBeDisabled();
  await expect(panel).toContainText("超过50%");
  await page.evaluate(() => { window.__uiFixture.fan.software = { cpu: { points: [{ tempC:40,dutyPct:30 },{ tempC:90,dutyPct:100 }], hysteresisC:3, rampPctPerS:10 } }; });
  await panel.getByRole("button", { name: "重新读取除尘状态" }).click();
  await panel.getByRole("switch", { name: "按星期除尘", exact: true }).click();
  await expect(availableAction(page, "除尘计划")).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.calibration.recoveryRequired = true; });
  await panel.getByRole("button", { name: "重新读取除尘状态" }).click();
  await expect(panel).toContainText("电池校准正在接管");
  expect(await writes(page)).toHaveLength(0);
});
test("失败保留草稿和恢复入口，读取失败撤销执行快照", async ({ page }) => {
  const panel = panelOf(page);
  await panel.getByRole("switch", { name: "系统唤醒后除尘", exact: true }).click();
  await page.evaluate(() => { window.__uiFixture.dustFault = "隔离计划失败，恢复尚需重试"; });
  await openConfigurationReview(page, "除尘计划");
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("恢复尚需重试");
  await expect(panel.getByRole("switch", { name: "系统唤醒后除尘", exact: true })).toBeChecked();
  await expect(availableAction(page, "除尘计划", true)).toBeEnabled();
  await page.evaluate(() => { window.__uiFixture.dustReadFault = "隔离除尘读取失败"; });
  await panel.getByRole("button", { name: "重新读取除尘状态" }).click();
  await expect(panel.getByRole("button", { name: "立即除尘", exact: true })).toHaveCount(0);
});
test("左右卡片窄屏回流且无能力时隐藏", async ({ page }) => {
  const panel = panelOf(page);
  await panel.getByRole("switch", { name: "按星期除尘", exact: true }).click();
  for (const width of [1440,760,360]) {
    await page.setViewportSize({ width, height: 1000 });
    await expect(panel.getByRole("combobox", { name: "除尘执行时间" })).toBeVisible();
    expect(await panel.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  }
  await page.setViewportSize({ width: 1000, height: 1000 }); await panel.screenshot({ path: "target/ui-check/fan-dust.png" });
  await page.evaluate(() => { window.__uiFixture.dust = null; });
  await panel.getByRole("button", { name: "重新读取除尘状态" }).click(); await expect(panel).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("原厂除尘计划未明确关闭时显示软件曲线冲突，关闭后允许接管", async ({ page }) => {
  const panel=panelOf(page), fan=page.getByRole("region",{name:"风扇曲线",exact:true});
  await fan.getByRole("radio", {name:"软件曲线",exact:true}).check();
  await expect(fan).toContainText("请先明确关闭定时及唤醒除尘");
  await expect(fan.getByRole("button", {name:"启用软件曲线",exact:true})).toBeDisabled();
  await panel.getByRole("button", {name:"应用除尘计划",exact:true}).click();
  await panel.getByRole("button", {name:"确认应用除尘计划？",exact:true}).click();
  await expect(fan.getByRole("button", {name:"启用软件曲线",exact:true})).toBeEnabled();
});
