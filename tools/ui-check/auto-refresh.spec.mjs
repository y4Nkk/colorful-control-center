import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const card = page => page.getByRole("region", { name: "自动刷新率", exact: true });
const trial = page => page.getByRole("dialog", { name: "自动刷新率试用" });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
async function enable(page) {
  const previous = (await calls(page, "enable_automatic_refresh")).length;
  const toggle = card(page).getByRole("switch", { name: "启用自动刷新率" });
  await expect(toggle).toHaveAccessibleDescription("启用时试用 144 Hz");
  await toggle.click();
  await expect(trial(page)).toBeVisible();
  expect((await calls(page, "enable_automatic_refresh")).length).toBe(previous + 1);
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/"); await openWorkspace(page, "设备配置");
  await expect(card(page)).toBeVisible();
  await expect(card(page).getByRole("switch", { name: "启用自动刷新率" })).toBeEnabled();
});

test("离开设备配置后的全局试用检查不触发显示模式枚举", async ({ page }) => {
  await openWorkspace(page, "概览");
  await expect(card(page)).not.toBeVisible();
  const inventories = (await calls(page, "read_display_modes")).length;
  const trials = (await calls(page, "read_automatic_display_trial")).length;
  await expect.poll(async () => {
    const idleReads = await calls(page, "read_automatic_display_trial");
    const modeReads = await calls(page, "read_display_modes");
    return idleReads.length + modeReads.length;
  }).toBeGreaterThan(inventories + trials);
  expect(await calls(page, "read_display_modes")).toHaveLength(inventories);
});

test("开关直接启用，跨页面保留试用后关闭并恢复启用前状态", async ({ page }) => {
  await enable(page);
  await expect(trial(page)).toContainText("60 Hz");
  await expect(trial(page)).toContainText("144 Hz");
  await expect(trial(page).getByRole("status")).toHaveText(/\d+ 秒后恢复/);
  await expect(page.getByRole("region", { name: "屏幕显示", exact: true }).getByRole("status"))
    .toHaveText("请在「自动刷新率试用」中保留或恢复本次试用");
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: "设置" }).click();
  await expect(trial(page)).toBeVisible();
  await trial(page).getByRole("button", { name: "保留刷新率" }).click();
  await expect(trial(page)).toHaveCount(0);
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: "设备配置" }).click();
  const toggle = card(page).getByRole("switch", { name: "启用自动刷新率" });
  await expect(toggle).toBeChecked();
  await expect(card(page).getByText(/^(已启用|未启用)$/)).toHaveCount(0);
  await expect(toggle).toHaveAccessibleDescription("停用时恢复 60 Hz");
  expect(await calls(page, "restore_automatic_refresh")).toEqual([]);
  await toggle.click();
  await expect.poll(() => page.evaluate(() => window.__uiFixture.display.displays[0].current.frequency)).toBe(60);
  await expect(toggle).not.toBeChecked();
  await expect(card(page).getByText(/^(已启用|未启用)$/)).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("取消或到期后恢复本次模式，过期试用无法保留", async ({ page }) => {
  await enable(page);
  await trial(page).getByRole("button", { name: "恢复本次试用" }).click();
  await expect(trial(page)).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => window.__uiFixture.display.displays[0].current.frequency)).toBe(60);
  const reads = (await calls(page, "read_automatic_refresh")).length;
  await expect.poll(async () => (await calls(page, "read_automatic_refresh")).length).toBeGreaterThan(reads);
  const toggle = card(page).getByRole("switch", { name: "启用自动刷新率" });
  await expect(toggle).toBeChecked();
  await toggle.click();
  await expect(toggle).not.toBeChecked();
  await expect(card(page).getByRole("button", { name: "恢复启用前状态", exact: true })).toHaveCount(0);
  await enable(page);
  await page.evaluate(() => { window.__uiFixture.displayExpiresAt = Date.now() - 1; });
  await expect(trial(page)).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => window.__uiFixture.display.displays[0].current.frequency)).toBe(60);
});

test("事务未完成时开关保持打开，关闭即恢复启用前状态", async ({ page }) => {
  await enable(page);
  await trial(page).getByRole("button", { name: "恢复本次试用" }).click();
  await expect(trial(page)).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.autoRefresh.active = false; window.__uiFixture.autoRefresh.error = "自动刷新率事务未完成，请先恢复"; });
  await card(page).getByRole("button", { name: "重新读取自动刷新率", exact: true }).click();
  const toggle = card(page).getByRole("switch", { name: "启用自动刷新率" });
  await expect(card(page).getByRole("status")).toHaveText("已暂停");
  await expect(card(page).getByRole("alert")).toContainText("请先恢复");
  await expect(toggle).toBeChecked();
  await expect(toggle).toBeEnabled();
  await expect(card(page).getByRole("button", { name: "恢复启用前状态", exact: true })).toHaveCount(0);
  await toggle.click();
  await expect(toggle).not.toBeChecked();
  expect((await calls(page, "restore_automatic_refresh")).length).toBe(1);
  await page.evaluate(() => { window.__uiFixture.autoRefresh.error = null; });
  await expect(toggle).toBeEnabled();
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("监听不可用时显示原因并禁止启用", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.autoRefresh.listenerOnline = false; window.__uiFixture.autoRefresh.issue = "隔离监听已停止"; });
  await expect(card(page).getByRole("switch", { name: "启用自动刷新率" })).toBeDisabled();
  await expect(card(page).getByRole("alert")).toContainText("隔离监听已停止");
  await card(page).getByRole("button", { name: "详情", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "刷新率规则与恢复" })).toContainText("隔离监听已停止");
  expect(await calls(page, "enable_automatic_refresh")).toEqual([]);
});

test("频率无需变化时直接启用并恢复策略，全程没有显示试用", async ({ page }) => {
  const toggle = card(page).getByRole("switch", { name: "启用自动刷新率" });
  await page.evaluate(() => { window.__uiFixture.autoRefresh.current.savedOrHighest = 60; });
  await expect(toggle).not.toHaveAccessibleDescription(/启用时试用/);
  await toggle.click();
  await expect(toggle).toBeChecked();
  await expect(trial(page)).toHaveCount(0);
  await toggle.click();
  await expect(toggle).not.toBeChecked();
  expect((await calls(page, "enable_automatic_refresh")).length).toBe(1);
  expect((await calls(page, "restore_automatic_refresh")).length).toBe(1);
});

test("模块边框和全局确认适配宽窄及深浅主题", async ({ page }) => {
  const readout = card(page).locator(".ccc-refresh-readout");
  const targets = readout.locator(".ccc-refresh-rates");
  const artwork = readout.locator("svg.ccc-refresh-target-art");
  await expect(readout.locator(".ccc-refresh-screen")).toHaveText(/^当前主屏\s*60 Hz$/);
  await expect(readout.locator("[data-icon]")).toHaveCount(0);
  await expect(targets.locator("dt")).toHaveText(["低刷新率", "恢复刷新率"]);
  await expect(targets.locator("dd")).toHaveText(["60 Hz", "144 Hz"]);
  await expect(artwork).toHaveAttribute("aria-hidden", "true");
  await expect(artwork).toHaveAttribute("focusable", "false");
  await expect(artwork).toHaveCSS("pointer-events", "none");
  for (const theme of ["light", "dark"]) {
    await page.evaluate(value => { document.documentElement.dataset.theme = value; }, theme);
    for (const width of [360, 760, 1440]) {
      await page.setViewportSize({ width, height: 900 });
      await card(page).scrollIntoViewIfNeeded();
      const geometry = await card(page).evaluate(element => {
        const footer = element.querySelector(".ccc-refresh-control");
        const style = getComputedStyle(element);
        return {
          width: element.clientWidth, scroll: element.scrollWidth,
          border: parseFloat(style.borderTopWidth),
          footerLast: footer === element.lastElementChild,
          footerInset: element.getBoundingClientRect().bottom - footer.getBoundingClientRect().bottom
            - parseFloat(style.borderBottomWidth),
          padding: parseFloat(style.paddingBottom),
        };
      });
      expect(geometry.scroll).toBeLessThanOrEqual(geometry.width + 1);
      expect(geometry.border).toBeGreaterThan(0);
      expect(geometry.footerLast).toBe(true);
      expect(geometry.footerInset).toBeCloseTo(geometry.padding, 0);
      const gap = await targets.evaluate(element => {
        const [low, restore] = [...element.children].map(rate => rate.getBoundingClientRect());
        return restore.left - low.right;
      });
      expect(gap).toBeCloseTo(0, 1);
      if (width === 1440) {
        const display = await page.getByRole("region", { name: "屏幕显示", exact: true }).boundingBox();
        const auto = await card(page).boundingBox();
        expect(display.y).toBeCloseTo(auto.y, 0);
      }
      if (width !== 760) await card(page).screenshot({ path: `target/ui-check/auto-refresh-${theme}-${width}.png` });
    }
  }
  await page.setViewportSize({ width: 360, height: 900 });
  await enable(page);
  const rect = await trial(page).boundingBox();
  expect(rect.x).toBeGreaterThanOrEqual(0);
  expect(rect.x + rect.width).toBeLessThanOrEqual(360);
  await trial(page).screenshot({ path: "target/ui-check/auto-refresh-trial-dark-360.png" });
});

const scanArt = page => card(page).locator("svg.ccc-refresh-scan");
const scanFrame = (page, time) => scanArt(page).evaluate((svg, time) => {
  const animations = svg.getAnimations({ subtree: true });
  const frame = svg.getBoundingClientRect();
  animations.forEach(animation => { animation.pause(); animation.currentTime = time; });
  const edge = svg.querySelector(".ccc-refresh-scan-edge").getBoundingClientRect();
  const state = {
    height: frame.height, edgeTop: edge.top - frame.top, edgeBottom: edge.bottom - frame.top,
    beamOpacity: Number(getComputedStyle(svg.querySelector(".ccc-refresh-scan-beam")).opacity),
    edgeOpacity: Number(getComputedStyle(svg.querySelector(".ccc-refresh-scan-edge")).opacity),
    inside: edge.left >= frame.left && edge.right <= frame.right && edge.top >= frame.top && edge.bottom <= frame.bottom,
    pulses: [...svg.querySelectorAll(".ccc-refresh-scan-pulse")].map(pulse => ({
      offset: getComputedStyle(pulse).strokeDashoffset, opacity: Number(getComputedStyle(pulse).opacity) })),
  };
  animations.forEach(animation => animation.play());
  return state;
}, time);
const scanAnimations = page => scanArt(page).evaluate(svg =>
  svg.getAnimations({ subtree: true }).map(motion => [motion.animationName, motion.effect.getTiming().iterations]).sort());

test("扫描光束与光点在活动页循环，并始终留在纹理画幅内", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.evaluate(() => { window.__uiFixture.preferences.motion = "system"; document.documentElement.dataset.motion = "system"; });
  const scan = scanArt(page);
  await expect(scan).toHaveAttribute("data-active", "true");
  expect(await scanAnimations(page)).toEqual([["ccc-refresh-scan-pulse", Infinity], ["ccc-refresh-scan-pulse", Infinity],
    ["ccc-refresh-scan-pulse", Infinity], ["ccc-refresh-scan-sweep", Infinity]]);
  const frames = [];
  for (const time of [0, 216, 1440, 2376, 2592, 3240]) frames.push(await scanFrame(page, time));
  const tops = frames.map(frame => frame.edgeTop);
  expect(tops).toEqual([...tops].sort((a, b) => a - b));
  expect(tops.at(-1) - tops[0]).toBeGreaterThan(frames[0].height - 3);
  expect(frames.map(frame => frame.beamOpacity)).toEqual([0, 1, 1, 1, 1, 0]);
  expect(frames.slice(1, 5).every(frame => frame.inside)).toBe(true);
  for (let index = 0; index < 3; index += 1) {
    expect(new Set(frames.map(frame => frame.pulses[index].offset)).size).toBeGreaterThan(2);
    expect(Math.max(...frames.map(frame => frame.pulses[index].opacity))).toBeGreaterThan(0);
  }
  for (const theme of ["light", "dark"]) {
    await page.evaluate(value => { document.documentElement.dataset.theme = value; }, theme);
    for (const width of [1440, 360]) {
      await page.setViewportSize({ width, height: 900 });
      await card(page).scrollIntoViewIfNeeded();
      for (const time of [720, 1900]) {
        await scan.evaluate((svg, time) => {
          for (const motion of svg.getAnimations({ subtree: true })) { motion.pause(); motion.currentTime = time; }
        }, time);
        await card(page).screenshot({ path: `target/ui-check/auto-refresh-scan-${theme}-${width}-${time}.png` });
      }
      await scan.evaluate(svg => { for (const motion of svg.getAnimations({ subtree: true })) motion.play(); });
    }
  }
});

for (const [name, systemMotion, appMotion] of [["系统", "reduce", "system"], ["应用", "no-preference", "reduce"]]) {
  test(`${name}减少动效时扫描纹理保持静态，恢复后继续循环`, async ({ page }) => {
    await page.emulateMedia({ reducedMotion: systemMotion });
    await page.evaluate(motion => {
      window.__uiFixture.preferences.motion = motion; document.documentElement.dataset.motion = motion;
    }, appMotion);
    const scan = scanArt(page);
    await expect(scan).toHaveAttribute("data-active", "true");
    expect(await scanAnimations(page)).toEqual([]);
    const frame = await scanFrame(page, 0);
    expect(frame.edgeTop).toBeGreaterThan(frame.height * .4);
    expect(frame.edgeBottom).toBeLessThan(frame.height * .6);
    expect(frame.edgeOpacity).toBe(.26);
    expect(frame.pulses.map(pulse => pulse.opacity)).toEqual([0, 0, 0]);
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await page.evaluate(() => { window.__uiFixture.preferences.motion = "system"; document.documentElement.dataset.motion = "system"; });
    expect(await scanAnimations(page)).toHaveLength(4);
  });
}

test("读数未知或离开活动页时扫描纹理静止", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.evaluate(() => { window.__uiFixture.preferences.motion = "system"; document.documentElement.dataset.motion = "system"; });
  const scan = scanArt(page);
  await expect(scan).toHaveAttribute("data-active", "true");
  expect(await scanAnimations(page)).toHaveLength(4);
  await openWorkspace(page, "设置");
  const hiddenScan = page.locator('section[aria-label="自动刷新率"] svg.ccc-refresh-scan');
  await expect(hiddenScan).not.toHaveAttribute("data-active", "true");
  expect(await hiddenScan.evaluate(svg => svg.getAnimations({ subtree: true }).length)).toBe(0);
  await openWorkspace(page, "设备配置");
  await expect(scan).toHaveAttribute("data-active", "true");
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = (command, args) => command === "read_automatic_refresh"
      ? Promise.reject(new Error("隔离读取失败")) : invoke(command, args);
  });
  await expect(card(page).getByRole("alert")).toContainText("隔离读取失败");
  await expect(card(page).locator(".ccc-refresh-screen")).toContainText("—");
  await expect(card(page).locator(".ccc-refresh-targets")).toHaveCount(0);
  await expect(scan).not.toHaveAttribute("data-active", "true");
  expect(await scanAnimations(page)).toEqual([]);
});
