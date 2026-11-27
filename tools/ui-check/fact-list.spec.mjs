import { expect, test } from "@playwright/test";

const list = (page, name) => page.locator(`dl[aria-label='${name}']`);
const sameRow = facts => facts.locator("dt").evaluateAll(labels =>
  labels[0].getBoundingClientRect().top === labels[1].getBoundingClientRect().top);

for (const theme of ["light", "dark"]) {
  test(`事实行在 ${theme} 下统一文字层级，窄窗可完整阅读`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width: 320, height: 650 });
    await page.goto("/tools/ui-check/?fact-list");
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    const rows = list(page, "诊断字段");
    const labels = rows.locator("dt");
    const plain = rows.locator("dd").nth(1);
    await expect(labels.first()).toHaveText("驱动报告的硬件设备识别码");
    await expect(labels.first().locator("svg")).toHaveAttribute("aria-hidden", "true");
    await expect(labels.first()).toHaveCSS("font-weight", "500");
    await expect(plain).toHaveCSS("font-weight", "400");
    await expect(rows.locator("dd strong")).toHaveCSS("font-weight", "600");
    await expect(rows.locator("dd strong")).toHaveCSS("font-variant-numeric", "tabular-nums");
    await expect(rows.locator("dd code")).toContainText("SUBSYS_00000000");
    const labelColor = await labels.first().evaluate(label => getComputedStyle(label).color);
    const plainColor = await plain.evaluate(value => getComputedStyle(value).color);
    const titleColor = await page.getByRole("heading", { name: "设备诊断" }).evaluate(title => getComputedStyle(title).color);
    expect(new Set([labelColor, plainColor, titleColor]).size).toBe(3);

    const geometry = await rows.evaluate(root => {
      const terms = [...root.querySelectorAll("dt")];
      const values = [...root.querySelectorAll("dd")];
      const valueStarts = new Set(values.map(value => value.getBoundingClientRect().left));
      const labelStart = terms[0].lastElementChild.getBoundingClientRect().top;
      const codeStart = values[0].firstElementChild.getBoundingClientRect().top;
      return {
        sharedColumn: valueStarts.size === 1,
        labelFits: terms.every(term => term.getBoundingClientRect().width <= root.clientWidth * 0.4 + 1),
        wraps: values.every(value => value.scrollWidth <= value.clientWidth + 1),
        firstLinesAlign: Math.abs(labelStart - codeStart) <= 6,
      };
    });
    expect(geometry).toEqual({ sharedColumn: true, labelFits: true, wraps: true, firstLinesAlign: true });
    await expect(rows.getByText("已读取", { exact: true })).toHaveAttribute("data-tone", "success");
    await expect(rows.getByText("未识别", { exact: true })).toHaveAttribute("data-tone", "warning");
    await expect(list(page, "采样摘要")).toHaveCSS("flex-wrap", "wrap");
    await expect(list(page, "采样摘要").getByText("已停止", { exact: true })).toHaveAttribute("data-tone", "neutral");
    const tiles = list(page, "待应用参数");
    await expect.poll(() => sameRow(tiles)).toBe(false);
    const comparison = list(page, "档案参数差异");
    const comparisonHeader = page.locator(".ccc-fact-comparison-head");
    const current = comparison.getByRole("group", { name: "当前值", exact: true });
    const target = comparison.getByRole("group", { name: "目标值", exact: true });
    await expect(comparisonHeader).toBeHidden();
    await expect(comparison.getByText("显示试用需保留确认")).toBeVisible();
    await expect(current.last()).toContainText("未读取");
    await expect(target.first().getByText("目标值", { exact: true })).toBeVisible();
    await expect(target.first().locator("strong")).toHaveCSS("font-weight", "600");
    expect(await comparison.evaluate(node => node.scrollWidth - node.clientWidth)).toBeLessThanOrEqual(1);
    await page.screenshot({ path: testInfo.outputPath(`fact-list-${theme}-320.png`), fullPage: true });

    await page.keyboard.press("Tab");
    await expect(rows.getByRole("link")).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page).toHaveURL(/#capture$/);
    await page.setViewportSize({ width: 760, height: 650 });
    await expect.poll(() => sameRow(tiles)).toBe(true);
    await expect(comparisonHeader).toBeVisible();
    for (const [name, cells] of [["当前值", current], ["目标值", target]]) {
      const head = await comparisonHeader.getByText(name, { exact: true }).boundingBox();
      const starts = await cells.evaluateAll(nodes => nodes.map(node => node.getBoundingClientRect().left));
      for (const start of starts) expect(start).toBeCloseTo(head.x);
    }
    await page.screenshot({ path: testInfo.outputPath(`fact-list-${theme}-760.png`), fullPage: true });
  });
}
