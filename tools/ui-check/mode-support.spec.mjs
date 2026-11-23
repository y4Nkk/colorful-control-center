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

test("回读从娱乐换到性能时紫色轨迹淡出，风刃分层入场后继续掠光", async ({ page }, info) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.addInitScript(() => {
    window.__uiFixture.preferences.motion = "system";
    window.__uiFixture.performanceOptions.currentMode = 3;
  });
  await page.goto("/");
  await openWorkspace(page, "性能与散热");
  const artwork = artworkFor(page);
  const incoming = artwork.locator(".ccc-preset-layer:not([data-outgoing])");
  const drawing = incoming.locator(".ccc-preset-drawing");
  const current = panel(page).locator(".ccc-preset-current");
  await expect(page.locator("html")).toHaveAttribute("data-motion", "system");
  await expect(artwork).toHaveAttribute("data-preset-palette", "entertainment");
  expect(await drawing.evaluate(group => group.getAnimations().length)).toBe(0);
  expect(await incoming.locator(".ccc-preset-orbit-front").evaluate(path => path.getAnimations()[0].animationName))
    .toBe("ccc-preset-orbit");

  await artwork.evaluate(svg => {
    const freeze = event => {
      if (event.animationName !== "ccc-preset-reveal") return;
      svg.removeEventListener("animationstart", freeze);
      for (const motion of svg.getAnimations({ subtree: true })) {
        motion.pause();
        motion.currentTime = 180;
      }
    };
    svg.addEventListener("animationstart", freeze);
  });
  await preset(page, "性能").click();
  await expect(preset(page, "性能")).toBeChecked();
  await expect(artwork).toHaveAttribute("data-preset-palette", "performance");
  await expect.poll(() => drawing.evaluate(group => group.getAnimations().map(motion => [motion.animationName, motion.playState])))
    .toEqual([["ccc-preset-reveal", "paused"]]);
  await expect(drawing).not.toHaveCSS("opacity", "1");
  await expect(drawing).not.toHaveCSS("transform", "none");
  const outgoing = artwork.locator(".ccc-preset-layer[data-outgoing]");
  await expect(outgoing).toHaveAttribute("data-preset-palette", "entertainment");
  await expect(outgoing.locator(".ccc-preset-drawing")).not.toHaveCSS("opacity", "1");
  await expect(outgoing.locator('linearGradient[id$="-body"] stop').first()).toHaveCSS("stop-color", "rgb(243, 223, 255)");
  await expect(incoming.locator('linearGradient[id$="-body"] stop').first()).toHaveCSS("stop-color", "rgb(255, 240, 196)");
  const ribbonPositions = await incoming.locator(".ccc-preset-wind").evaluateAll(ribbons =>
    ribbons.map(ribbon => getComputedStyle(ribbon).transform));
  expect(new Set(ribbonPositions).size).toBe(3);
  await panel(page).screenshot({ path: info.outputPath("preset-entertainment-to-performance.png") });
  await artwork.evaluate(svg => { for (const motion of svg.getAnimations({ subtree: true })) motion.play(); });
  await expect(outgoing).toHaveCount(0);
  const glint = await incoming.locator(".ccc-preset-wind-light").evaluate(path => {
    const [motion] = path.getAnimations();
    motion.pause();
    motion.currentTime = 2000;
    const first = getComputedStyle(path).strokeDashoffset;
    motion.currentTime = 3100;
    const second = getComputedStyle(path).strokeDashoffset;
    const visible = Number(getComputedStyle(path).opacity) > 0;
    motion.play();
    return { changing: first !== second, visible, iterations: motion.effect.getTiming().iterations };
  });
  expect(glint).toEqual({ changing: true, visible: true, iterations: Infinity });
  await expect(current).toHaveText("性能模式");
  await openWorkspace(page, "概览");
  const hiddenArtwork = page.locator('section[aria-label="预设模式"] .ccc-preset-artwork');
  expect(await hiddenArtwork.evaluate(svg => svg.getAnimations({ subtree: true }).length)).toBe(0);
});

test("各档位使用独立轮廓与档名渐变，并兼容强制色彩", async ({ page }, info) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/");
  await openWorkspace(page, "性能与散热");
  const artwork = artworkFor(page);
  const drawing = artwork.locator(".ccc-preset-drawing");
  const current = panel(page).locator(".ccc-preset-current");
  const silhouettes = new Set();
  const gradients = new Set();
  for (const [name, palette] of [["娱乐", "entertainment"], ["省电", "eco"], ["安静", "quiet"], ["性能", "performance"]]) {
    const option = panel(page).locator(`.ccc-preset-switcher .ccc-preset-label[data-preset-palette="${palette}"]`);
    await expect(option).toHaveCSS("background-image", /linear-gradient/);
    gradients.add(await option.evaluate(label => getComputedStyle(label).backgroundImage));
    await preset(page, name).click();
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
    expect(await artwork.evaluate(svg => svg.getAnimations({ subtree: true }).map(motion => motion.animationName)))
      .toEqual(["ccc-preset-orbit", "ccc-preset-orbit"]);
    await expect(artwork).not.toHaveAttribute("data-switching", "true");
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
