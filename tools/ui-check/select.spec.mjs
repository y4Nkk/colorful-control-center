import { test, expect } from "@playwright/test";

test.beforeEach(async ({ page }) => { await page.goto("/tools/ui-check/index.html"); });
const mode = page => page.getByRole("combobox", { name: "选择模式", exact: true });
const options = page => page.getByRole("listbox", { name: "选择模式", exact: true });

test("普通/搜索菜单同源，普通选择完整键盘与焦点行为", async ({ page }) => {
  const field = mode(page);
  await field.press("ArrowDown");
  await expect(options(page)).toBeVisible();
  await field.press("ArrowDown");
  await expect(page.getByRole("option", { name: "Charlie", exact: true })).toHaveAttribute("data-active", "true");
  await field.press("Escape");
  await expect(page.getByLabel("已选模式")).toHaveText("a"); await expect(field).toBeFocused();
  await field.press("End"); await field.press("Enter");
  await expect(page.getByLabel("已选模式")).toHaveText("d"); await expect(field).toBeFocused();
  await field.press("Home"); await field.press("Enter");
  await expect(page.getByLabel("已选模式")).toHaveText("a");
  await field.press("c"); await field.press("Enter");
  await expect(page.getByLabel("已选模式")).toHaveText("c");
  await field.click(); await field.press("Tab");
  await expect(page.getByRole("textbox", { name: "后一个字段" })).toBeFocused(); await expect(options(page)).toHaveCount(0);
  await field.click(); await field.press("Shift+Tab");
  await expect(page.getByRole("textbox", { name: "前一个字段" })).toBeFocused();
});

test("搜索、空列表、文本编辑键及选项焦点不进入 Tab 序列", async ({ page }) => {
  const field = page.getByRole("combobox", { name: "选择字体", exact: true });
  await field.click();
  const search = page.getByRole("searchbox", { name: "搜索字体" });
  await expect(search).toBeFocused();
  await search.fill("Mono"); await search.press("Home"); await search.press("End");
  await expect(search).toHaveValue("Mono"); await expect(page.getByRole("option")).toHaveCount(1);
  await search.press("Enter"); await expect(page.getByLabel("已选字体")).toHaveText("Monospace");
  await expect(field).toBeFocused();
  await field.click(); await search.fill("not-existing"); await expect(page.locator(".ccc-select-menu").getByRole("status")).toHaveText("没有可选项");
  await search.press("Escape"); await expect(field).toBeFocused(); await expect(page.getByLabel("已选字体")).toHaveText("Monospace");
});

test("禁用、异步选项、读取错误与空值不会虚构选中值", async ({ page }) => {
  await page.getByRole("button", { name: "替换选项" }).click();
  await expect(mode(page)).toHaveText("请选择"); await expect(page.getByLabel("已选模式")).toHaveText("a");
  await mode(page).click(); await page.getByRole("option", { name: "Yankee" }).click();
  await expect(page.getByLabel("已选模式")).toHaveText("y");
  await page.getByRole("button", { name: "模拟读取失败" }).click(); await mode(page).click();
  await expect(page.getByRole("alert")).toHaveText("隔离读取失败");
  await expect(page.getByRole("button", { name: "重试", exact: true })).toBeDisabled();
  await page.getByRole("textbox", { name: "后一个字段" }).click(); await expect(options(page)).toHaveCount(0);
  await page.getByRole("button", { name: "切换禁用" }).click(); await expect(mode(page)).toBeDisabled();
  await page.getByRole("button", { name: "切换禁用" }).click();
  await page.getByRole("button", { name: "清空选项" }).click(); await mode(page).click();
  await expect(page.getByRole("status").last()).toHaveText("没有可选项");
  await expect(page.getByLabel("已选模式")).toHaveText("y");
});

test("共享原语禁用、草稿与两步确认没有第二实现", async ({ page }) => {
  const checkbox = page.getByRole("checkbox", { name: "测试复选" });
  await checkbox.press("Space"); await expect(checkbox).toBeChecked();
  await expect(page.getByRole("switch", { name: "测试开关" })).toBeChecked();
  await page.getByRole("radio", { name: "第一项", exact: true }).press("ArrowRight");
  await expect(page.getByRole("radio", { name: "第二项", exact: true })).toBeChecked();
  await expect(page.getByRole("radio", { name: "不可选项" })).toBeDisabled();
  const slider = page.getByRole("slider", { name: "测试滑杆" });
  await slider.press("ArrowRight"); await expect(page.getByLabel("滑杆数值")).toHaveText("31");
  await page.getByRole("button", { name: "执行隔离操作", exact: true }).click();
  await expect(page.getByLabel("执行次数")).toHaveText("0");
  await expect(page.getByRole("button", { name: "确认执行隔离操作？" })).toHaveAttribute("data-tone", "danger");
  await page.getByRole("button", { name: "确认执行隔离操作？" }).click();
  await expect(page.getByLabel("执行次数")).toHaveText("1");
});

for (const scale of [0.9, 1, 1.1, 1.25]) {
  test(`菜单脱离裁切，${scale} 倍缩放下靠边翻转并随滚动/尺寸变化定位`, async ({ page }) => {
    await page.setViewportSize({ width: 900, height: 560 });
    await page.evaluate(value => {
      document.documentElement.style.zoom = String(value);
      const area = document.getElementById("anchor-area");
      area.style.position = "fixed"; area.style.left = `${720 / value}px`; area.style.top = `${490 / value}px`; area.style.width = `${170 / value}px`;
    }, scale);
    await mode(page).click();
    const menu = page.locator("body > .ccc-select-menu");
    await expect(menu).toHaveAttribute("data-side", "top");
    await expect(menu).toBeVisible();
    const box = await menu.boundingBox(), anchor = await mode(page).boundingBox();
    expect(box.x).toBeGreaterThanOrEqual(0); expect(box.x + box.width).toBeLessThanOrEqual(900);
    expect(box.y + box.height).toBeLessThan(anchor.y + 1);
    await page.getByRole("option", { name: "Charlie", exact: true }).click();
    await expect(page.getByLabel("已选模式")).toHaveText("c");
    await mode(page).click(); await page.setViewportSize({ width: 850, height: 530 });
    await expect.poll(async () => { const rect = await menu.boundingBox(); return rect.x + rect.width; }).toBeLessThanOrEqual(850);
    await page.keyboard.press("Escape");
  });
}

test("工作区滚动时跟随锚点，锚点离开工作区后关闭", async ({ page }) => {
  await mode(page).click();
  const menu = page.locator("body > .ccc-select-menu");
  const before = (await menu.boundingBox()).y;
  await page.getByRole("main").evaluate(el => el.scrollTop += 35);
  await expect.poll(async () => (await menu.boundingBox())?.y).not.toBe(before);
  await page.getByRole("main").evaluate(el => el.scrollTop = 1500);
  await expect(menu).toHaveCount(0);
});
