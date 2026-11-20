import { openProfileResults, openProfileSave, openStartupParameters, saveProfileButton, showProfileSettings } from "./profile-review.mjs";
import { availableAction, configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openWorkspace, openPowerArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const navigate = (page, title) => page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: title, exact: true }).click();
const preview = page => page.getByRole("region", { name: "档案预览", exact: true });
const mutations = page => page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply_|save_|start_fan_dust|import_|set_startup)/.test(c.command)));
async function selectProfile(page) {
  await openPowerArea(page, "配置档案");
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await showProfileSettings(page);
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const current = { weekdays: 65, hour: 23, onResume: true };
    window.__uiFixture.dust = { revision: "isolated-dust-0", running: false, current, initial: null, softwareActive: false,
      lastStart: null, startBlockedReason: null, scheduleBlockedReason: null };
    window.__uiFixture.profile.settings.dustSchedule = structuredClone(current);
  });
  await page.goto("/"); await openWorkspace(page, "性能与散热");
});

test("除尘档案以星期和唤醒语义预览，确认仅提交计划目标", async ({ page }) => {
  await selectProfile(page);
  const card = preview(page).getByRole("region", { name: "档案除尘计划" });
  await expect(card).toContainText("周六、周日 23:00");
  await expect(card.getByRole("listitem", { name: "周六 执行", exact: true })).toHaveAttribute("data-selected", "true");
  await expect(card.getByRole("listitem", { name: "周一 不执行", exact: true })).toHaveAttribute("data-selected", "false");
  await expect(card).toContainText("仅配置计划，不立即启动除尘");
  expect(await mutations(page)).toEqual([]);
  await previewConfiguration(page);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  const calls = await mutations(page);
  expect(calls).toHaveLength(1);
  expect(calls[0].command).toBe("apply_profile");
  expect(calls[0].args.expected.settings.dustSchedule).toEqual({ weekdays: 65, hour: 23, onResume: true });
});

test("保存当前捕获除尘计划，未报告初态明确失败且不生成默认计划", async ({ page }) => {
  await openPowerArea(page, "配置档案");
  await openProfileSave(page, "除尘档案", ["设备功能"]);
  await saveProfileButton(page).click();
  await showProfileSettings(page);
  await expect(preview(page).getByLabel("档案除尘计划")).toContainText("23:00");
  const saved = (await mutations(page))[0];
  expect(saved.args.args.settings.dustSchedule.weekdays).toBe(65);
  await page.evaluate(() => { window.__uiFixture.dust.current = null; });
  await openProfileSave(page, "未知计划", ["设备功能"]);
  await expect(saveProfileButton(page)).toBeDisabled();
  await expect(page.getByRole("alert")).toContainText("原厂未报告除尘计划");
  expect(await mutations(page)).toHaveLength(1);
});

test("关闭计划明确展示全部触发关闭，不伪造进度或立即执行", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.profile.settings.dustSchedule = { weekdays: 0, hour: 0, onResume: false }; });
  await selectProfile(page);
  const card = preview(page).getByLabel("档案除尘计划");
  await expect(card).toContainText("定时和唤醒触发均关闭");
  await expect(card.locator('[data-selected="true"]')).toHaveCount(0);
  await expect(card.getByRole("progressbar")).toHaveCount(0);
  expect(await mutations(page)).toEqual([]);
});

test("启动确认发现文件被外部修改后拒绝保存，重选后预览新计划", async ({ page }) => {
  await openPowerArea(page, "电源设置");
  const panel = page.getByRole("region", { name: "启动档案", exact: true });
  const select = panel.getByRole("combobox", { name: "启动档案", exact: true });
  await select.click(); await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  const parameters = await openStartupParameters(page);
  await expect(parameters.getByLabel("档案除尘计划")).toContainText("23:00");
  await page.keyboard.press("Escape");
  await page.evaluate(() => { window.__uiFixture.profile.settings.dustSchedule.hour = 8; });
  await configurationView(page).getByRole("button", { name: "授权启动应用档案", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("启动档案已变化");
  expect(await page.evaluate(() => window.__uiFixture.startupSelection ?? null)).toBeNull();
  await select.click(); await page.getByRole("option", { name: "不应用档案", exact: true }).click();
  await select.click(); await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  const updated = await openStartupParameters(page);
  await expect(updated.getByLabel("档案除尘计划")).toContainText("08:00");
  await page.keyboard.press("Escape");
  await expect(availableAction(page, "启动档案")).toBeVisible();
});

test("除尘恢复失败显示具体阶段，保留用户可操作诊断", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.profileSteps = [
    { step: "除尘计划", ok: true, message: "已确认" }, { step: "软件风扇曲线", ok: false, message: "隔离监护失败" },
    { step: "恢复风扇", ok: true, message: "已确认" }, { step: "恢复除尘计划", ok: false, message: "原能力已变化，快照保留" }]; });
  await selectProfile(page);
  await previewConfiguration(page);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("部分设置恢复失败");
  const results = await openProfileResults(page);
  await expect(results).toContainText("原能力已变化，快照保留");
});

test("星期卡片在360至1440像素保留模块间隔且不溢出", async ({ page }) => {
  await selectProfile(page);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 1200 });
      const card = preview(page).getByLabel("档案除尘计划");
      await card.scrollIntoViewIfNeeded();
      expect(await card.evaluate(el => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
      await card.screenshot({ path: `target/ui-check/profile-dust-${theme}-${width}.png` });
    }
  }
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
