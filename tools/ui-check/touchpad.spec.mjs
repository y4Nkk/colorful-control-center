import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "触摸板设置", exact: true });
const current = page => panel(page).getByRole("region", { name: "当前触摸板状态", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply|restore)_touchpad$/.test(call.command)));
async function apply(page) {
  await panel(page).getByRole("button", { name: "应用触摸板设置", exact: true }).click();
  await panel(page).getByRole("button", { name: "确认关闭触摸板？", exact: true }).click();
}
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openWorkspace(page, "键盘与输入"); });

test("草稿与确认分离，系统开关与原始偏好分别恢复", async ({ page }) => {
  await expect(current(page)).toContainText("系统触摸板已启用");
  await expect(current(page)).toContainText("保存偏好已关闭");
  await panel(page).getByRole("radio", { name: "关闭", exact: true }).check();
  await expect(panel(page)).toContainText("已启用 → 已关闭");
  expect(await writes(page)).toHaveLength(0);
  await panel(page).getByRole("button", { name: "应用触摸板设置", exact: true }).click();
  expect(await writes(page)).toHaveLength(0);
  await panel(page).getByRole("button", { name: "确认关闭触摸板？", exact: true }).click();
  await expect(current(page)).toContainText("系统触摸板已关闭");
  await panel(page).getByRole("button", { name: "恢复触摸板初始", exact: true }).click();
  await panel(page).getByRole("button", { name: "恢复修改前的触摸板状态与偏好？", exact: true }).click();
  await expect(current(page)).toContainText("系统触摸板已启用");
  await expect(current(page)).toContainText("保存偏好已关闭");
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});
test("新修订撤销待确认，刷新保留目标草稿", async ({ page }) => {
  await panel(page).getByRole("radio", { name: "关闭", exact: true }).check();
  await panel(page).getByRole("button", { name: "应用触摸板设置", exact: true }).click();
  await page.evaluate(() => { const p = window.__uiFixture.touchpad; p.current.saved = 1; p.revision = "external-touchpad"; });
  await panel(page).getByRole("button", { name: "重新读取触摸板", exact: true }).click();
  await expect(panel(page).getByRole("button", { name: "应用触摸板设置", exact: true })).toBeVisible();
  await expect(panel(page).getByRole("radio", { name: "关闭", exact: true })).toBeChecked();
  expect(await writes(page)).toHaveLength(0);
});
test("读取失败撤销执行快照，重试后保留草稿", async ({ page }) => {
  await panel(page).getByRole("radio", { name: "关闭", exact: true }).check();
  await page.evaluate(() => window.__uiFixture.touchpadReadFault = "隔离读取失败");
  await panel(page).getByRole("button", { name: "重新读取触摸板", exact: true }).click();
  await expect(panel(page)).toContainText("隔离读取失败");
  await expect(panel(page).getByRole("button", { name: "应用触摸板设置", exact: true })).toHaveCount(0);
  await page.evaluate(() => window.__uiFixture.touchpadReadFault = null);
  await panel(page).getByRole("button", { name: "重新读取触摸板", exact: true }).click();
  await expect(panel(page).getByRole("radio", { name: "关闭", exact: true })).toBeChecked();
  expect(await writes(page)).toHaveLength(0);
});
test("部分失败保留首次记录，恢复失败可重试", async ({ page }) => {
  await panel(page).getByRole("radio", { name: "关闭", exact: true }).check();
  await page.evaluate(() => { window.__uiFixture.touchpadWriteFault = "隔离写入失败"; window.__uiFixture.touchpadRecoveryFault = true; });
  await apply(page);
  await expect(panel(page)).toContainText("恢复失败，记录保留");
  await page.evaluate(() => { window.__uiFixture.touchpadRestoreFault = "隔离恢复失败"; });
  await panel(page).getByRole("button", { name: "恢复触摸板初始", exact: true }).click();
  await panel(page).getByRole("button", { name: "恢复修改前的触摸板状态与偏好？", exact: true }).click();
  await expect(panel(page)).toContainText("隔离恢复失败");
  expect(await page.evaluate(() => window.__uiFixture.touchpad.initial.enabled)).toBe(true);
  await page.evaluate(() => window.__uiFixture.touchpadRestoreFault = null);
  await panel(page).getByRole("button", { name: "恢复触摸板初始", exact: true }).click();
  await panel(page).getByRole("button", { name: "恢复修改前的触摸板状态与偏好？", exact: true }).click();
  await expect(current(page)).toContainText("系统触摸板已启用");
});
test("设备变化阻止新的切换与旧恢复", async ({ page }) => {
  await panel(page).getByRole("radio", { name: "关闭", exact: true }).check(); await apply(page);
  await page.evaluate(() => { const p = window.__uiFixture.touchpad; p.current.devices = ["replacement"]; p.revision = "replacement"; });
  await panel(page).getByRole("button", { name: "重新读取触摸板", exact: true }).click();
  await expect(panel(page)).toContainText("触摸板设备集合已变化");
  await expect(panel(page).getByRole("radio", { name: "启用", exact: true })).toBeDisabled();
  await expect(panel(page).getByRole("button", { name: "恢复触摸板初始", exact: true })).toBeDisabled();
  expect(await writes(page)).toHaveLength(1);
});
test("未知系统值不使用保存偏好冒充状态", async ({ page }) => {
  await expect(current(page)).toBeVisible();
  await page.evaluate(() => { const p = window.__uiFixture.touchpad; p.current.enabled = null; p.current.saved = 1; p.revision = "unknown"; });
  await panel(page).getByRole("button", { name: "重新读取触摸板", exact: true }).click();
  await expect(current(page)).toContainText("系统触摸板未知");
  await expect(current(page)).toContainText("保存偏好已启用");
  await expect(panel(page).getByRole("radio", { name: "关闭", exact: true })).toBeDisabled();
  expect(await writes(page)).toHaveLength(0);
});
test("触摸板热键有独立范围", async ({ page }) => {
  const hotkeys = page.getByRole("region", { name: "原厂热键与屏显", exact: true });
  const touchpad = hotkeys.getByRole("checkbox", { name: /^触摸板开关/ });
  await expect(touchpad).not.toBeChecked(); await touchpad.check();
  await expect(hotkeys.getByRole("checkbox", { name: /^麦克风静音/ })).not.toBeChecked();
  expect(await writes(page)).toHaveLength(0);
});
test("深浅宽窄模块边框与左右布局", async ({ page }) => {
  for (const theme of ["light", "dark"]) for (const width of [1440, 760, 360]) {
    await page.setViewportSize({ width, height: 1000 });
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    await panel(page).scrollIntoViewIfNeeded();
    const layout = await panel(page).evaluate(element => {
      const cards = [...element.querySelectorAll(".ccc-touchpad-grid > *")];
      const [a, b] = cards.map(card => card.getBoundingClientRect());
      return { left: a.left, right: a.right, second: b.left, top: a.top, nextTop: b.top,
        borders: cards.map(card => getComputedStyle(card).borderWidth),
        overflow: document.documentElement.scrollWidth > innerWidth };
    });
    expect(layout.overflow).toBe(false); expect(layout.borders.every(value => value !== "0px")).toBe(true);
    if (width === 1440) { expect(layout.second).toBeGreaterThan(layout.right); expect(layout.nextTop).toBe(layout.top); }
    if (width === 360) expect(layout.nextTop).toBeGreaterThan(layout.top);
    await panel(page).screenshot({ path: `target/ui-check/touchpad-${theme}-${width}.png` });
  }
});
