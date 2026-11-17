import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { installTopCoverFixture } from "./top-cover-fixture.mjs";

const panel = page => page.getByRole("region", { name: "灯光策略与外围灯区", exact: true });
const cover = page => panel(page).getByRole("region", { name: "独立顶盖灯", exact: true });
const slider = page => cover(page).getByRole("slider", { name: "顶盖亮度", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => /^(apply|restore)_lighting_options$/.test(c.command)));
async function apply(page) {
  await panel(page).getByRole("button", { name: "应用灯光选项", exact: true }).click();
  await panel(page).getByRole("button", { name: "确认应用灯光选项？", exact: true }).click();
}
async function restore(page) {
  await panel(page).getByRole("button", { name: "恢复灯光选项初始", exact: true }).click();
  await panel(page).getByRole("button", { name: "恢复本会话修改前的灯光选项？", exact: true }).click();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(installTopCoverFixture);
  await page.goto("/"); await openWorkspace(page, "灯光效果");
});

test("顶盖亮度与颜色先预览，确认写入后回读并恢复", async ({ page }) => {
  await expect(slider(page)).toHaveValue("3");
  await cover(page).getByLabel("顶盖颜色", { exact: true }).fill("#00fe81");
  await slider(page).fill("1");
  await expect(panel(page)).toContainText("亮度3级 · #0A141E → 亮度1级 · #00FE81");
  await expect(cover(page).getByRole("img").locator("rect").nth(1)).toHaveAttribute("fill", "#0a141e");
  await panel(page).getByRole("button", { name: "应用灯光选项", exact: true }).click();
  expect(await writes(page)).toHaveLength(0);
  await panel(page).getByRole("button", { name: "确认应用灯光选项？", exact: true }).click();
  expect((await writes(page))[0].args.request.values.topCover).toEqual({ brightness: 1, color: { r: 0, g: 254, b: 129 } });
  await expect(cover(page)).toContainText("当前亮度1级");
  await expect(panel(page)).toContainText("独立顶盖亮度3级 · #0A141E");
  await restore(page);
  await expect(slider(page)).toHaveValue("3");
  await expect(cover(page).getByLabel("顶盖颜色", { exact: true })).toHaveValue("#0a141e");
  expect(await page.evaluate(() => window.__uiFixture.lightingOptions.initial.topCover.brightness)).toBe(3);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("关闭保留颜色，多次应用仍恢复首次值", async ({ page }) => {
  await cover(page).getByLabel("顶盖颜色", { exact: true }).fill("#123456");
  await slider(page).fill("0");
  await apply(page);
  await expect(cover(page)).toContainText("当前关闭 · #123456");
  await slider(page).fill("4");
  await apply(page);
  await expect(cover(page)).toContainText("当前亮度4级 · #123456");
  await restore(page);
  await expect(slider(page)).toHaveValue("3");
  await expect(cover(page).getByLabel("顶盖颜色", { exact: true })).toHaveValue("#0a141e");
});

test("外部修订撤销旧确认，干净状态更新而草稿保留", async ({ page }) => {
  await slider(page).fill("2");
  await panel(page).getByRole("button", { name: "应用灯光选项", exact: true }).click();
  await page.evaluate(() => { const options = window.__uiFixture.lightingOptions; options.current.topCover.brightness = 4; options.revision = "external-cover"; });
  await panel(page).getByRole("button", { name: "重新读取灯光选项", exact: true }).click();
  await expect(panel(page)).toContainText("亮度4级 · #0A141E → 亮度2级 · #0A141E");
  await expect(panel(page).getByRole("button", { name: "应用灯光选项", exact: true })).toBeVisible();
  expect(await writes(page)).toHaveLength(0);
  await slider(page).fill("4");
  await page.evaluate(() => { const options = window.__uiFixture.lightingOptions; options.current.topCover.brightness = 1; options.revision = "clean-cover"; });
  await panel(page).getByRole("button", { name: "重新读取灯光选项", exact: true }).click();
  await expect(slider(page)).toHaveValue("1");
});

test("能力移除撤销顶盖草稿，其他灯光选项继续可编辑", async ({ page }) => {
  await slider(page).fill("2");
  await panel(page).getByRole("spinbutton", { name: "闲置等待秒数" }).fill("45");
  await page.evaluate(() => { const options = window.__uiFixture.lightingOptions; options.current.topCover = null; options.capabilities.topCover = false; options.revision = "no-cover"; });
  await panel(page).getByRole("button", { name: "重新读取灯光选项", exact: true }).click();
  await expect(cover(page)).toHaveCount(0);
  await expect(panel(page).getByLabel("Logo 灯颜色", { exact: true })).toHaveCount(0);
  await apply(page);
  expect((await writes(page))[0].args.request.values).toMatchObject({ topCover: null, idleTimer: { seconds: 45 } });
});

test("写入及恢复失败保留草稿和基线，显式重试完成恢复", async ({ page }) => {
  await slider(page).fill("1");
  await page.evaluate(() => window.__uiFixture.lightingOptionsFault = "隔离顶盖保存失败，本次前态已恢复");
  await apply(page);
  await expect(panel(page).getByRole("alert")).toContainText("隔离顶盖保存失败");
  await expect(slider(page)).toHaveValue("1");
  await expect(cover(page)).toContainText("当前亮度3级");
  await page.evaluate(() => window.__uiFixture.lightingOptionsRestoreFault = "隔离顶盖恢复失败，基线保留");
  await restore(page);
  await expect(panel(page).getByRole("alert")).toContainText("基线保留");
  await page.evaluate(() => window.__uiFixture.lightingOptionsRestoreFault = null);
  await restore(page);
  await expect(slider(page)).toHaveValue("3");
  await expect(panel(page).getByRole("alert")).toHaveCount(0);
});

test("顶盖模块左右布局、独立边框和深浅窄屏不溢出", async ({ page }) => {
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [1440, 760, 360]) {
      await page.setViewportSize({ width, height: 1200 });
      await cover(page).scrollIntoViewIfNeeded();
      await expect(cover(page)).toHaveAttribute("data-surface", "inset");
      expect(await cover(page).evaluate(el => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      const positions = await cover(page).evaluate(el => [...el.children].map(child => ({ x: child.getBoundingClientRect().x, y: child.getBoundingClientRect().y })));
      if (width === 1440) expect(positions[1].x).toBeGreaterThan(positions[0].x);
      if (width === 360) expect(positions[1].y).toBeGreaterThan(positions[0].y);
      await cover(page).screenshot({ path: `target/ui-check/top-cover-${theme}-${width}.png` });
    }
  }
});
