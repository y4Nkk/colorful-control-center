import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace, openSystemArea } from "./workspace.mjs";

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
});

async function expectPlotScale(frame, temperatures, measure) {
  await expect(frame.locator(".ccc-plot-measure")).toHaveText(measure);
  await expect(frame.locator(".ccc-plot-unit")).toHaveText("温度 °C");
  await expect(frame.locator(".ccc-plot-axis > span")).toHaveText(temperatures.map(String));
  await expect(frame.locator(".ccc-plot-ylabels > span")).toHaveText(["100%", "80%", "60%", "40%", "20%", "0%"]);
  const alignment = await frame.evaluate(element => {
    const x = [...element.querySelectorAll(".ccc-plot-axis > span")].map(label => label.getBoundingClientRect());
    const y = [...element.querySelectorAll(".ccc-plot-ylabels > span")].map(label => label.getBoundingClientRect());
    const lines = [...element.querySelectorAll(".ccc-plot-grid line")];
    const offsets = lines.map((line, index) => {
      const point = new DOMPoint(Number(line.getAttribute("x1")), Number(line.getAttribute("y1"))).matrixTransform(line.getScreenCTM());
      if (index < x.length) return Math.abs(x[index].left + x[index].width / 2 - point.x);
      const label = y[index - x.length];
      return Math.abs(label.top + label.height / 2 - point.y);
    });
    return {
      offsets,
      tickOverlap: x.slice(1).some((label, index) => label.left < x[index].right),
    };
  });
  expect(alignment.offsets).toHaveLength(temperatures.length + 6);
  expect(Math.max(...alignment.offsets)).toBeLessThan(1);
  expect(alignment.tickOverlap).toBe(false);
}

async function dragPoint(page, frame, handle, temperature, percentage) {
  await handle.scrollIntoViewIfNeeded();
  const origin = await handle.boundingBox();
  const target = await frame.locator("svg").evaluate((svg, { temperature, percentage }) =>
    new DOMPoint(temperature, 100 - percentage).matrixTransform(svg.getScreenCTM()).toJSON(), { temperature, percentage });
  await page.mouse.move(origin.x + origin.width / 2, origin.y + origin.height / 2);
  await page.mouse.down();
  await page.mouse.move(target.x, target.y, { steps: 4 });
  await page.mouse.up();
}

async function expectNoFanWrites(page) {
  const { calls, faults } = await page.evaluate(() => window.__uiFixture);
  expect(calls.filter(({ command }) => /^(apply_|set_fan|restore_fan)/.test(command))).toEqual([]);
  expect(faults).toEqual([]);
}

test("固件两点图的刻度贴合网格，鼠标和键盘同步草稿", async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 720 });
  await page.evaluate(() => {
    const { fan } = window.__uiFixture;
    Object.assign(fan.fans[0], { t1: 51, d1Pct: 23, t2: 60, d2Pct: 32, t3: 80, d3Pct: 33 });
    fan.fans.push({ ...fan.fans[0], id: "gpu1", t1: 15 });
  });
  await openSystemArea(page, "风扇散热");
  const frame = page.locator(".ccc-plot-frame:visible");
  await expectPlotScale(frame, [30, 40, 60, 80, 100], "占空比");
  await expect(frame.locator(".ccc-plot-current")).toHaveCount(0);
  await page.screenshot({ path: "target/ui-check/fan-grid-light-1100.png" });

  const handle = frame.getByRole("slider", { name: /^T3\/D3 可调点/ });
  await handle.press("ArrowUp");
  await expect(page.getByRole("spinbutton", { name: "T3 占空比" })).toHaveValue("34");
  await expect(frame.locator('.ccc-plot-label[data-point="p3"]')).toHaveCSS("opacity", "1");
  await dragPoint(page, frame, handle, 80, 60);
  await expect(handle).toHaveAttribute("aria-valuetext", "温度 80°C，占空比 60%");
  await expect(page.getByRole("spinbutton", { name: "T3 占空比" })).toHaveValue("60");
  await expect(frame.locator(".ccc-plot-current")).toHaveCount(1);
  await expect(frame.locator(".ccc-plot-draft")).toHaveAttribute("d", /L80,40/);
  await expect(page.locator(".ccc-plot-key:visible .ccc-key-item")).toHaveText(["当前设置", "草稿"]);

  await page.getByRole("radio", { name: "GPU 1", exact: true }).check();
  await expectPlotScale(frame, [10, 20, 40, 60, 80, 100], "占空比");
  await expect(frame.locator(".ccc-plot-draft")).toHaveAttribute("d", /^M15,/);
  await expectNoFanWrites(page);
});

test("窄窗深色下十级 RPM 与软件曲线共用网格并保留各自端点", async ({ page }) => {
  await page.setViewportSize({ width: 360, height: 720 });
  await page.evaluate(() => {
    document.documentElement.dataset.theme = "dark";
    window.__uiFixture.fanKind = "rpmTable";
  });
  await openSystemArea(page, "风扇散热");
  const frame = page.locator(".ccc-plot-frame:visible");
  await expectPlotScale(frame, [0, 20, 40, 60, 80, 100], "目标转速");
  await expect(frame.locator(".ccc-plot-draft")).toHaveAttribute("d", /^M0,80 H40 H45 V72.*H100$/);
  await expect(page.locator(".ccc-plot-key:visible")).toContainText("100% = 5000 RPM");
  await page.screenshot({ path: "target/ui-check/fan-grid-rpm-dark-360.png" });

  const rpmPoint = frame.getByRole("slider", { name: "第 1 级温度与转速", exact: true });
  await dragPoint(page, frame, rpmPoint, 20, 20);
  await expect(rpmPoint).toHaveAttribute("aria-valuetext", "20°C，最大转速的 20%");
  await expect(page.getByRole("spinbutton", { name: "RPM 温度" })).toHaveValue("20");

  await page.getByRole("radio", { name: "软件曲线", exact: true }).check();
  await expectPlotScale(frame, [20, 40, 60, 80, 95], "占空比");
  await expect(frame.locator(".ccc-plot-draft")).toHaveAttribute("d", /^M20,70 L40,70.*L95,0$/);
  await expect(page.locator(".ccc-plot-key:visible .ccc-key-item")).toHaveText(["草稿"]);
  const point = frame.getByRole("slider", { name: "软件曲线第 2 点", exact: true });
  await dragPoint(page, frame, point, 60, 60);
  await expect(point).toHaveAttribute("aria-valuetext", "60°C，占空比 60%");
  await expect(page.getByRole("spinbutton", { name: "软件点占空比" })).toHaveValue("60");
  await expect(frame.getByRole("slider", { name: /固定安全端点/ })).toHaveAttribute("aria-valuetext", "95°C，占空比 100%");
  await page.screenshot({ path: "target/ui-check/fan-grid-software-dark-360.png" });
  await expectNoFanWrites(page);
});
