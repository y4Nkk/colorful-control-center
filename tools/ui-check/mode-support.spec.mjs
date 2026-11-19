import { openSystemArea } from "./workspace.mjs";
import { availableAction } from "./configuration-review.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "预设模式", exact: true });
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); });

test("静音增强复用安静的月牙插画", async ({ page }) => {
  await page.addInitScript(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      const result = await invoke(command, args);
      return command === "read_oem_power_mode" ? { ...result, availableModes: [...result.availableModes, 4] } : result;
    };
  });
  await page.goto("/");
  await openSystemArea(page, "性能调校");
  const presets = panel(page);
  await expect(presets.getByRole("radio", { name: "静音增强", exact: true })).toBeEnabled();
  await expect(presets.getByRole("radio")).toHaveCount(5);
  const quiet = presets.locator('svg[data-mode-art="quiet"]');
  await expect(quiet).toHaveCount(2);
  const [quietArt, enhancedArt] = await quiet.evaluateAll(items => items.map(svg => svg.innerHTML));
  expect(quietArt).toBe(enhancedArt);
  for (const [width, columns] of [[1440, 5], [1050, 5], [360, 2]]) {
    await page.setViewportSize({ width, height: 900 });
    const layout = await presets.locator(".ccc-mode-option").evaluateAll(items => ({
      columns: new Set(items.map(item => item.getBoundingClientRect().left)).size,
      titlesFit: items.every(item => {
        const title = item.querySelector(".ccc-mode-copy");
        return title.scrollWidth <= title.clientWidth + 1 && title.getBoundingClientRect().right <= item.getBoundingClientRect().right;
      }),
    }));
    expect(layout).toEqual({ columns, titlesFit: true });
  }
});

test("预设卡只在悬停时改变对应图形，移开复位并遵循减少动效", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.addInitScript(() => { window.__uiFixture.preferences.motion = "system"; });
  await page.goto("/");
  await openSystemArea(page, "性能调校");

  const presets = panel(page);
  const cards = presets.locator(".ccc-mode-option");
  const title = presets.getByRole("heading", { name: "预设模式" });
  const frame = () => readArtworkFrames(cards);
  await expect(page.locator("html")).toHaveAttribute("data-motion", "system");
  await expect(cards).toHaveCount(4);
  const rest = await frame();
  const changed = async () => (await frame()).flatMap((style, index) => style === rest[index] ? [] : [index]);

  for (const index of [0, 1, 3]) {
    await cards.nth(index).hover();
    await expect.poll(changed).toEqual([index]);
    if (index === 3) {
      await cards.nth(index).locator(".ccc-mode-art-needle").evaluate(async part => {
        await Promise.all(part.getAnimations().map(animation => animation.finished));
      });
      await presets.screenshot({ path: "target/ui-check/preset-modes-hover-performance.png" });
    }
    await title.hover();
    await expect.poll(frame).toEqual(rest);
  }

  const controller = cards.nth(2);
  await controller.evaluate(card => {
    window.__presetButtonEvents = [];
    const record = event => {
      if (event.animationName !== "ccc-mode-keypress") return;
      const index = [...event.target.parentElement.children].indexOf(event.target);
      window.__presetButtonEvents.push(`${event.type}:${index}`);
    };
    card.addEventListener("animationstart", record);
    card.addEventListener("animationend", record);
  });
  await controller.hover();
  await expect.poll(() => page.evaluate(() => window.__presetButtonEvents)).toEqual([
    "animationstart:0", "animationstart:1", "animationend:0", "animationend:1",
  ]);
  await expect.poll(frame).toEqual(rest);
  expect(await controller.locator("svg[data-mode-art]").evaluate(svg => svg.getAnimations({ subtree: true }).length)).toBe(0);
  await title.hover();

  await cards.first().hover();
  await expect.poll(changed).toEqual([0]);
  await page.emulateMedia({ reducedMotion: "reduce" });
  await expect.poll(frame).toEqual(rest);
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await expect.poll(changed).toEqual([0]);
  await page.evaluate(() => { document.documentElement.dataset.motion = "reduce"; });
  await expect.poll(frame).toEqual(rest);
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "apply_oem_power_mode"))).toEqual([]);
  await page.evaluate(() => { document.documentElement.dataset.motion = "system"; });
  await openSystemArea(page, "概览");
  await page.evaluate(async () => {
    const sample = await window.__TAURI_INTERNALS__.invoke("read_hardware_snapshot");
    window.__uiEmit("hardware-snapshot", sample);
  });
  const current = page.getByRole("region", { name: "当前模式", exact: true });
  const currentArtwork = current.locator("svg[data-mode-art]");
  await expect(currentArtwork).toHaveAttribute("data-mode-art", "performance");
  await current.hover();
  expect(await currentArtwork.evaluate(svg => svg.getAnimations({ subtree: true }).length)).toBe(0);
});

function readArtworkFrames(cards) {
  return cards.locator("svg[data-mode-art]").evaluateAll(items => items.map(svg =>
    [...svg.querySelectorAll(".ccc-mode-art-leaf, .ccc-mode-art-moon-light, .ccc-mode-art-button, .ccc-mode-art-needle")]
      .map(part => {
        const { transform, opacity } = getComputedStyle(part);
        return `${transform} / ${opacity}`;
      }).join(" | ")));
}

test.describe("触屏设备", () => {
  test.use({ hasTouch: true, isMobile: true, reducedMotion: "no-preference", viewport: { width: 430, height: 800 } });

  test("轻触切换预设卡片时图案保持静态", async ({ page }) => {
    await page.addInitScript(() => { window.__uiFixture.preferences.motion = "system"; });
    await page.goto("/");
    await openSystemArea(page, "性能调校");
    const eco = panel(page).getByRole("radio", { name: "省电", exact: true });
    await eco.tap();
    await expect(eco).toBeChecked();
    const svg = panel(page).locator('svg[data-mode-art="eco"]');
    expect(await svg.evaluate(item => item.getAnimations({ subtree: true }).length)).toBe(0);
  });
});

for (const product of [20720, 20724]) test(`未支持机型${product}保留当前模式，关闭编辑并说明范围`, async ({ page }) => {
  await page.addInitScript(product => { window.__uiFixture.unsupportedModeProduct = product; }, product);
  await page.goto("/"); await openSystemArea(page, "性能调校");
  await expect(panel(page)).toContainText(`尚未支持${product}机型`);
  await expect(panel(page).getByRole("radio", { name: "性能", exact: true })).toBeChecked();
  for (const radio of await panel(page).getByRole("radio").all()) await expect(radio).toBeDisabled();
  await expect(availableAction(page, "预设模式")).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|restore_)/.test(call.command)))).toEqual([]);
});

test("机型变化后后端拒绝先前可用的模式目标", async ({ page }) => {
  await page.goto("/"); await openSystemArea(page, "概览");
  const result = await page.evaluate(async () => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    const before = await invoke("read_oem_power_mode", {});
    window.__uiFixture.unsupportedModeProduct = 20720;
    const result = await invoke("apply_oem_power_mode", { expected: before.targets.find(target => target.mode === 3) });
    return { result, initial: window.__uiFixture.modeInitial ?? null, current: window.__uiFixture.performanceOptions.currentMode };
  });
  expect(result.result.ok).toBe(false);
  expect(result.result.message).toContain("尚未支持");
  expect(result.initial).toBeNull(); expect(result.current).toBe(2);
});
