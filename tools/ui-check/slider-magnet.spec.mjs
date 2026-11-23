import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { previewConfiguration } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  await openWorkspace(page, "性能与散热");
});

async function track(page, name) {
  const slider = page.getByRole("slider", { name, exact: true });
  await expect(slider).toBeEnabled();
  await slider.scrollIntoViewIfNeeded();
  const rail = await slider.locator("..").locator(".ccc-range-ticks").boundingBox();
  const { min, max } = await slider.evaluate(field => ({ min: Number(field.min), max: Number(field.max) }));
  const pixelsPerUnit = rail.width / (max - min);
  return { slider, x: value => rail.x + (value - min) * pixelsPerUnit, y: rail.y + rail.height / 2, pixelsPerUnit };
}

const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(({ command }) =>
  /^(set_fan_offset|apply_(saved_cpu|runtime_cpu|gpu))/.test(command)));

test("风扇偏移吸入、保持和脱开，完成时只提交一次", async ({ page }) => {
  const { slider, x, y, pixelsPerUnit } = await track(page, "加速偏移");
  const enter = Math.min(4, 2.4 * pixelsPerUnit);
  const release = Math.min(6, 3.6 * pixelsPerUnit);
  await page.mouse.move(x(50) - enter * 0.8, y);
  await page.mouse.down();
  await expect(slider).toHaveValue("50");
  await page.mouse.move(x(50) + (enter + release) / 2, y);
  await expect(slider).toHaveValue("50");
  await page.mouse.move(x(50) + release + 2, y);
  await expect.poll(async () => Number(await slider.inputValue())).toBeGreaterThan(50);
  await page.mouse.move(x(50), y);
  expect(await writes(page)).toEqual([]);
  await page.mouse.up();
  await expect.poll(() => writes(page)).toEqual([
    { command: "set_fan_offset", args: { expected: { mode: 0, slot: null }, pct: 50 } },
  ]);
  await expect(slider).toBeEnabled();
  await slider.press("ArrowRight");
  await expect(slider).toHaveValue("51");
  await expect.poll(async () => (await writes(page)).map(call => call.args.pct)).toEqual([50, 51]);
});

test("缩放下功耗保持刻度间的细调，吸附草稿进入已有确认", async ({ page }) => {
  await page.setViewportSize({ width: 480, height: 720 });
  await page.evaluate(() => { document.documentElement.style.zoom = "1.25"; });
  const { slider, x, y, pixelsPerUnit } = await track(page, "持续功耗上限");
  await page.mouse.move(x(45) + Math.min(3.2, pixelsPerUnit), y);
  await page.mouse.down();
  await expect(slider).toHaveValue("45");
  await page.mouse.move(x(47.1), y);
  await expect(slider).toHaveValue("47");
  await page.mouse.move(x(50) - Math.min(3.2, pixelsPerUnit), y);
  await page.mouse.up();
  await expect(slider).toHaveValue("50");
  const view = await previewConfiguration(page, "立即应用 · CPU");
  await expect(view.getByText("50 W", { exact: true })).toBeVisible();
  expect(await writes(page)).toEqual([]);
});

test("GPU 偏移可吸到零点，松手后可按键离开刻度", async ({ page }) => {
  const { slider, x, y, pixelsPerUnit } = await track(page, "核心频率偏移");
  await slider.fill("100");
  await page.mouse.move(x(0) - Math.min(3, pixelsPerUnit * 8), y);
  await page.mouse.down();
  await expect(slider).toHaveValue("0");
  await page.mouse.up();
  await slider.press("ArrowLeft");
  await expect(slider).toHaveValue("-1");
  expect(await writes(page)).toEqual([]);
});

test("触摸取消会撤销吸附预览，触摸结束提交当前值", async ({ page, context }) => {
  const { slider, x, y } = await track(page, "加速偏移");
  const cdp = await context.newCDPSession(page);
  await cdp.send("Emulation.setTouchEmulationEnabled", { enabled: true });
  const touch = (type, value) => cdp.send("Input.dispatchTouchEvent",
    { type, touchPoints: value === undefined ? [] : [{ x: x(value), y }] });
  await touch("touchStart", 20);
  await touch("touchMove", 60);
  await expect(slider).toHaveValue("60");
  await touch("touchCancel");
  await expect(slider).toHaveValue("0");
  expect(await writes(page)).toEqual([]);
  await touch("touchStart", 40);
  await expect(slider).toHaveValue("40");
  await touch("touchEnd");
  await expect.poll(async () => (await writes(page)).map(call => call.args.pct)).toEqual([40]);
});
