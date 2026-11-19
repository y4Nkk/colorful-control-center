import { configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "原厂热键与屏显", exact: true });
const facts = page => page.getByRole("region", { name: "原厂低电量阈值事实与预览", exact: true });
const scope = page => panel(page).getByRole("checkbox", { name: "原厂低电量阈值", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
async function applied(page) {
  await expect(facts(page)).toBeVisible();
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, battery = h.facts.batteryThresholds;
    const baseline = structuredClone(battery.current), targets = [25, 20, 22];
    battery.current.plans.forEach(plan => { plan.values = targets.map(value => [value, value]); });
    battery.current.cache = [11, 7, 6];
    battery.recovery = { phase: "applied", baseline, currentActive: baseline.active, current: structuredClone(battery.current.plans),
      cache: [...battery.current.cache], owned: baseline.plans.map(plan => plan.values.map((pair, i) => pair.map(value => value !== targets[i]))),
      cacheOwned: [true, true, true] };
    battery.restoreRevision = "battery-restore-0";
    h.current = { enabled: true, actions: ["batteryThresholds"], osd: true }; h.authorized = true; h.status.running = true;
    h.revision = "battery-applied"; window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(facts(page).getByRole("button", { name: "恢复初始电池阈值", exact: true })).toBeEnabled();
}
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与灯光"); });

test("预览真实计划和三个比例目标，选择草稿不写入", async ({ page }) => {
  await expect(facts(page)).toContainText("平衡计划 · 当前活动");
  await expect(facts(page)).toContainText("低电量 10 / 11%");
  await expect(facts(page)).toContainText("节能计划");
  for (const [name, value] of [["低电量", "25"], ["严重电量", "20"], ["预留电量", "22"]]) {
    await expect(facts(page).getByRole("meter", { name, exact: true })).toHaveAttribute("aria-valuenow", value);
  }
  await scope(page).check();
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
  expect(await calls(page, "restore_battery_thresholds")).toEqual([]);
});

test("阈值事件独立授权，确认前不应用", async ({ page }) => {
  await panel(page).getByRole("switch", { name: "启用原厂热键接管", exact: true }).check();
  await scope(page).check();
  await previewConfiguration(page, "原厂热键与屏显");
  expect(await calls(page, "apply_hotkeys")).toEqual([]);
  await configurationView(page).getByRole("button", { name: "保存热键接管范围", exact: true }).click();
  expect((await calls(page, "apply_hotkeys"))[0].args.request.settings.actions).toEqual(["batteryThresholds"]);
});

test("逐计划初始值预览后确认恢复，失败可重试", async ({ page }) => {
  await applied(page);
  await expect(facts(page)).toContainText("低电量 25→10% / 25→11%");
  await expect(facts(page)).toContainText("低电量 25→9% / 25→0%");
  await page.evaluate(() => { window.__uiFixture.batteryThresholdRestoreFault = "计划恢复失败"; });
  for (let attempt = 0; attempt < 2; attempt++) {
    await facts(page).getByRole("button", { name: "恢复初始电池阈值", exact: true }).click();
    if (!attempt) expect(await calls(page, "restore_battery_thresholds")).toEqual([]);
    await facts(page).getByRole("button", { name: "确认停止监听并恢复上述阈值？", exact: true }).click();
    if (!attempt) await expect(page.getByText("计划恢复失败", { exact: true }).first()).toBeVisible();
  }
  expect(await calls(page, "restore_battery_thresholds")).toHaveLength(2);
  await expect(facts(page)).toContainText("低电量 10 / 11%");
  await expect(panel(page)).toContainText("热键监听已停止");
  await expect(scope(page)).toBeChecked();
});

test("来源异常展示记录并禁止覆盖", async ({ page }) => {
  await applied(page);
  await page.evaluate(() => {
    const h = window.__uiFixture.hotkeys, battery = h.facts.batteryThresholds;
    battery.identity = null; battery.error = "电池计划阈值已变化"; battery.restoreRevision = null;
    battery.restoreError = "计划阈值已被其他程序修改";
    const cap = h.facts.capabilities.find(c => c.scope === "batteryThresholds"); cap.identity = null; cap.reason = battery.error;
    h.revision = "battery-conflict"; window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(facts(page)).toContainText("初始恢复目标");
  await expect(facts(page)).toContainText("计划阈值已被其他程序修改");
  await expect(facts(page).getByRole("button", { name: "恢复初始电池阈值", exact: true })).toBeDisabled();
  expect(await calls(page, "restore_battery_thresholds")).toEqual([]);
});

test("深浅宽窄布局保留卡片边框、间距和左右分组", async ({ page }) => {
  await applied(page);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 900 }); await facts(page).scrollIntoViewIfNeeded();
      const box = await facts(page).evaluate(el => {
        const columns = el.querySelector(".ccc-battery-threshold-columns"), parts = [...columns.children];
        const [left, right] = parts.map(part => part.getBoundingClientRect());
        return { overflow: el.scrollWidth > el.clientWidth + 1, right: el.getBoundingClientRect().right,
          borders: parts.every(part => parseFloat(getComputedStyle(part).borderTopWidth) > 0), gap: parseFloat(getComputedStyle(columns).gap),
          layout: el.clientWidth >= 380 ? right.left >= left.right - 1 : right.top >= left.bottom - 1 };
      });
      expect(box.overflow).toBe(false); expect(box.right).toBeLessThanOrEqual(width);
      expect(box.borders).toBe(true); expect(box.gap).toBeGreaterThan(0); expect(box.layout).toBe(true);
      if (width !== 760) await facts(page).screenshot({ path: `target/ui-check/battery-threshold-${theme}-${width}.png` });
    }
  }
});
