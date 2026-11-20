import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace, openPowerArea } from "./workspace.mjs";
import { availableAction, pageActions, previewConfiguration, submitConfiguration } from "./configuration-review.mjs";

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
});

async function choose(page, label, option) {
  await page.getByRole("combobox", { name: label, exact: true }).click();
  await page.getByRole("option", { name: option, exact: true }).click();
}

test("充电调整即时提交并保留启动与供电规则的独立预览", async ({ page }) => {
  await page.goto("/");
  await openPowerArea(page, "电源设置");
  const nav = page.getByRole("navigation", { name: "主导航" });
  for (const name of ["电池与节能", "自动策略"])
    await expect(nav.getByRole("button", { name, exact: true })).toHaveCount(0);
  const main = page.getByRole("main", { name: "电源与档案", exact: true });
  await expect(main.getByRole("region", { name: "电池与充电", exact: true })).toBeVisible();
  const profileSettings = main.getByRole("region", { name: "档案应用", exact: true });
  for (const name of ["启动档案", "供电与唤醒档案"])
    await expect(profileSettings.getByRole("region", { name, exact: true })).toBeVisible();

  await main.getByRole("switch", { name: "限制充电范围", exact: true }).check();
  await choose(page, "开始充电阈值", "70%");
  await choose(page, "启动档案", "隔离档案");
  await expect(profileSettings.getByText("已保存 · 不应用档案", { exact: true })).toBeVisible();
  await choose(page, "插电时", "隔离档案");
  await openWorkspace(page, "设置");
  await openPowerArea(page, "电源设置");
  for (const scope of ["启动档案", "供电与唤醒档案"])
    await expect(availableAction(page, scope)).toBeVisible();

  const writes = () => page.evaluate(() => window.__uiFixture.calls
    .filter(call => ["apply_battery_charge", "set_startup_profile", "save_power_profile_policy"].includes(call.command))
    .map(call => call.command));
  await expect(pageActions(page)).toHaveCount(0);
  const chargeWrites = await writes();
  expect(chargeWrites.length).toBeGreaterThan(0);
  expect(chargeWrites.every(command => command === "apply_battery_charge")).toBe(true);
  const review = await previewConfiguration(page, "启动档案");
  await submitConfiguration(review);
  await expect(profileSettings.getByText("已保存 · 不应用档案", { exact: true })).toHaveCount(0);
  await expect(profileSettings.getByRole("combobox", { name: "启动档案", exact: true })).toHaveText("隔离档案");
  await expect(availableAction(page, "启动档案")).toHaveCount(0);
  expect(await writes()).toEqual([...chargeWrites, "set_startup_profile"]);
  await expect(main.getByRole("combobox", { name: "开始充电阈值", exact: true })).toHaveText("70%");
  await expect(availableAction(page, "供电与唤醒档案")).toBeVisible();
});

for (const savedPage of ["battery", "automation"]) {
  test(`旧的 ${savedPage} 最近访问记录打开合并后的电源页`, async ({ page }) => {
    await page.addInitScript(value => {
      localStorage.setItem("ccc-last-page", value);
      window.__uiFixture.preferences.startupPage = "lastVisited";
    }, savedPage);
    await page.goto("/");
    await expect(page.getByRole("main", { name: "电源与档案", exact: true })).toBeVisible();
    expect(await page.evaluate(() => localStorage.getItem("ccc-last-page"))).toBe("power");
  });
}
