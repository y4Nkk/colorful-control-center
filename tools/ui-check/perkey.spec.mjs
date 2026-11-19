import { availableAction, configurationView, previewConfiguration, submitConfiguration } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { colorButton, setPickerColor } from "./color-picker.mjs";

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => { window.__uiFixture.family = "perkey"; });
  await page.goto("/"); await openWorkspace(page, "键盘与灯光");
});
const panelOf = page => page.getByRole("region", { name: "逐键 RGB", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(c => c.command === "apply_perkey_lighting").length);
const choose = async (panel, label, value) => {
  await panel.getByRole("combobox", { name: label, exact: true }).click();
  await panel.page().getByRole("option", { name: value, exact: true }).click();
};
const apply = async panel => {
  const page = panel.page();
  await submitConfiguration(await previewConfiguration(page, "逐键 RGB"));
};

test("真实大键映射、多模式草稿与确认恢复", async ({ page }) => {
  const panel = panelOf(page);
  await panel.getByRole("button", { name: /^空格 · 灯位/ }).click();
  await expect(panel).toContainText("已选 4 个灯位");
  await setPickerColor(panel, "逐键颜色", "#112233");
  await panel.getByRole("button", { name: "填充所选键", exact: true }).click();
  await previewConfiguration(page, "逐键 RGB");
  expect(await writes(page)).toBe(0);
  await page.keyboard.press("Escape");
  await choose(panel, "逐键灯效", "波浪");
  await expect(panel.getByRole("combobox", { name: "逐键灯效" })).toHaveText("波浪");
  await expect(availableAction(page, "逐键 RGB")).toHaveCount(0);
  await choose(panel, "逐键灯效", "逐键静态");
  expect(await writes(page)).toBe(2);
  expect(await page.evaluate(() => window.__uiFixture.perkey.current.pattern.colors[105])).toEqual({ r: 20, g: 80, b: 160 });
  await apply(panel);
  await expect(availableAction(page, "逐键 RGB")).toHaveCount(0);
  const colors = await page.evaluate(() => window.__uiFixture.perkey.current.pattern.colors);
  expect(colors.map((c, i) => c.r === 17 && c.g === 34 && c.b === 51 ? i : -1).filter(i => i >= 0)).toEqual([105, 106, 108, 109]);
  await panel.screenshot({ path: "target/ui-check/perkey-keycaps.png" });
  await previewConfiguration(page, "逐键 RGB", true);
  await configurationView(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  expect(await page.evaluate(() => window.__uiFixture.perkey.current.pattern.colors[105])).toEqual({ r: 20, g: 80, b: 160 });
  await page.setViewportSize({ width: 360, height: 720 });
  expect(await panel.evaluate(el => el.getBoundingClientRect().right <= innerWidth)).toBe(true);
  await panel.screenshot({ path: "target/ui-check/perkey-keycaps-narrow.png" });
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("动态灯效与响应方式即时切换，精确配色单独保存", async ({ page }) => {
  const panel = panelOf(page);
  await choose(panel, "逐键灯效", "涟漪");
  await choose(panel, "按键响应方式", "单键响应");
  await choose(panel, "动态配色", "自定义颜色");
  await setPickerColor(panel, "灯效颜色", "#334455");
  expect(await writes(page)).toBe(3);
  await apply(panel);
  await expect(panel).toContainText("按键响应运行中");
  expect(await page.evaluate(() => window.__uiFixture.perkey.current.pattern)).toEqual({ kind: "ripple", reactive: true, color: { r: 51, g: 68, b: 85 } });
  await page.evaluate(() => { window.__uiFixture.perkey.reactiveRunning = false; window.__uiFixture.perkey.reactiveError = "隔离事件发送失败"; });
  await panel.getByRole("button", { name: "重新读取逐键灯", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("隔离事件发送失败");
  await apply(panel);
  await expect(panel.getByRole("alert")).toHaveCount(0);
  expect(await writes(page)).toBe(5);
});

test("未知布局保留灯位矩阵，旧MCU不展示高级参数", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.perkey.device.compact = null; window.__uiFixture.perkey.device.newEffects = false; });
  const panel = panelOf(page);
  await panel.getByRole("button", { name: "重新读取逐键灯", exact: true }).click();
  await expect(panel).toContainText("键帽布局尚未识别");
  await expect(panel.getByRole("button", { name: /^灯位 \d+-\d+$/ })).toHaveCount(120);
  await choose(panel, "逐键灯效", "波浪");
  await expect(panel.getByRole("combobox", { name: "灯效方向" })).toHaveCount(0);
  await expect(panel.getByRole("combobox", { name: "动态配色" })).toHaveCount(0);
});

test("扫描双色各自编辑并进入同一灯光草稿", async ({ page }) => {
  const panel = panelOf(page);
  await choose(panel, "逐键灯效", "扫描");
  await choose(panel, "动态配色", "自定义双色");
  await setPickerColor(panel, "扫描颜色 2", "#ab1234");
  await expect(colorButton(panel, "扫描颜色 1")).toHaveAccessibleDescription("#0064FF");
  await expect(await previewConfiguration(page, "逐键 RGB")).toContainText("#0064ff / #ab1234");
  expect(await writes(page)).toBe(2);
});

test("动态灯效直接切换失败回读实际模式和恢复资格", async ({ page }) => {
  const panel = panelOf(page);
  await page.evaluate(() => { window.__uiFixture.perkeyFault = "隔离跨页保存失败，已补偿"; });
  await choose(panel, "逐键灯效", "按键渐灭");
  await expect(panel.getByRole("alert")).toContainText("隔离跨页保存失败");
  await expect(panel.getByRole("combobox", { name: "逐键灯效", exact: true })).toHaveText("逐键静态");
  await expect(availableAction(page, "逐键 RGB", true)).toBeEnabled();
  expect(await page.evaluate(() => window.__uiFixture.perkey.current.pattern.kind)).toBe("static");
});

test("四种语言及机型分支使用原厂可见键位集合", async ({ page }) => {
  const panel = panelOf(page);
  const counts = [[102, 83, 102, 102], [103, 83, 103, 102], [107, 88, 104, 104], [105, 83, 105, 102]];
  for (let language = 0; language < 4; language++) for (let variant = 0; variant < 4; variant++) {
    await page.evaluate(({ language, variant }) => Object.assign(window.__uiFixture.perkey.device,
      { language, physicalThreeColumn: variant >= 2, compact: variant % 2 === 1 }), { language, variant });
    await panel.getByRole("button", { name: "重新读取逐键灯", exact: true }).click();
    await expect(panel.locator('button[data-variant="keycap"]')).toHaveCount(counts[language][variant]);
  }
  await page.evaluate(() => Object.assign(window.__uiFixture.perkey.device, { language: 1, physicalThreeColumn: false, compact: false }));
  await panel.getByRole("button", { name: "重新读取逐键灯", exact: true }).click();
  await panel.getByRole("button", { name: "Enter · 灯位 4-16、3-15、3-16", exact: true }).click();
  await expect(panel).toContainText("已选 3 个灯位");
});
