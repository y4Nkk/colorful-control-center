import { expect, test } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { profilePanel } from "./profile-review.mjs";
import { openPowerArea } from "./workspace.mjs";

const builtin = page => page.getByRole("region", { name: "内置档案", exact: true });
const personal = page => page.getByRole("region", { name: "档案列表", exact: true });
const apply = page => profilePanel(page).getByRole("button", { name: "应用档案", exact: true });
const refresh = page => profilePanel(page).getByRole("switch", { name: "随内置档案切换内屏刷新率" });
const trial = page => page.getByRole("region", { name: "档案显示确认", exact: true });
const frequencies = page => page.evaluate(() => window.__uiFixture.display.displays.map(display => display.current.frequency));

async function select(page, name) {
  await builtin(page).getByRole("button", { name: new RegExp(`^${name}`) }).click();
  await expect(profilePanel(page).getByRole("heading", { name, exact: true })).toBeVisible();
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
});

test("档案独立预览在宽窗并排、窄窗纵排，键盘可返回列表", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.evaluate(() => { window.__uiFixture.profile = null; });
  await openPowerArea(page, "配置档案");
  await expect(builtin(page).getByRole("button", { name: /，预览$/ })).toHaveCount(3);
  await expect(personal(page)).toContainText("还没有个人档案");
  await expect(profilePanel(page)).toHaveCount(0);
  const rowLayout = await builtin(page).locator(".ccc-profile-row").evaluateAll(rows => rows.map(row => {
    const name = row.querySelector(".ccc-settings-label").getBoundingClientRect();
    const action = row.querySelector("svg").getBoundingClientRect();
    return [name.left, action.right, name.top];
  }));
  expect(new Set(rowLayout.map(row => row[0])).size).toBe(1);
  expect(new Set(rowLayout.map(row => row[1])).size).toBe(1);
  expect(rowLayout[2][2]).toBeLessThan((await personal(page).boundingBox()).y);
  await page.screenshot({ path: testInfo.outputPath("profile-groups.png") });

  const row = builtin(page).getByRole("button", { name: /^安静办公/ });
  await row.focus();
  await row.press("Enter");
  const pane = profilePanel(page);
  await expect(pane.getByRole("heading", { name: "安静办公", exact: true })).toBeFocused();
  await expect(row).toHaveAttribute("aria-pressed", "true");
  const listBox = await builtin(page).boundingBox();
  const previewBox = await pane.boundingBox();
  expect(previewBox.x).toBeGreaterThan(listBox.x + listBox.width);
  expect(previewBox.y).toBeCloseTo(listBox.y);
  const builtinCard = await builtin(page).locator(".ccc-profile-list").boundingBox();
  const detailCard = await pane.locator(".ccc-profile-detail").boundingBox();
  expect(detailCard.y).toBeCloseTo(builtinCard.y);
  const difference = pane.locator("dl[aria-label='档案设置差异']");
  const header = pane.locator(".ccc-fact-comparison-head");
  await expect(header).toHaveText("设置当前值目标值");
  await expect(difference).toHaveAttribute("data-layout", "comparison");
  await expect(difference.getByText("包含原厂模式联动", { exact: true })).toBeVisible();
  await expect(difference.getByRole("group", { name: "当前值" })).toContainText("性能");
  await expect(difference.getByRole("group", { name: "目标值" })).toContainText("安静");
  const actions = profilePanel(page).locator(".ccc-review-section > .ccc-actions:last-child");
  await expect(actions.getByRole("button", { name: "复制为我的档案" })).toBeVisible();
  await expect(actions.getByRole("button", { name: "应用档案" })).toBeVisible();
  const innerRows = builtin(page).locator(".ccc-profile-list > div").or(pane.locator(
    ".ccc-builtin-refresh, .ccc-builtin-refresh > div, .ccc-review-section > .ccc-actions",
  )).or(header).or(difference.locator("div, dt, dd"));
  const rowsWithDividers = await innerRows.evaluateAll(rows => rows.filter(row => {
    const { borderBlockStartWidth, borderBlockEndWidth } = getComputedStyle(row);
    return borderBlockStartWidth !== "0px" || borderBlockEndWidth !== "0px";
  }).map(row => row.className));
  expect(rowsWithDividers).toEqual([]);
  await pane.getByRole("button", { name: "收起预览" }).click();
  await expect(row).toBeFocused();
  await expect(row).toHaveAttribute("aria-pressed", "false");
  await expect(profilePanel(page)).toHaveCount(0);
  await select(page, "安静办公");
  await expect(header).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath("profile-preview.png") });
  await page.setViewportSize({ width: 360, height: 760 });
  await select(page, "移动续航");
  const narrowList = await personal(page).boundingBox();
  const narrowPreview = await pane.boundingBox();
  expect(narrowPreview.x).toBeCloseTo(narrowList.x);
  expect(narrowPreview.y).toBeGreaterThan(narrowList.y + narrowList.height);
  await expect(pane.getByRole("heading", { name: "移动续航", exact: true })).toBeInViewport();
  await expect(header).toBeHidden();
  const narrowTarget = difference.getByRole("group", { name: "目标值" }).last();
  await expect(narrowTarget.getByText("目标值", { exact: true })).toBeVisible();
  await expect(narrowTarget).toBeInViewport();
  expect(await profilePanel(page).evaluate(panel => panel.scrollWidth - panel.clientWidth)).toBeLessThanOrEqual(1);
  await page.screenshot({ path: testInfo.outputPath("profile-preview-narrow.png") });
  await openPowerArea(page, "电源设置");
  await openPowerArea(page, "配置档案");
  await expect(profilePanel(page).getByRole("heading", { name: "移动续航", exact: true })).toBeVisible();
  await page.evaluate(() => { document.documentElement.dataset.theme = "dark"; });
  await expect(profilePanel(page).getByRole("heading", { name: "移动续航", exact: true })).toHaveCSS("color", "rgb(255, 255, 255)");
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_(builtin_)?profile|save_profile)$/.test(call.command))))
    .toEqual([]);
});

test("内置预览只读，同名个人档案存在时也能另存目标副本", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.profile.name = "安静办公"; });
  await openPowerArea(page, "配置档案");
  await expect(builtin(page).getByRole("button")).toHaveCount(3);
  await expect(personal(page).getByRole("heading", { name: "我的档案" })).toBeVisible();
  await select(page, "安静办公");
  await expect(profilePanel(page).getByRole("button", { name: "更多", exact: true })).toHaveCount(0);
  await expect(refresh(page)).not.toBeChecked();
  await profilePanel(page).getByRole("button", { name: "复制为我的档案", exact: true }).click();
  const copy = page.getByRole("dialog", { name: "复制为我的档案" });
  const name = copy.getByRole("textbox", { name: "档案名" });
  await name.fill("安静办公");
  await expect(copy.getByRole("button", { name: "保存副本" })).toBeDisabled();
  await name.fill("图书馆");
  await copy.getByRole("button", { name: "保存副本" }).click();
  await expect(profilePanel(page).getByRole("heading", { name: "图书馆" })).toBeVisible();
  await expect(profilePanel(page).getByRole("button", { name: "更多" })).toBeVisible();
  const state = await page.evaluate(() => {
    const data = window.__uiFixture;
    return { file: data.profile, existing: data.otherProfiles.map(file => file.name),
      commands: data.calls.filter(call => /^(save_profile|apply_profile|apply_builtin_profile|set_startup_profile|save_power_profile_policy)$/.test(call.command)).map(call => call.command) };
  });
  expect(state.file.settings).toMatchObject({ oemPowerMode: 0, display: null, pl1W: null, gpu: null });
  expect(state.existing).toContain("安静办公");
  expect(state.commands).toEqual(["save_profile"]);
  await openPowerArea(page, "电源设置");
  await page.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await expect(page.getByRole("option", { name: "图书馆", exact: true })).toBeVisible();
  await expect(page.getByRole("option", { name: "插电游戏", exact: true })).toHaveCount(0);
});

test("游戏档在电池或未知供电时阻止应用，插电后可重新预览", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.battery.power.acOnline = false; });
  await openPowerArea(page, "配置档案");
  await select(page, "插电游戏");
  const panel = profilePanel(page);
  await expect(panel.getByRole("alert")).toContainText("当前使用电池供电");
  await expect(apply(page)).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.battery.power.acOnline = null; });
  await panel.getByRole("button", { name: "重新预览" }).click();
  await expect(panel.getByRole("alert")).toContainText("供电状态未知");
  await page.evaluate(() => { window.__uiFixture.battery.power.acOnline = true; });
  await panel.getByRole("button", { name: "重新预览" }).click();
  await expect(apply(page)).toBeEnabled();
  await apply(page).click();
  await expect(panel.getByRole("region", { name: "最近应用记录" })).toContainText("已应用");
  expect(await page.evaluate(() => window.__uiFixture.performanceOptions.currentMode)).toBe(2);
});

test("三份档案共享刷新率选项，确认后只改变内屏", async ({ page }, testInfo) => {
  await page.evaluate(() => {
    const data = window.__uiFixture;
    const panel = data.display.displays[0];
    panel.frequencies = [60, 120, 144]; panel.current.frequency = 144;
    data.display.displays.push({ current: { ...panel.current, device: "DISPLAY2", identity: "external", name: "外接屏", frequency: 120 },
      frequencies: [60, 120, 144], primary: false });
  });
  await openPowerArea(page, "配置档案");
  await select(page, "安静办公");
  await refresh(page).check();
  await profilePanel(page).getByRole("combobox", { name: "办公刷新率" }).click();
  await page.getByRole("option", { name: "120 Hz" }).click();
  await expect(profilePanel(page)).toContainText("续航 60 Hz · 游戏 144 Hz");
  await expect(apply(page)).toBeEnabled();
  expect(await frequencies(page)).toEqual([144, 120]);
  await page.screenshot({ path: testInfo.outputPath("builtin-profiles.png") });
  for (const [name, mode, hz] of [["移动续航", 1, 60], ["插电游戏", 2, 144], ["安静办公", 0, 120]]) {
    await select(page, name);
    await expect(apply(page)).toBeEnabled();
    await apply(page).click();
    await expect(trial(page)).toBeVisible();
    expect(await frequencies(page)).toEqual([hz, 120]);
    await trial(page).getByRole("button", { name: "保留显示并继续档案" }).click();
    await expect(trial(page)).toHaveCount(0);
    await expect(profilePanel(page).getByRole("region", { name: "最近应用记录" })).toContainText("已应用");
    expect(await page.evaluate(() => window.__uiFixture.performanceOptions.currentMode)).toBe(mode);
  }
});

test("显示试用中拔电会撤回游戏刷新率", async ({ page }) => {
  await openPowerArea(page, "配置档案");
  await select(page, "插电游戏");
  await refresh(page).check();
  await expect(apply(page)).toBeEnabled();
  await apply(page).click();
  await expect(trial(page)).toBeVisible();
  expect(await frequencies(page)).toEqual([144]);
  await page.evaluate(() => { window.__uiFixture.battery.power.acOnline = false; });
  await trial(page).getByRole("button", { name: "保留显示并继续档案" }).click();
  await expect(trial(page)).toHaveCount(0);
  await expect(profilePanel(page).getByRole("region", { name: "最近应用记录" })).toContainText("当前使用电池供电");
  expect(await frequencies(page)).toEqual([60]);
});

test("已有自动刷新率管理时只提供原厂模式预览", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.autoRefresh.initial = [0, 0, 144]; });
  await openPowerArea(page, "配置档案");
  await select(page, "移动续航");
  await expect(refresh(page)).toBeDisabled();
  await expect(profilePanel(page)).toContainText("内屏刷新率由「自动刷新率」管理");
  await expect(apply(page)).toBeEnabled();
});
