import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "档案预览", exact: true });
const charge = page => panel(page).getByRole("region", { name: "档案充电保护", exact: true });
const energy = page => panel(page).getByRole("region", { name: "档案电池节能", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|save_|import_|set_startup|start_battery)/.test(call.command)));
async function select(page) {
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: "性能档案", exact: true }).click();
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
}
async function capture(page, name) {
  await page.getByPlaceholder("游戏 / 静音").fill(name);
  await page.getByRole("button", { name: "保存当前", exact: true }).click();
  await page.getByRole("button", { name: "保存当前设置？", exact: true }).click();
}
async function exported(page) {
  const download = page.waitForEvent("download"); await panel(page).getByRole("button", { name: "导出", exact: true }).click();
  return JSON.parse(await readFile(await (await download).path(), "utf8"));
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const data = window.__uiFixture;
    data.profile.settings.batteryCharge = { values: { enabled: true, start: 70, stop: 90 },
      startValues: data.battery.charge.startValues, stopValues: data.battery.charge.stopValues };
    const policy = structuredClone(data.energy.saved);
    policy.everyDay = false; policy.weekdays = 62; policy.slots = [{ start: 8, end: 18 }, { start: 20, end: 22 }];
    data.profile.settings.batteryEnergy = { policy, capability: data.energy.capability };
  });
  await page.goto("/"); await openWorkspace(page, "电池与节能");
});

test("电池档案以真实阈值环和时段预览，确认提交完整目标", async ({ page }) => {
  await select(page);
  await expect(charge(page).getByRole("meter", { name: "开始充电阈值", exact: true })).toHaveAttribute("aria-valuenow", "70");
  await expect(charge(page).getByRole("meter", { name: "停止充电阈值", exact: true })).toHaveAttribute("aria-valuenow", "90");
  await expect(charge(page)).toContainText("先停止节能计划");
  await expect(energy(page)).toContainText("周一、周二、周三、周四、周五");
  await expect(energy(page).getByRole("img", { name: "档案节能时段：08:00到18:00，20:00到22:00", exact: true })).toBeVisible();
  await expect(energy(page)).toContainText("计划关闭");
  await expect(energy(page).locator("input,button")).toHaveCount(0); expect(await writes(page)).toEqual([]);
  await panel(page).getByRole("button", { name: "应用", exact: true }).click(); expect(await writes(page)).toEqual([]);
  await panel(page).getByRole("button", { name: "应用「隔离档案」？", exact: true }).click();
  const calls = await writes(page); expect(calls).toHaveLength(1);
  expect(calls[0].args.expected.settings.batteryCharge.values).toEqual({ enabled: true, start: 70, stop: 90 });
  expect(calls[0].args.expected.settings.batteryEnergy.policy.slots).toEqual([{ start: 8, end: 18 }, { start: 20, end: 22 }]);
  await expect(panel(page).getByLabel("执行结果")).toContainText("电池配置");
});

test("捕获关闭原始零阈值并导出候选，不填充默认阈值", async ({ page }) => {
  await select(page);
  await page.evaluate(() => { window.__uiFixture.battery.charge.current = { enabled: false, start: 0, stop: 0 }; });
  await capture(page, "关闭阈值");
  await expect(charge(page)).toContainText("关闭阈值控制"); await expect(charge(page).getByRole("meter")).toHaveCount(0);
  const file = await exported(page);
  expect(file.settings.batteryCharge.values).toEqual({ enabled: false, start: 0, stop: 0 });
  expect(file.settings.batteryCharge.startValues).toContain(95);
  expect(file.settings.batteryEnergy.policy.enabled).toBe(false);
  expect((await writes(page)).map(call => call.command)).toEqual(["save_profile"]);
});

test("运行节能时捕获计划且排除独立充电目标", async ({ page }) => {
  await select(page);
  await page.evaluate(() => {
    window.__uiFixture.energy.saved.enabled = true;
    window.__uiFixture.battery.charge.current = { enabled: true, start: 95, stop: 100 };
  });
  await capture(page, "节能管理");
  await expect(charge(page)).toHaveCount(0); await expect(energy(page)).toContainText("启用节能时段");
  const file = await exported(page);
  expect(file.settings.batteryCharge).toBeNull(); expect(file.settings.batteryEnergy.policy.enabled).toBe(true);
  expect((await writes(page)).map(call => call.command)).toEqual(["save_profile"]);
});

test("导入无手动阈值的节能档案，展示完整配置且不触发校准", async ({ page }) => {
  await select(page);
  const file = await page.evaluate(() => {
    const file = structuredClone(window.__uiFixture.profile); file.name = "节能导入";
    file.settings.batteryCharge = null; file.settings.batteryEnergy.policy.enabled = true; return file;
  });
  await page.getByLabel("导入性能档案", { exact: true }).setInputFiles({ name: "节能.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(file)) });
  await expect(energy(page)).toContainText("计划管理充电阈值"); await expect(charge(page)).toHaveCount(0);
  await expect(energy(page)).toContainText("20:00–22:00"); expect(await writes(page)).toEqual([]);
});

test("启动档案共享电池预览，保存选择不发送充电或节能命令", async ({ page }) => {
  await openWorkspace(page, "自动策略");
  const startup = page.getByRole("region", { name: "启动档案", exact: true });
  await startup.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  await startup.getByText("查看启动档案参数", { exact: true }).click();
  await expect(startup.getByLabel("档案充电保护")).toContainText("启用阈值控制");
  await expect(startup.getByLabel("档案电池节能")).toContainText("停止节能时段");
  await startup.getByRole("button", { name: "保存", exact: true }).click();
  await startup.getByRole("button", { name: "启动时应用「隔离档案」？", exact: true }).click();
  const calls = await writes(page); expect(calls.map(call => call.command)).toEqual(["set_startup_profile"]);
  expect(calls[0].args.expected.settings.batteryCharge.values.stop).toBe(90);
});

test("旧电池目标确认失效，恢复失败记录保持可见", async ({ page }) => {
  await select(page); await panel(page).getByRole("button", { name: "应用", exact: true }).click();
  await page.evaluate(() => { window.__uiFixture.profile.settings.batteryCharge.values.start = 60; });
  await panel(page).getByRole("button", { name: "应用「隔离档案」？", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("档案已变化");
  await page.evaluate(() => { window.__uiFixture.profileSteps = [{ step: "电池配置", ok: false, message: "节能记录已变化" },
    { step: "恢复电池配置", ok: false, message: "旧操作不再拥有写入权，记录保留" }]; });
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await panel(page).getByRole("button", { name: "应用", exact: true }).click();
  await panel(page).getByRole("button", { name: "应用「隔离档案」？", exact: true }).click();
  await expect(panel(page).getByLabel("执行结果")).toContainText("旧操作不再拥有写入权，记录保留");
});

test("校准和待恢复节能记录阻止捕获，不创建替代档案", async ({ page }) => {
  await select(page); await page.evaluate(() => { window.__uiFixture.calibration.recoveryRequired = true; });
  await capture(page, "待恢复电池"); await expect(page.getByRole("alert")).toContainText("电池校准正在接管");
  await page.evaluate(() => { window.__uiFixture.calibration.recoveryRequired = false; window.__uiFixture.energy.recoveryRequired = true; });
  await capture(page, "待恢复节能"); await expect(page.getByRole("alert")).toContainText("节能操作尚需恢复");
  expect(await writes(page)).toEqual([]);
});

test("电池档案宽窄深浅卡片布局和真实时间条不溢出", async ({ page }) => {
  await select(page);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    for (const width of [1440, 360]) {
      await page.setViewportSize({ width, height: 1800 }); await energy(page).scrollIntoViewIfNeeded();
      expect(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)).toBe(false);
      const columns = await energy(page).evaluate(element => getComputedStyle(element).gridTemplateColumns.split(" ").length);
      expect(columns).toBe(width === 1440 ? 2 : 1);
      await energy(page).screenshot({ path: `target/ui-check/profile-energy-${theme}-${width}.png` });
      await charge(page).scrollIntoViewIfNeeded(); await charge(page).screenshot({ path: `target/ui-check/profile-charge-${theme}-${width}.png` });
    }
  }
});
