import { openWorkspace } from "./workspace.mjs";
import { availableAction } from "./configuration-review.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "预设模式", exact: true });
const artworkFor = page => panel(page).locator("svg.ccc-preset-artwork");
const backdropFor = region => region.locator("svg.ccc-preset-backdrop");
const backdropColors = region => backdropFor(region).locator("linearGradient").first().locator("stop")
  .evaluateAll(stops => stops.map(stop => getComputedStyle(stop).stopColor));
const preset = (page, name) => panel(page).getByRole("radio", { name, exact: true });
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); });

test("默认机型的四档预设在宽窄窗口完整显示", async ({ page }) => {
  await page.addInitScript(() => { window.__uiFixture.performanceOptions.currentMode = 0; });
  await page.goto("/");
  const overview = page.getByRole("region", { name: "当前模式", exact: true });
  const overviewBackdrop = backdropFor(overview);
  await expect(overviewBackdrop).toHaveAttribute("data-preset-palette", "quiet");
  await expect(overviewBackdrop.locator("linearGradient").first()).toHaveAttribute("x2", "100%");
  const fade = overviewBackdrop.locator("linearGradient").last();
  await expect(fade).toHaveAttribute("y2", "100%");
  await expect(fade.locator("stop").first()).toHaveCSS("stop-color", "rgb(255, 255, 255)");
  const quietColors = await backdropColors(overview);
  expect(new Set(quietColors).size).toBe(2);
  await openWorkspace(page, "性能与散热");
  const presets = panel(page);
  await expect(presets.getByRole("radio")).toHaveCount(4);
  for (const name of ["省电", "安静", "娱乐", "性能"]) await expect(preset(page, name)).toBeEnabled();
  await expect(artworkFor(page)).toHaveAttribute("data-preset-palette", "quiet");
  await expect(backdropFor(presets)).toHaveAttribute("data-preset-palette", "quiet");
  expect(await backdropColors(presets)).toEqual(quietColors);
  await expect(preset(page, "安静")).toBeChecked();
  await expect(presets.locator(".ccc-preset-current")).toHaveCSS("background-image", "none");
  for (const width of [1440, 1050, 360]) {
    await page.setViewportSize({ width, height: 900 });
    const layout = await presets.locator(".ccc-segmented-option").evaluateAll(items => ({
      columns: new Set(items.map(item => item.getBoundingClientRect().left)).size,
      titlesFit: items.every(item => {
        const title = item.querySelector("span").getBoundingClientRect();
        const option = item.getBoundingClientRect();
        return title.left >= option.left && title.right <= option.right;
      }),
    }));
    expect(layout).toEqual({ columns: 4, titlesFit: true });
  }
  await page.evaluate(() => { document.documentElement.dataset.theme = "dark"; });
  const darkSurface = await presets.evaluate(element => getComputedStyle(element).backgroundColor);
  expect(darkSurface).not.toBe("rgb(255, 255, 255)");
  await expect(backdropFor(presets).locator("linearGradient").last().locator("stop").first())
    .toHaveCSS("stop-color", darkSurface);
  await page.emulateMedia({ forcedColors: "active" });
  await expect(backdropFor(presets)).toBeHidden();
});

test("越界的固件编号显示未知，禁用四档切换", async ({ page }) => {
  await page.addInitScript(() => { window.__uiFixture.performanceOptions.currentMode = 4; });
  await page.goto("/");
  await openWorkspace(page, "性能与散热");
  const presets = panel(page);
  await expect(presets.locator(".ccc-preset-current")).toHaveText("当前模式未知");
  await expect(artworkFor(page)).toHaveAttribute("data-preset-palette", "neutral");
  await expect(backdropFor(presets)).toHaveCount(0);
  await expect(presets.getByRole("radio")).toHaveCount(4);
  for (const radio of await presets.getByRole("radio").all()) {
    await expect(radio).not.toBeChecked();
    await expect(radio).toBeDisabled();
  }
  await expect(presets).toContainText("无法确认恢复前态");
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
  await expect(backdropFor(panel(page))).toHaveAttribute("data-preset-palette", "entertainment");
  expect(await drawing.evaluate(group => group.getAnimations().length)).toBe(0);
  expect(await incoming.locator(".ccc-preset-orbit-light:not([data-orbit])").evaluate(path => path.getAnimations()[0].animationName))
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
  await expect(backdropFor(panel(page))).toHaveAttribute("data-preset-palette", "performance");
  await expect.poll(() => drawing.evaluate(group => group.getAnimations().map(motion => [motion.animationName, motion.playState])))
    .toEqual([["ccc-preset-reveal", "paused"]]);
  await expect(drawing).not.toHaveCSS("opacity", "1");
  await expect(drawing).not.toHaveCSS("transform", "none");
  const outgoing = artwork.locator(".ccc-preset-layer[data-outgoing]");
  await expect(outgoing).toHaveAttribute("data-preset-palette", "entertainment");
  await expect(outgoing.locator(".ccc-preset-drawing")).not.toHaveCSS("opacity", "1");
  await expect(outgoing.locator('linearGradient[id$="-glass"] stop').first()).toHaveCSS("stop-color", "rgb(243, 223, 255)");
  await expect(incoming.locator('linearGradient[id$="-glass"] stop').first()).toHaveCSS("stop-color", "rgb(255, 240, 196)");
  const ribbonPositions = await incoming.locator(".ccc-preset-wind").evaluateAll(ribbons =>
    ribbons.map(ribbon => getComputedStyle(ribbon).transform));
  expect(new Set(ribbonPositions).size).toBe(3);
  await panel(page).screenshot({ path: info.outputPath("preset-entertainment-to-performance.png") });
  await artwork.evaluate(svg => { for (const motion of svg.getAnimations({ subtree: true })) motion.play(); });
  await expect(outgoing).toHaveCount(0);
  const glint = await incoming.locator('[data-wind="main"] .ccc-preset-glint').evaluate(path => {
    const [motion] = path.getAnimations();
    const { delay, duration, iterations } = motion.effect.getTiming();
    motion.pause();
    motion.currentTime = delay + duration * .28;
    const first = getComputedStyle(path).strokeDashoffset;
    motion.currentTime = delay + duration * .56;
    const second = getComputedStyle(path).strokeDashoffset;
    const visible = Number(getComputedStyle(path).opacity) > 0;
    motion.play();
    return { changing: first !== second, visible, iterations };
  });
  expect(glint).toEqual({ changing: true, visible: true, iterations: Infinity });
  await expect(current).toHaveText("性能模式");
  await openWorkspace(page, "概览");
  const hiddenArtwork = page.locator('section[aria-label="预设模式"] .ccc-preset-artwork');
  expect(await hiddenArtwork.evaluate(svg => svg.getAnimations({ subtree: true }).length)).toBe(0);
  const drawings = page.locator("svg.ccc-preset-artwork .ccc-preset-drawing");
  await expect(drawings).toHaveCount(2);
  expect(await drawings.evaluateAll(layers => layers.every(layer => [...layer.querySelectorAll("[href]")].every(node =>
    layer.contains(document.getElementById(node.getAttribute("href").slice(1))))))).toBe(true);
});

test("四档动效各有节奏，主体和动态细节保留在画幅内", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.addInitScript(() => { window.__uiFixture.preferences.motion = "system"; });
  await page.goto("/");
  await openWorkspace(page, "性能与散热");
  const artwork = artworkFor(page);
  const durations = new Set();
  for (const [name, kind, subject, ornament, property] of [
    ["性能", "performance", '[data-wind="main"] .ccc-preset-wind-blade', ".ccc-preset-glint", "stroke-dashoffset"],
    ["娱乐", "entertainment", ".ccc-preset-planet", '[data-orbit="rear"]', "stroke-dashoffset"],
    ["省电", "eco", '[data-leaf="main"]', ".ccc-preset-glint", "stroke-dashoffset"],
    ["安静", "quiet", ".ccc-preset-moon", ".ccc-preset-water", "transform"],
  ]) {
    if (kind !== "performance") await preset(page, name).click();
    await expect(artwork).toHaveAttribute("data-preset-palette", kind);
    await expect(artwork).not.toHaveAttribute("data-switching", "true");
    const sample = await artwork.evaluate((svg, { subject, ornament, property }) => {
      const drawing = svg.querySelector(".ccc-preset-drawing");
      const main = svg.querySelector(subject);
      const accent = svg.querySelector(ornament);
      const [primary] = main.getAnimations();
      const { duration } = primary.effect.getTiming();
      const animations = svg.getAnimations({ subtree: true });
      const viewport = svg.getBoundingClientRect();
      animations.forEach(animation => animation.pause());
      const frames = [0, .15, .30, .45, .60, .75, .90].map(phase => {
        animations.forEach(animation => { animation.currentTime = duration * phase; });
        const { x, y, width, height } = main.getBoundingClientRect();
        const bounds = drawing.getBoundingClientRect();
        return { x: x + width / 2, y: y + height / 2, accent: getComputedStyle(accent).getPropertyValue(property),
          fits: bounds.left >= viewport.left && bounds.right <= viewport.right &&
            bounds.top >= viewport.top && bounds.bottom <= viewport.bottom };
      });
      animations.forEach(animation => animation.play());
      const xs = frames.map(frame => frame.x);
      const ys = frames.map(frame => frame.y);
      return { duration, travel: Math.max(Math.max(...xs) - Math.min(...xs), Math.max(...ys) - Math.min(...ys)),
        accentFrames: new Set(frames.map(frame => frame.accent)).size, fits: frames.every(frame => frame.fits) };
    }, { subject, ornament, property });
    expect(sample.travel, name).toBeGreaterThan(6);
    expect(sample.accentFrames, name).toBeGreaterThan(2);
    expect(sample.fits, name).toBe(true);
    durations.add(sample.duration);
  }
  expect(durations.size).toBe(4);
});

test("四档 SVG 保留独立轮廓，档名与选项使用共享文字色", async ({ page }, info) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/");
  await openWorkspace(page, "性能与散热");
  const artwork = artworkFor(page);
  const drawing = artwork.locator(".ccc-preset-drawing");
  const current = panel(page).locator(".ccc-preset-current");
  const silhouettes = new Set();
  const gradients = new Set();
  for (const [name, kind] of [["娱乐", "entertainment"], ["省电", "eco"], ["安静", "quiet"], ["性能", "performance"]]) {
    const option = preset(page, name).locator("..");
    await expect(option).toHaveCSS("background-image", "none");
    await preset(page, name).click();
    await expect(preset(page, name)).toBeChecked();
    await expect(artwork).toHaveAttribute("data-preset-palette", kind);
    await expect(backdropFor(panel(page))).toHaveAttribute("data-preset-palette", kind);
    gradients.add(JSON.stringify(await backdropColors(panel(page))));
    await expect(current).toHaveText(`${name}模式`);
    const titleLines = await option.locator(":scope > span").evaluate(span => {
      const range = document.createRange();
      range.selectNodeContents(span);
      return range.getClientRects().length;
    });
    expect(titleLines, name).toBe(1);
    await expect(current).toHaveCSS("background-image", "none");
    await expect(artwork).not.toHaveAttribute("data-switching", "true");
    silhouettes.add(JSON.stringify(await drawing.locator("path:not(defs path)").evaluateAll(paths =>
      paths.filter(path => path.getAttribute("opacity") !== "0").map(path => path.getAttribute("d")))));
    await panel(page).screenshot({ path: info.outputPath(`preset-${kind}.png`) });
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
    expect(await artwork.evaluate(svg => svg.getAnimations({ subtree: true }).map(motion => motion.animationName).sort()))
      .toEqual(["ccc-preset-orbit", "ccc-preset-orbit", "ccc-preset-planet-drift"]);
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
