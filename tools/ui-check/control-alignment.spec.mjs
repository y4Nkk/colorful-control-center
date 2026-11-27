import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { colorButton } from "./color-picker.mjs";
import { openWorkspace, openPowerArea } from "./workspace.mjs";
import { profileSaveDialog, saveProfileButton } from "./profile-review.mjs";

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
});

async function bounds(...controls) {
  return Promise.all(controls.map(async control => {
    await expect(control).toBeVisible();
    return control.boundingBox();
  }));
}

const bottom = box => box.y + box.height;
const center = box => box.y + box.height / 2;

function expectAligned(positions) {
  expect(Math.max(...positions) - Math.min(...positions)).toBeLessThan(1);
}

test("配置档案的创建入口和保存弹窗在宽窄窗口保持对齐", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 720 });
  await openPowerArea(page, "配置档案");
  const capture = page.getByRole("button", { name: "保存当前配置", exact: true });
  const importFile = page.getByRole("button", { name: "导入档案", exact: true });
  expectAligned((await bounds(capture, importFile)).map(bottom));
  await capture.click();
  const editor = profileSaveDialog(page);
  const name = editor.getByRole("textbox", { name: "档案名", exact: true });
  const cancel = editor.getByRole("button", { name: "取消", exact: true });
  const save = saveProfileButton(page);
  await expect(save).toBeDisabled();
  await name.fill("隔离档案对齐测试");
  await expect(save).toBeEnabled();
  await page.setViewportSize({ width: 360, height: 720 });
  const [inputBox, cancelBox, saveBox] = await bounds(name, cancel, save);
  expect(saveBox.y).toBeGreaterThan(bottom(inputBox));
  expectAligned([cancelBox.y, saveBox.y]);
  expect(saveBox.x + saveBox.width).toBeLessThan(360);
});

test("同色灯光对齐主控件中心，窄窗按组换行", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await openWorkspace(page, "键盘与灯光");
  const panel = page.getByRole("region", { name: "键盘灯光", exact: true });
  const effect = panel.getByRole("combobox", { name: "灯效", exact: true });
  await effect.click();
  await page.getByRole("option", { name: "同色", exact: true }).click();
  const color = colorButton(panel, "键盘颜色");
  const brightness = panel.getByRole("group", { name: "亮度", exact: true });
  const [wideEffect, wideColor, wideBrightness] = await bounds(effect, color, brightness);
  expectAligned([wideEffect, wideColor, wideBrightness].map(center));
  expect(wideBrightness.x).toBeGreaterThan(wideColor.x + wideColor.width);

  await page.setViewportSize({ width: 760, height: 720 });
  const [effectBox, colorBox, brightnessBox] = await bounds(effect, color, brightness);
  expectAligned([effectBox, colorBox].map(center));
  expect(brightnessBox.y).toBeGreaterThan(bottom(colorBox));

  await page.setViewportSize({ width: 360, height: 720 });
  const [stackedEffect, stackedColor, stackedBrightness] = await bounds(effect, color, brightness);
  expect(stackedColor.y).toBeGreaterThan(bottom(stackedEffect));
  expect(stackedBrightness.y).toBeGreaterThan(bottom(stackedColor));
});

test("分区灯光在宽窗紧凑并排，窄窗按组换行", async ({ page }) => {
  await openWorkspace(page, "键盘与灯光");
  const panel = page.getByRole("region", { name: "键盘灯光", exact: true });
  const controls = [
    panel.getByRole("tablist", { name: "编辑灯区" }),
    panel.getByRole("checkbox", { name: "左区开关" }),
    panel.getByRole("slider", { name: "色相" }),
    panel.locator(".ccc-lighting-color-caption"),
  ];
  const board = panel.locator(".ccc-lighting-board");
  const right = box => box.x + box.width;
  for (const width of [1440, 900]) {
    await page.setViewportSize({ width, height: 960 });
    const [tabs, toggle, wheel, caption, preview] = await bounds(...controls, board);
    expectAligned([tabs, toggle, wheel, caption].map(center));
    expectAligned([tabs.x, preview.x]);
    const gaps = [toggle.x - right(tabs), wheel.x - right(toggle), caption.x - right(wheel), preview.y - bottom(wheel)];
    for (const gap of gaps) {
      expect(gap).toBeGreaterThan(0);
      expect(gap).toBeLessThanOrEqual(32);
    }
  }

  for (const width of [390, 320]) {
    await page.setViewportSize({ width, height: 820 });
    const [tabs, toggle, wheel, caption, preview] = await bounds(...controls, board);
    expect(Math.min(toggle.y, wheel.y)).toBeGreaterThan(bottom(tabs));
    expect(caption.y).toBeGreaterThan(bottom(wheel));
    expect(preview.y).toBeGreaterThan(bottom(caption));
    expect(Math.max(...[tabs, toggle, wheel, caption].map(right))).toBeLessThanOrEqual(right(preview));
  }
});

test("主操作在浅深主题共用操作蓝、胶囊形和焦点环", async ({ page }) => {
  await openPowerArea(page, "配置档案");
  const button = page.getByRole("button", { name: "保存当前配置", exact: true });
  await expect(button).toBeEnabled();

  const colors = () => button.evaluate(element => {
    const context = document.createElement("canvas").getContext("2d");
    const luminance = color => {
      context.clearRect(0, 0, 1, 1);
      context.fillStyle = color;
      context.fillRect(0, 0, 1, 1);
      const [red, green, blue, alpha] = context.getImageData(0, 0, 1, 1).data;
      if (alpha !== 255) throw new Error(`主按钮颜色应为实色：${color}`);
      const linear = channel => channel <= 10 ? channel / (255 * 12.92) : ((channel / 255 + 0.055) / 1.055) ** 2.4;
      return 0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue);
    };
    const { color, backgroundColor, backgroundImage, boxShadow } = getComputedStyle(element);
    const [dark, light] = [luminance(color), luminance(backgroundColor)].sort((a, b) => a - b);
    return { backgroundColor, backgroundImage, boxShadow, contrast: (light + 0.05) / (dark + 0.05) };
  });

  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    const normal = await colors();
    await button.hover();
    const hover = await colors();
    await page.mouse.down();
    try {
      const pressed = await colors();
      for (const { backgroundColor, contrast, backgroundImage, boxShadow } of [normal, hover, pressed]) {
        expect(backgroundColor).toBe("rgb(58, 131, 247)");
        expect(contrast).toBeGreaterThanOrEqual(3);
        expect(backgroundImage).toBe("none");
        expect(boxShadow).toBe("none");
      }
    } finally {
      await page.mouse.move(0, 0);
      await page.mouse.up();
    }
    await page.keyboard.press("Tab");
    await button.focus();
    await expect(button).toHaveCSS("border-radius", "9999px");
    await expect(button).toHaveCSS("outline-style", "solid");
    await expect(button).toHaveCSS("outline-width", "2px");
    await expect(button).toHaveCSS("outline-color", "rgb(0, 113, 227)");
  }
});

test("各工作区的按钮及可操作控件在浅深主题使用扁平色面", async ({ page }) => {
  const controls = page.locator([
    ".ccc-button", ".ccc-tabs-tab", ".ccc-segmented", ".ccc-segmented-option", ".ccc-checkbox-box",
    ".ccc-switch-track", ".ccc-switch-thumb", ".ccc-radio", ".ccc-field", "label.ccc-surface",
    '.ccc-range:not([data-appearance="hue"]) .ccc-range-track', ".ccc-range-thumb", ".ccc-lighting-board-key",
  ].join(", "));
  const workspaces = [
    ["概览", "当前模式"], ["性能与散热", "预设模式"], ["键盘与灯光", "键盘灯光"],
    ["电源与档案", "电池与充电"], ["设备配置", "屏幕显示"], ["设置", "外观"],
  ];
  for (const [pageName, heading] of workspaces) {
    await openWorkspace(page, pageName);
    await expect(page.getByRole("heading", { name: heading, exact: true })).toBeVisible();
    for (const theme of ["light", "dark"]) {
      await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
      const raised = await controls.evaluateAll(elements => elements.filter(element => element.getClientRects().length > 0)
        .filter(element => {
          const style = getComputedStyle(element);
          return style.boxShadow !== "none" || style.backgroundImage !== "none" || style.filter.includes("drop-shadow");
        }).map(element => element.getAttribute("aria-label") ?? element.className));
      expect(raised, `${pageName} · ${theme}`).toEqual([]);
    }
  }
});

test("实底面板与工作区分层，较高对比度和减少透明度可辨", async ({ page }) => {
  const shell = page.locator(".ccc-shell");
  const workspace = page.getByRole("main", { name: "概览", exact: true });
  const panel = workspace.getByRole("region", { name: "当前模式", exact: true });
  const transparent = "rgba(0, 0, 0, 0)";

  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    const paint = await workspace.evaluate(node => getComputedStyle(node).backgroundColor);
    await expect(panel).not.toHaveCSS("background-color", paint);
    await expect(panel).not.toHaveCSS("border-top-color", transparent);
    await expect(panel).toHaveCSS("border-top-width", "1px");
    await expect(panel).toHaveCSS("border-radius", "18px");
    await expect(panel).toHaveCSS("box-shadow", "none");
  }

  await page.emulateMedia({ contrast: "more" });
  await expect(panel).not.toHaveCSS("border-top-color", transparent);
  await page.emulateMedia({ forcedColors: "active", contrast: "no-preference" });
  await expect(panel).not.toHaveCSS("border-top-color", transparent);
  await page.emulateMedia({ forcedColors: "none" });

  const session = await page.context().newCDPSession(page);
  await session.send("Emulation.setEmulatedMedia", {
    features: [{ name: "prefers-reduced-transparency", value: "reduce" }],
  });
  await expect(shell).toHaveCSS("backdrop-filter", "none");
  await expect(shell).toHaveCSS("background-image", "none");
});
