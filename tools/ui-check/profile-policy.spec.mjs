import { availableAction, configurationView, previewConfiguration, submitConfiguration } from "./configuration-review.mjs";
import { openPowerArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "供电与唤醒档案", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
const alert = (page, text) => panel(page).getByRole("alert").filter({ hasText: text });
async function choose(page, label, value = "隔离档案") {
  await panel(page).getByRole("combobox", { name: label, exact: true }).click();
  await page.getByRole("option", { name: value, exact: true }).click();
}
async function save(page) {
  await submitConfiguration(await previewConfiguration(page, "供电与唤醒档案"));
}
async function enable(page) {
  await choose(page, "插电时");
  await save(page);
  await panel(page).getByRole("switch", { name: "启用自动档案", exact: true }).check();
  await expect(panel(page).getByRole("button", { name: "暂停自动应用", exact: true })).toBeEnabled();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => { window.__uiFixture.profile.settings.pl1W = 55; });
  await page.goto("/"); await openPowerArea(page, "电源设置");
  await expect(panel(page)).toContainText("保存一条规则后可开启自动档案");
});

test("规则只改草稿，应用绑定完整档案，启用不立即应用当前供电", async ({ page }) => {
  const p = panel(page);
  await expect(p.getByRole("switch", { name: "启用自动档案", exact: true })).not.toBeChecked();
  await expect(p.getByText(/^(已启用|未启用)$/)).toHaveCount(0);
  await choose(page, "插电时");
  const review = await previewConfiguration(page, "供电与唤醒档案");
  await review.getByRole("button", { name: "查看档案参数", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "查看「隔离档案」参数", exact: true })).toContainText("55 W");
  await page.keyboard.press("Escape");
  expect(await calls(page, "save_power_profile_policy")).toHaveLength(0);
  await submitConfiguration(review);
  await p.getByRole("switch", { name: "启用自动档案", exact: true }).check();
  await expect(p.getByRole("button", { name: "暂停自动应用", exact: true })).toBeEnabled();
  const request = (await calls(page, "save_power_profile_policy"))[0].args.request;
  expect(request.ac.file.settings.pl1W).toBe(55);
  expect(request.ac.mode).toBeNull();
  expect(request.battery).toBeNull(); expect(request.resume).toBeNull();
  expect(await calls(page, "apply_profile")).toHaveLength(0);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("插电、电池与唤醒三条规则分别选择并确认", async ({ page }) => {
  for (const label of ["插电时", "使用电池时", "唤醒后"]) await choose(page, label);
  await save(page);
  const request = (await calls(page, "save_power_profile_policy"))[0].args.request;
  for (const key of ["ac", "battery", "resume"]) expect(request[key].file.name).toBe("隔离档案");
});

test("暂停和停用保留已授权规则，继续直接核对保存绑定", async ({ page }) => {
  await enable(page);
  const p = panel(page);
  await p.getByRole("button", { name: "暂停自动应用", exact: true }).click();
  await expect(p.getByText("已暂停", { exact: true })).toBeVisible();
  expect(await page.evaluate(() => window.__uiFixture.powerPolicy.record.enabled)).toBe(true);
  await expect(p.getByRole("combobox", { name: "插电时", exact: true })).toHaveText("隔离档案");
  await p.getByRole("button", { name: "继续自动应用", exact: true }).click();
  expect(await calls(page, "save_power_profile_policy")).toHaveLength(1);
  await expect(p.getByRole("button", { name: "暂停自动应用", exact: true })).toBeEnabled();
  const master = p.getByRole("switch", { name: "启用自动档案", exact: true });
  await master.click();
  await expect(master).not.toBeChecked();
  expect((await calls(page, "set_power_profile_policy_enabled")).map(call => call.args.enabled)).toEqual([true, true, false]);
  expect(await calls(page, "apply_profile")).toHaveLength(0);
});

test("恢复初始规则单独预览确认并暂停，不恢复硬件现值", async ({ page }) => {
  await enable(page);
  const p = panel(page);
  const recovery = await previewConfiguration(page, "供电与唤醒档案", true);
  await expect(recovery).toContainText("恢复后暂停自动应用，设备现值保留。");
  expect(await calls(page, "restore_power_profile_policy")).toHaveLength(0);
  await configurationView(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(p.getByRole("switch", { name: "启用自动档案", exact: true })).not.toBeChecked();
  expect(await page.evaluate(() => window.__uiFixture.powerPolicy.paused)).toBe(true);
  await expect(p.getByRole("combobox", { name: "插电时", exact: true })).toHaveText("不应用档案");
  expect(await calls(page, "restore_power_profile_policy")).toHaveLength(1);
  expect(await calls(page, "apply_profile")).toHaveLength(0);
});

test("确认后文件变化被后端拒绝，重读后重新预览新内容", async ({ page }) => {
  await choose(page, "插电时");
  const p = panel(page);
  await previewConfiguration(page, "供电与唤醒档案");
  await page.evaluate(() => { window.__uiFixture.profile.settings.pl1W = 77; });
  await save(page);
  await expect(alert(page, "内容已变化")).toBeVisible();
  await p.getByRole("button", { name: "重新读取并暂停", exact: true }).click();
  await expect(p.getByRole("combobox", { name: "插电时", exact: true })).toHaveText("不应用档案");
  await enable(page);
  expect((await calls(page, "save_power_profile_policy")).at(-1).args.request.ac.file.settings.pl1W).toBe(77);
});

test("供电轮询保留正在编辑的草稿", async ({ page }) => {
  await choose(page, "使用电池时");
  const reads = (await calls(page, "read_power_profile_policy")).length;
  await page.evaluate(() => { window.__uiFixture.powerPolicy.source = "battery"; });
  await expect.poll(async () => (await calls(page, "read_power_profile_policy")).length).toBeGreaterThan(reads + 1);
  await expect(panel(page).getByRole("combobox", { name: "使用电池时", exact: true })).toHaveText("隔离档案");
  expect(await calls(page, "save_power_profile_policy")).toHaveLength(0);
});

test("旧的状态请求晚返回不能覆盖已经保存的规则", async ({ page }) => {
  await choose(page, "插电时");
  await page.evaluate(() => {
    const old = structuredClone(window.__uiFixture.powerPolicy), invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = (command, args) => {
      if (command === "read_power_profile_policy" && !window.__heldPowerPolicy) {
        window.__heldPowerPolicy = true;
        return new Promise(resolve => { window.__releasePowerPolicy = () => resolve(old); });
      }
      return invoke(command, args);
    };
  });
  await expect.poll(() => page.evaluate(() => Boolean(window.__releasePowerPolicy))).toBe(true);
  const review = await previewConfiguration(page, "供电与唤醒档案");
  await review.getByRole("button", { name: "保存自动档案规则", exact: true }).click();
  await expect.poll(async () => (await calls(page, "save_power_profile_policy")).length).toBe(1);
  await page.evaluate(() => window.__releasePowerPolicy());
  await expect(panel(page).getByRole("switch", { name: "启用自动档案", exact: true })).toBeEnabled();
  await expect(panel(page).getByRole("combobox", { name: "插电时", exact: true })).toHaveText("隔离档案");
});

test("通知离线时禁用启用操作，仍可读取和暂停软件规则", async ({ page }) => {
  await choose(page, "插电时");
  await save(page);
  await page.evaluate(() => { window.__uiFixture.powerPolicy.listenerError = "隔离通知线程退出"; window.__uiFixture.powerPolicy.route = null; });
  await expect(panel(page).getByRole("switch", { name: "启用自动档案", exact: true })).toBeDisabled();
  await expect(alert(page, "通知线程退出")).toBeVisible();
  await expect(panel(page).getByRole("button", { name: "重新读取并暂停", exact: true })).toBeEnabled();
});

test("外部策略变化需要重读，重读不自动接受其启用意图", async ({ page }) => {
  await enable(page);
  await expect(panel(page).getByRole("button", { name: "重新读取并暂停", exact: true })).toHaveCount(0);
  await page.evaluate(() => {
    window.__uiFixture.powerPolicyExternalRecord = structuredClone(window.__uiFixture.powerPolicy.record);
    window.__uiFixture.powerPolicy.lastIssue = "自动档案策略已被外部修改，请重新读取";
  });
  await expect(alert(page, "外部修改")).toBeVisible();
  await panel(page).getByRole("button", { name: "重新读取并暂停", exact: true }).click();
  await expect(panel(page).getByText("已暂停", { exact: true })).toBeVisible();
  await expect(panel(page).getByRole("switch", { name: "启用自动档案", exact: true })).toBeChecked();
  expect(await calls(page, "apply_profile")).toHaveLength(0);
});

test("共用档案卡和触发条件在宽窄窗口与深浅主题下保持边界", async ({ page }) => {
  const group = page.getByRole("region", { name: "档案应用", exact: true });
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    for (const width of [360, 640, 1000, 1440]) {
      await page.setViewportSize({ width, height: 960 });
      await group.scrollIntoViewIfNeeded();
      const geometry = await group.evaluate(el => ({
        width: el.clientWidth, scroll: el.scrollWidth, border: getComputedStyle(el).borderTopWidth,
      }));
      expect(geometry.scroll).toBeLessThanOrEqual(geometry.width + 1);
      expect(parseFloat(geometry.border)).toBeGreaterThan(0);
      const controls = group.getByRole("combobox");
      await expect(controls).toHaveCount(4);
      const positions = await controls.evaluateAll(items => items.map(item => {
        const { left, right, top } = item.getBoundingClientRect();
        return { left, right, top };
      }));
      for (const [index, position] of positions.slice(1).entries()) {
        expect(position.top).toBeGreaterThan(positions[index].top);
        expect(Math.abs(position.left - positions[0].left)).toBeLessThanOrEqual(1);
        expect(Math.abs(position.right - positions[0].right)).toBeLessThanOrEqual(1);
      }
      if (width === 1000) await group.screenshot({ path: `target/ui-check/power-profile-policy-${theme}.png` });
    }
  }
});
