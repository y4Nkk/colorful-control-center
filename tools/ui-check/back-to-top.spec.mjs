import { expect, test } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openPowerArea, openWorkspace } from "./workspace.mjs";

const views = [
  ["概览"], ["性能与散热"], ["键盘与灯光"], ["设备配置"],
  ["电源与档案", "电源设置"], ["电源与档案", "配置档案"], ["设置"],
];
const workspaceViewport = page => page.locator(".ccc-workspace > .ccc-scroll-viewport");
const backToTop = page => page.getByRole("button", { name: "返回顶部", exact: true });
const scrollTop = viewport => viewport.evaluate(element => element.scrollTop);

async function openApp(page, motion = "reduce") {
  await page.setViewportSize({ width: 500, height: 480 });
  await page.addInitScript({ content: `${MOCK_IPC}\nwindow.__uiFixture.preferences.motion = ${JSON.stringify(motion)};` });
  await page.goto("/");
  await expect(page.locator("html")).toHaveAttribute("data-motion", motion);
}

async function scrollWorkspace(viewport, top) {
  await expect.poll(() => viewport.evaluate(element => element.scrollHeight - element.clientHeight)).toBeGreaterThanOrEqual(top);
  await viewport.evaluate((element, offset) => { element.scrollTop = offset; }, top);
}

async function openProfile(page) {
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await expect(page.getByRole("region", { name: "档案预览" })).toBeVisible();
}

test("所有工作区与电源子区共用浮动返回顶部按钮", async ({ page }) => {
  await openApp(page);
  const viewport = workspaceViewport(page);
  const button = backToTop(page);
  for (const [title, area] of views) {
    if (area) await openPowerArea(page, area);
    else await openWorkspace(page, title);
    await expect(button).toHaveCount(0);
    if (area === "配置档案") await openProfile(page);

    await scrollWorkspace(viewport, 40);
    const main = page.getByRole("main", { name: title, exact: true });
    await expect(main.getByRole("button", { name: "返回顶部", exact: true })).toBeVisible();
    await expect(button).toHaveCount(1);
    const bounds = await button.boundingBox();
    await viewport.evaluate(element => { element.scrollTop = element.scrollHeight; });
    expect(await button.boundingBox()).toEqual(bounds);

    await button.click();
    await expect.poll(() => scrollTop(viewport)).toBe(0);
    await expect(button).toHaveCount(0);
  }
});

test("键盘回顶更新当前子区的位置，切换时恢复各自的位置", async ({ page }) => {
  await openApp(page);
  const viewport = workspaceViewport(page);
  const button = backToTop(page);
  const powerTab = name => page.getByRole("tab", { name, exact: true });

  await openWorkspace(page, "键盘与灯光");
  await scrollWorkspace(viewport, 80);
  await expect(button).toBeVisible();

  await openPowerArea(page, "电源设置");
  await expect(button).toHaveCount(0);
  await scrollWorkspace(viewport, 120);
  await expect(button).toBeVisible();

  // Activate tabs without Playwright first scrolling the current view to them.
  await powerTab("配置档案").evaluate(tab => tab.click());
  await expect(button).toHaveCount(0);
  await openProfile(page);
  await scrollWorkspace(viewport, 60);
  await expect(button).toBeVisible();

  await powerTab("电源设置").evaluate(tab => tab.click());
  await expect.poll(() => scrollTop(viewport)).toBe(120);
  await button.press("Enter");
  await expect(viewport).toBeFocused();
  await expect(button).toHaveCount(0);

  await powerTab("配置档案").evaluate(tab => tab.click());
  await expect.poll(() => scrollTop(viewport)).toBe(60);
  await expect(button).toBeVisible();
  await powerTab("电源设置").evaluate(tab => tab.click());
  await expect.poll(() => scrollTop(viewport)).toBe(0);
  await expect(button).toHaveCount(0);

  await openWorkspace(page, "键盘与灯光");
  await expect.poll(() => scrollTop(viewport)).toBe(80);
  await expect(button).toBeVisible();
});

for (const [label, motion, systemMotion, instant] of [
  ["默认平滑滚动", "system", "no-preference", false],
  ["应用减少动效", "reduce", "no-preference", true],
  ["系统减少动效", "system", "reduce", true],
]) {
  test(`${label}时的返回顶部`, async ({ page }) => {
    await page.emulateMedia({ reducedMotion: systemMotion });
    await openApp(page, motion);
    await openWorkspace(page, "性能与散热");
    const viewport = workspaceViewport(page);
    await scrollWorkspace(viewport, 160);
    const immediateTop = await backToTop(page).evaluate(button => {
      const viewport = button.previousElementSibling;
      button.click();
      return viewport.scrollTop;
    });
    expect(immediateTop === 0).toBe(instant);
    await expect.poll(() => scrollTop(viewport)).toBe(0);
    await expect(backToTop(page)).toHaveCount(0);
  });
}
