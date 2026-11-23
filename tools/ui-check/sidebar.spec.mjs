import { expect, test } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const rail = page => page.getByRole("navigation", { name: "主导航" });
const entry = (page, name) => rail(page).getByRole("button", { name, exact: true });
const indicator = page => rail(page).locator(".ccc-nav-indicator");
const sidebar = page => page.locator(".ccc-sidebar");

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await expect(entry(page, "概览")).toHaveAttribute("aria-current", "page");
});

async function expectIndicatorAt(page, name) {
  await expect(indicator(page)).toHaveCount(1);
  await expect.poll(async () => {
    const mark = await indicator(page).boundingBox();
    const button = await entry(page, name).boundingBox();
    return mark && button && [
      Math.round(mark.x - button.x),
      Math.round(mark.y + mark.height / 2 - button.y - button.height / 2),
      Math.round(mark.width),
      Math.round(mark.height),
    ];
  }).toEqual([0, 0, 3, 16]);
}

test("40px 导航按钮保持固定图标轴线，折叠提示支持指针与键盘", async ({ page }) => {
  const overview = entry(page, "概览");
  const glyph = overview.locator(".ccc-nav-glyph");
  const original = await overview.elementHandle();
  const expandedGlyph = await glyph.boundingBox();
  await expect(sidebar(page)).toHaveCSS("width", "224px");
  await expect(overview).toHaveCSS("height", "40px");
  await expect(overview).toHaveCSS("border-radius", "12px");
  await expect(glyph.locator('[data-weight="fill"]')).toHaveCSS("color", "rgb(50, 121, 246)");
  await expect(glyph.locator('[data-weight="regular"]')).toHaveCSS("opacity", "0");
  await expectIndicatorAt(page, "概览");
  const selectedFill = await overview.evaluate(button => getComputedStyle(button).backgroundColor);
  await overview.hover();
  await expect.poll(() => overview.evaluate(button => getComputedStyle(button, "::before").opacity)).toBe("1");
  await expect(page.getByRole("tooltip")).toHaveCount(0);
  const hoverFill = await overview.evaluate(button => getComputedStyle(button, "::before").backgroundColor);
  await page.mouse.down();
  expect(await overview.evaluate(button => getComputedStyle(button, "::before").backgroundColor)).not.toBe(hoverFill);
  await expect(overview).toHaveCSS("background-color", selectedFill);
  await page.mouse.up();

  await page.getByRole("button", { name: "收起侧边栏" }).click();
  await expect(sidebar(page)).toHaveCSS("width", "56px");
  await expect(overview).toHaveCSS("width", "40px");
  expect(await glyph.boundingBox()).toEqual(expandedGlyph);
  expect(await original.evaluate(button => button.isConnected)).toBe(true);
  await overview.hover();
  await expect(page.getByRole("tooltip")).toHaveText("概览");
  await page.keyboard.press("Escape");
  await expect(page.getByRole("tooltip")).toHaveCount(0);
  await page.getByRole("button", { name: "展开侧边栏" }).focus();
  await page.keyboard.press("Tab");
  await page.keyboard.press("Tab");
  await expect(overview).toBeFocused();
  await expect(page.getByRole("tooltip")).toHaveText("概览");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("tooltip")).toHaveCount(0);
});

test("指示条拉伸、连续切页接续并与底部设置独立定位", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.locator("html").evaluate(root => { root.dataset.motion = "system"; });
  const mark = indicator(page);
  await expectIndicatorAt(page, "概览");
  const stretchedHeight = await mark.evaluate(node => new Promise(resolve => {
    const list = node.closest("ul");
    const observer = new MutationObserver(() => {
      if (parseFloat(node.style.height) <= 24) return;
      observer.disconnect();
      list.querySelector('[aria-label="性能与散热"]').click();
      resolve(parseFloat(node.style.height));
    });
    observer.observe(node, { attributes: true, attributeFilter: ["style"] });
    list.querySelector('[aria-label="设备配置"]').click();
  }));
  expect(stretchedHeight).toBeGreaterThan(24);
  await expectIndicatorAt(page, "性能与散热");
  await expect(entry(page, "性能与散热")).toHaveAttribute("aria-current", "page");
  await expect(entry(page, "设备配置").locator('[data-weight="fill"]')).toHaveCSS("opacity", "0");
  await entry(page, "设置").click();
  await expect(rail(page).locator(".ccc-nav-footer .ccc-nav-indicator")).toHaveCount(1);
  await expectIndicatorAt(page, "设置");
  await entry(page, "键盘与灯光").click();
  await expectIndicatorAt(page, "键盘与灯光");
  await expect(rail(page).locator(".ccc-nav-footer .ccc-nav-indicator")).toHaveCount(0);
});

test("窄窗自动收起不覆盖偏好，短窗内蓝条随主列表滚动", async ({ page }) => {
  await page.setViewportSize({ width: 700, height: 220 });
  await expect(sidebar(page)).toHaveCSS("width", "56px");
  await expect(page.getByRole("button", { name: "展开侧边栏" })).toHaveCount(0);
  await entry(page, "设备配置").click();
  await expectIndicatorAt(page, "设备配置");
  await expect(entry(page, "设置")).toBeInViewport();
  expect(await page.evaluate(() => localStorage.getItem("ccc-sidebar-collapsed"))).toBeNull();
  await page.setViewportSize({ width: 701, height: 720 });
  await expect(sidebar(page)).toHaveCSS("width", "224px");
  await page.getByRole("button", { name: "收起侧边栏" }).click();
  await page.reload();
  await expect(sidebar(page)).toHaveCSS("width", "56px");
  await expect(page.getByRole("button", { name: "展开侧边栏" })).toHaveAttribute("aria-expanded", "false");
});

test("应用减少动效设置终止正在移动的指示条", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  const root = page.locator("html");
  await root.evaluate(element => { element.dataset.motion = "system"; });
  await indicator(page).evaluate(node => new Promise(resolve => {
    const observer = new MutationObserver(() => {
      if (parseFloat(node.style.height) <= 24) return;
      observer.disconnect();
      document.documentElement.dataset.motion = "reduce";
      resolve();
    });
    observer.observe(node, { attributes: true, attributeFilter: ["style"] });
    node.closest("ul").querySelector('[aria-label="设备配置"]').click();
  }));
  await expectIndicatorAt(page, "设备配置");
  await expect(sidebar(page).locator("..")).toHaveCSS("transition-duration", "0s");
  await entry(page, "设置").click();
  await page.getByRole("radio", { name: "深色", exact: true }).click();
  await expect(root).toHaveAttribute("data-theme", "dark");
  await expect(indicator(page)).toHaveCSS("background-color", "rgb(141, 180, 255)");
});
