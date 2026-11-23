import { openWorkspace } from "./workspace.mjs";
import { availableAction } from "./configuration-review.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "预设模式", exact: true });
const artworkFor = page => panel(page).locator("svg.ccc-preset-artwork");
const preset = (page, name) => panel(page).getByRole("radio", { name, exact: true });
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); });

test("五档预设在窄窗口内完整显示，静音增强沿用冷蓝色", async ({ page }) => {
  await page.addInitScript(() => {
    window.__uiFixture.performanceOptions.currentMode = 0;
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      const result = await invoke(command, args);
      return command === "read_oem_power_mode" ? { ...result, availableModes: [...result.availableModes, 4] } : result;
    };
  });
  await page.goto("/");
  await openWorkspace(page, "性能与散热");
  const presets = panel(page);
  await expect(preset(page, "静音增强")).toBeEnabled();
  await expect(presets.getByRole("radio")).toHaveCount(5);
  await expect(artworkFor(page)).toHaveAttribute("data-preset-palette", "quiet");
  await expect(preset(page, "安静")).toBeChecked();
  await openWorkspace(page, "概览");
  await page.evaluate(() => { window.__uiFixture.performanceOptions.currentMode = 4; });
  await openWorkspace(page, "性能与散热");
  await expect(preset(page, "静音增强")).toBeChecked();
  await expect(presets.locator(".ccc-preset-current")).toHaveText("静音增强");
  await expect(artworkFor(page)).toHaveAttribute("data-preset-palette", "quiet");
  await expect(presets.locator(".ccc-preset-current")).toHaveCSS("background-image", /linear-gradient/);
  for (const [width, columns] of [[1440, 5], [1050, 5], [360, 3]]) {
    await page.setViewportSize({ width, height: 900 });
    const layout = await presets.locator(".ccc-segmented-option").evaluateAll(items => ({
      columns: new Set(items.map(item => item.getBoundingClientRect().left)).size,
      titlesFit: items.every(item => {
        const title = item.querySelector("span").getBoundingClientRect();
        const option = item.getBoundingClientRect();
        return title.left >= option.left && title.right <= option.right;
      }),
    }));
    expect(layout).toEqual({ columns, titlesFit: true });
  }
});

test("回读换档重用同一 SVG，更新轮廓和文字渐变并播放一次入场", async ({ page }, info) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.addInitScript(() => { window.__uiFixture.preferences.motion = "system"; });
  await page.goto("/");
  await openWorkspace(page, "性能与散热");
  const artwork = artworkFor(page);
  const drawing = artwork.locator(".ccc-preset-drawing");
  const current = panel(page).locator(".ccc-preset-current");
  await expect(page.locator("html")).toHaveAttribute("data-motion", "system");
  await expect(artwork).toHaveAttribute("data-preset-palette", "performance");
  expect(await artwork.evaluate(svg => svg.getAnimations({ subtree: true }).length)).toBe(0);
  const gradient = artwork.locator('linearGradient[id$="-body"]');
  const gradientId = await gradient.getAttribute("id");
  const lightStop = gradient.locator("stop").first();
  await expect(lightStop).toHaveCSS("stop-color", "rgb(255, 240, 196)");

  await artwork.evaluate(svg => {
    const freeze = event => {
      if (event.animationName !== "ccc-preset-reveal") return;
      svg.removeEventListener("animationstart", freeze);
      for (const motion of svg.getAnimations({ subtree: true })) {
        motion.pause();
        motion.currentTime = 250;
      }
    };
    svg.addEventListener("animationstart", freeze);
  });
  await preset(page, "娱乐").click();
  await expect(preset(page, "娱乐")).toBeChecked();
  await expect(artwork).toHaveAttribute("data-preset-palette", "entertainment");
  await expect.poll(() => drawing.evaluate(group => group.getAnimations().map(motion => [motion.animationName, motion.playState])))
    .toEqual([["ccc-preset-reveal", "paused"]]);
  await expect(drawing).not.toHaveCSS("opacity", "1");
  await expect(drawing).not.toHaveCSS("transform", "none");
  const intermediate = await lightStop.evaluate(stop => getComputedStyle(stop).stopColor);
  expect(intermediate).not.toBe("rgb(255, 240, 196)");
  expect(intermediate).not.toBe("rgb(243, 223, 255)");
  await artwork.evaluate(svg => { for (const motion of svg.getAnimations({ subtree: true })) motion.play(); });
  await expect(lightStop).toHaveCSS("stop-color", "rgb(243, 223, 255)");

  const silhouettes = new Set();
  const gradients = new Set();
  for (const [name, palette] of [["娱乐", "entertainment"], ["省电", "eco"], ["安静", "quiet"], ["性能", "performance"]]) {
    const option = panel(page).locator(`.ccc-preset-switcher .ccc-preset-label[data-preset-palette="${palette}"]`);
    await expect(option).toHaveCSS("background-image", /linear-gradient/);
    gradients.add(await option.evaluate(label => getComputedStyle(label).backgroundImage));
    if (palette !== "entertainment") await preset(page, name).click();
    await expect(preset(page, name)).toBeChecked();
    await expect(artwork).toHaveAttribute("data-preset-palette", palette);
    await expect(current).toHaveText(`${name}模式`);
    await expect(current).toHaveCSS("background-image", /linear-gradient/);
    await expect(artwork).not.toHaveAttribute("data-switching", "true");
    silhouettes.add(JSON.stringify(await drawing.locator("path").evaluateAll(paths => paths.map(path => path.getAttribute("d")))));
    await panel(page).screenshot({ path: info.outputPath(`preset-${palette}.png`) });
  }
  expect(silhouettes.size).toBe(4);
  expect(gradients.size).toBe(4);
  await expect(lightStop).toHaveCSS("stop-color", "rgb(255, 240, 196)");
  await expect(gradient).toHaveAttribute("id", gradientId);
  await page.emulateMedia({ forcedColors: "active" });
  await expect(current).toHaveCSS("background-image", "none");
  await expect(current).not.toHaveCSS("color", "rgba(0, 0, 0, 0)");
  await expect(artwork).not.toHaveAttribute("data-switching", "true");
});

for (const [name, systemMotion, appMotion] of [["系统", "reduce", "system"], ["应用", "no-preference", "reduce"]]) {
  test(`${name}减少动效时预设直接换色`, async ({ page }) => {
    await page.emulateMedia({ reducedMotion: systemMotion });
    await page.addInitScript(motion => { window.__uiFixture.preferences.motion = motion; }, appMotion);
    await page.goto("/");
    await openWorkspace(page, "性能与散热");
    await expect(page.locator("html")).toHaveAttribute("data-motion", appMotion);
    const artwork = artworkFor(page);
    await expect(artwork).toHaveAttribute("data-preset-palette", "performance");
    expect(await artwork.evaluate(svg => parseFloat(getComputedStyle(svg).getPropertyValue("--ccc-motion-preset")))).toBe(0);
    await preset(page, "娱乐").click();
    await expect(artwork).toHaveAttribute("data-preset-palette", "entertainment");
    await expect(artwork).not.toHaveAttribute("data-switching", "true");
    expect(await artwork.evaluate(svg => svg.getAnimations({ subtree: true }).length)).toBe(0);
    await expect(artwork.locator(".ccc-preset-drawing")).toHaveCSS("opacity", "1");
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await page.evaluate(() => { document.documentElement.dataset.motion = "system"; });
    expect(await artwork.evaluate(svg => svg.getAnimations({ subtree: true }).length)).toBe(0);
  });
}

test.describe("触屏设备", () => {
  test.use({ hasTouch: true, isMobile: true, reducedMotion: "no-preference", viewport: { width: 430, height: 800 } });

  test("轻触也能在回读后触发插画入场", async ({ page }) => {
    await page.addInitScript(() => { window.__uiFixture.preferences.motion = "system"; });
    await page.goto("/");
    await openWorkspace(page, "性能与散热");
    const artwork = artworkFor(page);
    await expect(artwork).toHaveAttribute("data-preset-palette", "performance");
    await artwork.evaluate(svg => {
      svg.addEventListener("animationstart", event => {
        if (event.animationName === "ccc-preset-reveal") window.__presetTouchReveal = true;
      });
    });
    await preset(page, "省电").tap();
    await expect(preset(page, "省电")).toBeChecked();
    await expect(artwork).toHaveAttribute("data-preset-palette", "eco");
    await expect.poll(() => page.evaluate(() => window.__presetTouchReveal)).toBe(true);
  });
});

for (const product of [20720, 20724]) test(`未支持机型${product}保留当前模式，关闭编辑并说明范围`, async ({ page }) => {
  await page.addInitScript(product => { window.__uiFixture.unsupportedModeProduct = product; }, product);
  await page.goto("/"); await openWorkspace(page, "性能与散热");
  await expect(panel(page)).toContainText(`尚未支持${product}机型`);
  await expect(panel(page).getByRole("radio", { name: "性能", exact: true })).toBeChecked();
  for (const radio of await panel(page).getByRole("radio").all()) await expect(radio).toBeDisabled();
  await expect(availableAction(page, "预设模式")).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|restore_)/.test(call.command)))).toEqual([]);
});

test("机型变化后后端拒绝先前可用的模式目标", async ({ page }) => {
  await page.goto("/"); await openWorkspace(page, "概览");
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
