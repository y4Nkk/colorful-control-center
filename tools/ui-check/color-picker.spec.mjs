import { test, expect } from "@playwright/test";
import { colorButton } from "./color-picker.mjs";

test.beforeEach(async ({ page }) => { await page.goto("/tools/ui-check/index.html"); });
const swatch = page => colorButton(page, "测试颜色");
const editor = page => page.getByRole("dialog", { name: "测试颜色", exact: true });

test("RGB、HEX、色相和色域只提交有效草稿，关闭返回色块焦点", async ({ page }) => {
  const trigger = swatch(page), picker = editor(page);
  await expect(picker).toBeHidden();
  await trigger.press("Enter");
  await expect(picker.getByRole("heading")).toBeFocused();
  const red = picker.getByRole("spinbutton", { name: "R", exact: true });
  const green = picker.getByRole("spinbutton", { name: "G", exact: true });
  const blue = picker.getByRole("spinbutton", { name: "B", exact: true });
  const hex = picker.getByRole("textbox", { name: "HEX", exact: true });
  const hue = picker.getByRole("slider", { name: "色相", exact: true });
  await expect(red).toHaveValue("127");
  await red.fill("18");
  await expect(hex).toHaveValue("#1200CB");
  for (const invalid of ["", "-1", "256", "42.5"]) {
    await red.fill(invalid);
    await expect(red).toHaveAttribute("aria-invalid", "true");
    await expect(trigger).toHaveAccessibleDescription("#1200CB");
  }
  await green.focus();
  await expect(red).toHaveValue("18");
  await hex.fill("#12XZ56");
  await expect(hex).toHaveAttribute("aria-invalid", "true");
  await expect(trigger).toHaveAccessibleDescription("#1200CB");
  await hex.fill("808080");
  for (const channel of [red, green, blue]) await expect(channel).toHaveValue("128");
  await hue.press("End");
  await expect(hex).toHaveValue("#808080");
  await expect(hue).toHaveValue("359");
  const { x, y, width, height } = await picker.locator(".ccc-color-picker-plane").boundingBox();
  await page.mouse.move(x + width * 0.6, y + height * 0.4);
  await page.mouse.down();
  await page.mouse.move(x + width * 0.85, y + height * 0.15, { steps: 4 });
  await page.mouse.up();
  await expect(hue).toHaveValue("359");
  await expect(hex).not.toHaveValue("#808080");
  const chosen = await hex.inputValue();
  await page.keyboard.press("Escape");
  await expect(picker).toBeHidden();
  await expect(trigger).toBeFocused();
  await expect(trigger).toHaveAccessibleDescription(chosen);
  await page.getByRole("button", { name: "切换禁用" }).click();
  await expect(trigger).toBeDisabled();
});

test("色块面板避让上下和两侧边缘，支持短视口与缩放", async ({ page }) => {
  const anchor = page.locator("#color-anchor"), trigger = swatch(page), picker = editor(page);
  for (const [width, height, zoom, side] of [[1000, 720, 1, "below"], [360, 520, 1, "above"], [320, 300, 1.25, "center"]]) {
    await page.setViewportSize({ width, height });
    await anchor.evaluate((element, { zoom, side }) => {
      document.documentElement.style.zoom = String(zoom);
      Object.assign(element.style, { position: "fixed", right: "4px", zIndex: "10",
        top: side === "below" ? "12px" : side === "center" ? "50%" : "auto",
        bottom: side === "above" ? "12px" : "auto" });
    }, { zoom, side });
    await trigger.click();
    await expect(picker).toBeVisible();
    const [popup, button] = await Promise.all([picker.boundingBox(), trigger.boundingBox()]);
    expect(popup.x).toBeGreaterThanOrEqual(0);
    expect(popup.y).toBeGreaterThanOrEqual(0);
    expect(popup.x + popup.width).toBeLessThanOrEqual(width + 1);
    expect(popup.y + popup.height).toBeLessThanOrEqual(height + 1);
    if (side === "below") expect(popup.y).toBeGreaterThan(button.y + button.height);
    if (side === "above") expect(popup.y + popup.height).toBeLessThan(button.y);
    if (side === "center") {
      const hex = picker.getByRole("textbox", { name: "HEX" });
      await hex.scrollIntoViewIfNeeded();
      const input = await hex.boundingBox();
      expect(input.y + input.height).toBeLessThanOrEqual(popup.y + popup.height);
    }
    await colorButton(picker, "关闭选色").click();
    await expect(trigger).toBeFocused();
  }
  await trigger.click();
  await page.mouse.click(1, 1);
  await expect(picker).toBeHidden();
  await trigger.click();
  await expect(picker).toBeVisible();
  await anchor.evaluate(element => { element.hidden = true; });
  await expect(anchor.locator(":popover-open")).toHaveCount(0);
});
