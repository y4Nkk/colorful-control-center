import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const facts = page => page.getByRole("region", { name: "原厂低电量亮度事实与预览", exact: true });
const scope = page => panel(page).getByRole("checkbox", { name: "原厂低电量亮度", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
async function applied(page) {
  await scope(page).check();
  await expect(facts(page)).toBeVisible();
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, low = h.facts.batteryLowBrightness;
    const baseline = structuredClone(low.current);
    low.current.levels = [0, 0]; low.current.cache = [80, 20];
    low.settingTarget = [0, 0]; low.endingTarget = [80, 80];
    low.recovery = { record: { phase: "applied", baseline, levelsOwned: [true, true], cacheOwned: [true, true],
      version: 2, current: structuredClone(low.current), pending: null }, active: baseline.plan, panel: baseline.panel, levels: [0, 0], cache: [80, 20] };
    low.restoreRevision = "low-brightness-restore-0";
    h.current = { enabled: true, actions: ["batteryLowBrightness"], osd: true }; h.authorized = true; h.status.running = true;
    h.revision = "low-brightness-applied"; window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(facts(page).getByRole("button", { name: "恢复低电量前亮度", exact: true })).toBeEnabled();
}
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与输入"); });

test("计划亮度与事件目标使用有分母的比例环，草稿不写入", async ({ page }) => {
  await scope(page).check();
  const current = facts(page).locator(".ccc-battery-low-columns > *").first();
  const target = facts(page).locator(".ccc-battery-low-columns > *").last();
  for (const [label, value] of [["插电计划亮度", "80"], ["电池计划亮度", "60"]]) {
    await expect(current.getByRole("meter", { name: label, exact: true })).toHaveAttribute("aria-valuenow", value);
    await expect(target.getByRole("meter", { name: label, exact: true })).toHaveAttribute("aria-valuenow", "0");
  }
  await scope(page).check();
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
  expect(await calls(page, "restore_low_battery_brightness")).toEqual([]);
});

test("低电量亮度独立授权，确认前不应用", async ({ page }) => {
  await panel(page).getByRole("checkbox", { name: "启用原厂热键接管", exact: true }).check();
  await scope(page).check();
  await panel(page).getByRole("button", { name: "应用热键策略", exact: true }).click();
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
  await panel(page).getByRole("button", { name: "确认此范围的热键接管？", exact: true }).click();
  expect((await calls(page, "apply_hotkeys"))[0].args.request.settings.actions).toEqual(["batteryLowBrightness"]);
});

test("分别预览两种原值，确认恢复失败后可重试", async ({ page }) => {
  await applied(page);
  await expect(facts(page)).toContainText("插电计划亮度 0% → 80%");
  await expect(facts(page)).toContainText("电池计划亮度 0% → 60%");
  await page.evaluate(() => { window.__uiFixture.lowBatteryRestoreFault = "恢复计划失败"; });
  for (let attempt = 0; attempt < 2; attempt++) {
    await facts(page).getByRole("button", { name: "恢复低电量前亮度", exact: true }).click();
    if (!attempt) expect(await calls(page, "restore_low_battery_brightness")).toEqual([]);
    await facts(page).getByRole("button", { name: "确认停止监听并恢复上述亮度？", exact: true }).click();
    if (!attempt) await expect(page.getByText("恢复计划失败", { exact: true }).first()).toBeVisible();
  }
  expect(await calls(page, "restore_low_battery_brightness")).toHaveLength(2);
  await expect(panel(page)).toContainText("热键监听已停止");
  await expect(scope(page)).toBeChecked();
  await expect(facts(page).getByRole("meter", { name: "插电计划亮度", exact: true }).first()).toHaveAttribute("aria-valuenow", "80");
});

test("来源变化时保留诊断并禁止恢复", async ({ page }) => {
  await applied(page);
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, low = h.facts.batteryLowBrightness;
    low.identity = null; low.error = "活动计划已变化"; low.restoreRevision = null; low.restoreError = "计划亮度已被其他程序修改";
    const cap = h.facts.capabilities.find(value => value.scope === "batteryLowBrightness"); cap.identity = null; cap.reason = low.error;
    h.revision = "low-conflict"; window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(facts(page)).toContainText("计划亮度已被其他程序修改");
  await expect(facts(page).getByRole("button", { name: "恢复低电量前亮度", exact: true })).toBeDisabled();
  expect(await calls(page, "restore_low_battery_brightness")).toEqual([]);
});

test("深浅宽窄布局保留边框和模块间隔", async ({ page }) => {
  await applied(page);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 900 }); await facts(page).scrollIntoViewIfNeeded();
      const box = await facts(page).evaluate(el => {
        const columns = el.querySelector(".ccc-battery-low-columns"), parts = [...columns.children];
        const [left, right] = parts.map(part => part.getBoundingClientRect());
        return { overflow: el.scrollWidth > el.clientWidth + 1, right: el.getBoundingClientRect().right,
          borders: parts.every(part => parseFloat(getComputedStyle(part).borderTopWidth) > 0), gap: parseFloat(getComputedStyle(columns).gap),
          layout: el.clientWidth >= 380 ? right.left >= left.right - 1 : right.top >= left.bottom - 1 };
      });
      expect(box.overflow).toBe(false); expect(box.right).toBeLessThanOrEqual(width);
      expect(box.borders).toBe(true); expect(box.gap).toBeGreaterThan(0); expect(box.layout).toBe(true);
      if (width !== 760) await facts(page).screenshot({ path: `target/ui-check/battery-low-brightness-${theme}-${width}.png` });
    }
  }
});

test("自然结束后仍能预览并确认恢复首次不同的插电电池亮度", async ({ page }) => {
  await applied(page);
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, low = h.facts.batteryLowBrightness;
    low.current.levels = [80, 80]; low.current.cache = [0, 20];
    low.settingTarget = [0, 0]; low.endingTarget = [80, 80];
    low.recovery.record.current = structuredClone(low.current);
    low.recovery.levels = [80, 80]; low.recovery.cache = [0, 20];
    low.restoreRevision = "natural-end-initial-restore";
    h.revision = "natural-end"; window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(facts(page)).toContainText("电池计划亮度 80% → 60%");
  const restore = facts(page).getByRole("button", { name: "恢复低电量前亮度", exact: true });
  await restore.click();
  expect(await calls(page, "restore_low_battery_brightness")).toEqual([]);
  await facts(page).getByRole("button", { name: "确认停止监听并恢复上述亮度？", exact: true }).click();
  expect((await calls(page, "restore_low_battery_brightness"))[0].args.expectedRevision).toBe("natural-end-initial-restore");
  await expect(facts(page).getByRole("meter", { name: "电池计划亮度", exact: true }).first()).toHaveAttribute("aria-valuenow", "60");
});

test("重复事件目标由后端预览提供，不固定绘制零亮度", async ({ page }) => {
  await applied(page);
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, low = h.facts.batteryLowBrightness;
    low.current.levels = [52, 37]; low.current.cache = [80, 20];
    low.settingTarget = [52, 37]; low.endingTarget = [52, 52];
    h.revision = "repeat-current-brightness"; window.__uiEmit("hardware-controls-changed", null);
  });
  const target = facts(page).locator(".ccc-battery-low-columns > *").last();
  await expect(target.getByRole("meter", { name: "插电计划亮度", exact: true })).toHaveAttribute("aria-valuenow", "52");
  await expect(target.getByRole("meter", { name: "电池计划亮度", exact: true })).toHaveAttribute("aria-valuenow", "37");
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
});
