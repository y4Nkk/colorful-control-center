import { expect, test } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { profilePanel } from "./profile-review.mjs";
import { openSystemArea } from "./workspace.mjs";

const openProfiles = page => openSystemArea(page, "配置档案");
const savedSettings = page => page.evaluate(() => window.__uiFixture.calls.find(call => call.command === "save_profile").args.args);
const saveButton = page => page.getByRole("button", { name: "保存当前配置", exact: true });
const fileInput = page => page.getByLabel("导入配置档案", { exact: true });
const mutations = page => page.evaluate(() => window.__uiFixture.calls
  .filter(call => /^(apply_profile|save_profile|delete_profile)$/.test(call.command)).map(call => call.command));

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
});

test("空库只展示真实入口，保存性能组不受其他设备失败影响", async ({ page }, testInfo) => {
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.profile = null;
    data.calibration.recoveryRequired = true;
    data.keyboard.current.effect = null;
    data.fan.currentMode = 6;
  });
  await openProfiles(page);
  await expect(page.getByRole("heading", { name: "还没有配置档案" })).toBeVisible();
  await expect(page.getByRole("region", { name: "档案列表", exact: true })).toHaveCount(0);
  await expect(profilePanel(page)).toHaveCount(0);
  await page.screenshot({ path: testInfo.outputPath("profiles-empty.png") });
  await saveButton(page).click();
  const editor = page.getByRole("dialog", { name: "保存当前配置" });
  await expect(editor.getByRole("checkbox", { name: "性能与散热" })).toBeChecked();
  await expect(editor.getByRole("checkbox", { name: "显示与显卡" })).not.toBeChecked();
  await editor.getByRole("textbox", { name: "档案名" }).fill("夜间静音");
  await expect(editor.getByRole("button", { name: "查看已读取的参数" })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath("profiles-capture.png") });
  await editor.getByRole("button", { name: "保存档案", exact: true }).click();
  await expect(profilePanel(page).getByRole("heading", { name: "夜间静音" })).toBeVisible();
  expect(await savedSettings(page)).toMatchObject({ expectedRevision: null,
    settings: { pl1W: 65, display: null, batteryCharge: null, keyboardLighting: null, lanPolicy: null } });
  await page.evaluate(() => {
    window.__uiFixture.power.pl1W = 45;
    window.__uiFixture.fan.fans[0].t2 = 55;
  });
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /夜间静音/ }).click();
  const row = profilePanel(page).locator(".ccc-profile-values-list > div").filter({ hasText: "持续功耗上限 · 保存值" });
  await expect(row).toContainText("45 W");
  await expect(row).toContainText("65 W");
  await expect(profilePanel(page).getByRole("img", { name: "CPU 当前与档案曲线" }).locator("path")).toHaveCount(2);
  const details = profilePanel(page).getByRole("button", { name: "查看全部设置" });
  await expect(details).toHaveAttribute("aria-expanded", "false");
  await details.click();
  const cpuValue = profilePanel(page).getByLabel("保存设置 · CPU 持续功耗上限", { exact: true });
  await expect(cpuValue).toHaveText("65 W");
  await profilePanel(page).getByRole("radio", { name: "应用前已读取" }).check();
  await expect(cpuValue).toHaveText("45 W");
  await details.click();
  await page.setViewportSize({ width: 360, height: 720 });
  await page.evaluate(() => { document.documentElement.dataset.theme = "dark"; });
  await page.screenshot({ path: testInfo.outputPath("profiles-preview-dark-narrow.png") });
  expect(await mutations(page)).toEqual(["save_profile"]);
  await profilePanel(page).getByRole("button", { name: "应用档案" }).click();
  await expect(profilePanel(page).getByRole("region", { name: "最近应用记录" })).toContainText("已应用");
  expect(await mutations(page)).toEqual(["save_profile", "apply_profile"]);
});

test("覆盖提示关联规则并允许在电源页重新确认同一个档案", async ({ page }) => {
  await page.evaluate(async () => {
    const { profile } = window.__uiFixture;
    const ipc = window.__TAURI_INTERNALS__;
    await ipc.invoke("set_startup_profile", { expected: profile, expectedMode: null });
    const policy = await ipc.invoke("read_power_profile_policy");
    await ipc.invoke("save_power_profile_policy", { request: { expected: policy.record, enabled: false,
      battery: { file: profile, mode: null }, ac: null, resume: null } });
  });
  await openProfiles(page);
  const profile = profilePanel(page);
  await expect(profile.getByRole("button", { name: "查看电源规则" })).toBeVisible();
  await profile.getByRole("button", { name: "更多" }).click();
  await profile.getByRole("button", { name: "用当前配置更新" }).click();
  const editor = page.getByRole("dialog", { name: "用当前配置更新档案" });
  await expect(editor).toContainText("启动时、使用电池时");
  await expect(editor).toContainText("覆盖后，请到电源页重新确认这些规则");
  await editor.getByRole("button", { name: "覆盖档案" }).click();
  await expect(profile).toContainText("档案有更新，请重新确认关联规则");
  const args = await savedSettings(page);
  expect(args.expectedRevision).toMatch(/^[a-f0-9]{64}$/);
  await profile.getByRole("button", { name: "查看电源规则" }).click();
  await expect(page.getByRole("main", { name: "电源", exact: true })).toBeVisible();
  await expect(page.getByText("档案有更新，请核对并保存规则")).toBeVisible();
  await page.getByRole("button", { name: "保存自动档案规则", exact: true }).click();
  await expect(page.getByText("档案有更新，请核对并保存规则")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "授权启动应用档案", exact: true })).toBeEnabled();
});

test("导入先预览再保存，删除须确认所见版本", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.profile = null; });
  await openProfiles(page);
  const file = { version: 1, name: "测试导入", savedAtMs: 1, settings: { pl1W: 50 } };
  await fileInput(page).setInputFiles({ name: "fixture.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(file)) });
  const preview = profilePanel(page);
  await expect(preview.getByText("待导入", { exact: true })).toBeVisible();
  await expect(preview).toContainText("50 W");
  expect(await mutations(page)).toEqual([]);
  await preview.getByRole("button", { name: "导入档案", exact: true }).click();
  await expect(preview.getByRole("button", { name: "应用档案", exact: true })).toBeVisible();
  expect(await mutations(page)).toEqual(["save_profile"]);
  await preview.getByRole("button", { name: "更多", exact: true }).click();
  await preview.getByRole("button", { name: "删除档案", exact: true }).click();
  const confirm = page.getByRole("dialog", { name: "删除档案", exact: true });
  await expect(confirm).toContainText("删除「测试导入」？");
  await confirm.getByRole("button", { name: "删除档案", exact: true }).click();
  await expect(page.getByRole("heading", { name: "还没有配置档案" })).toBeVisible();
  expect(await mutations(page)).toEqual(["save_profile", "delete_profile"]);
});

test("档案加载失败显示重试入口", async ({ page }) => {
  await page.evaluate(() => { window.__uiFixture.profileListFault = "隔离目录读取失败"; });
  await openProfiles(page);
  await expect(page.getByRole("alert")).toContainText("隔离目录读取失败");
  await expect(page.getByRole("heading", { name: "还没有配置档案" })).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.profileListFault = null; });
  await page.getByRole("button", { name: "重新读取档案" }).click();
  await expect(profilePanel(page).getByRole("heading", { name: "隔离档案" })).toBeVisible();
});
