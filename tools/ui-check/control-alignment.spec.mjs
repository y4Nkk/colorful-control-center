import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { colorButton } from "./color-picker.mjs";
import { openSystemArea, openWorkspace } from "./workspace.mjs";
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
  await openSystemArea(page, "配置档案");
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

test("软件风扇曲线的数值框与交还固件按钮底部对齐", async ({ page }) => {
  await page.setViewportSize({ width: 900, height: 720 });
  await openSystemArea(page, "风扇散热");
  await page.getByRole("radio", { name: "软件曲线", exact: true }).check();
  const hysteresis = page.getByRole("spinbutton", { name: "软件曲线回差" });
  const ramp = page.getByRole("spinbutton", { name: "软件曲线斜坡" });
  const handBack = page.getByRole("button", { name: "交还固件", exact: true });
  expectAligned((await bounds(hysteresis, ramp, handBack)).map(bottom));
});

test("三区灯光对齐主控件中心，窄窗按组换行", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await openWorkspace(page, "键盘与灯光");
  const panel = page.getByRole("region", { name: "键盘灯光", exact: true });
  const effect = panel.getByRole("combobox", { name: "灯效", exact: true });
  const color = colorButton(panel, "左区颜色");
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
