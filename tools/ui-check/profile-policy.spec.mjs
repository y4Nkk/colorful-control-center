import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "供电与唤醒档案", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
async function choose(page, label, value = "隔离档案") {
  await panel(page).getByRole("combobox", { name: label, exact: true }).click();
  await page.getByRole("option", { name: value, exact: true }).click();
}
async function enable(page) {
  await choose(page, "插电时");
  await panel(page).getByRole("checkbox", { name: "启用自动档案", exact: true }).check();
  await panel(page).getByRole("button", { name: "保存并启用", exact: true }).click();
  await panel(page).getByRole("button", { name: "按以上条件自动应用档案？", exact: true }).click();
  await expect(panel(page).getByText("自动应用已启用", { exact: true })).toBeVisible();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => { window.__uiFixture.profile.settings.pl1W = 55; });
  await page.goto("/"); await openWorkspace(page, "自动策略");
  await expect(panel(page).getByText("已插电", { exact: true })).toBeVisible();
});

test("规则只改草稿，两步确认绑定完整档案，启用不立即应用当前供电", async ({ page }) => {
  const p = panel(page);
  await expect(p.getByText("自动应用未启用", { exact: true })).toBeVisible();
  await choose(page, "插电时");
  await p.getByText("查看「隔离档案」参数", { exact: true }).click();
  await expect(p).toContainText("55");
  await p.getByRole("checkbox", { name: "启用自动档案", exact: true }).check();
  await p.getByRole("button", { name: "保存并启用", exact: true }).click();
  expect(await calls(page, "save_power_profile_policy")).toHaveLength(0);
  await p.getByRole("button", { name: "按以上条件自动应用档案？", exact: true }).click();
  await expect(p.getByText("自动应用已启用", { exact: true })).toBeVisible();
  const request = (await calls(page, "save_power_profile_policy"))[0].args.request;
  expect(request.ac.file.settings.pl1W).toBe(55);
  expect(request.ac.mode).toBeNull();
  expect(request.battery).toBeNull(); expect(request.resume).toBeNull();
  expect(await calls(page, "apply_profile")).toHaveLength(0);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("插电、电池与唤醒三条规则分别选择并确认", async ({ page }) => {
  for (const label of ["插电时", "使用电池时", "唤醒后"]) await choose(page, label);
  await panel(page).getByRole("checkbox", { name: "启用自动档案", exact: true }).check();
  await panel(page).getByRole("button", { name: "保存并启用", exact: true }).click();
  await panel(page).getByRole("button", { name: "按以上条件自动应用档案？", exact: true }).click();
  await expect(panel(page).getByText("自动应用已启用", { exact: true })).toBeVisible();
  const request = (await calls(page, "save_power_profile_policy"))[0].args.request;
  for (const key of ["ac", "battery", "resume"]) expect(request[key].file.name).toBe("隔离档案");
});

test("暂停不删除已确认规则，重新启用仍需完整确认", async ({ page }) => {
  await enable(page);
  const p = panel(page);
  await p.getByRole("button", { name: "暂停自动应用", exact: true }).click();
  await expect(p.getByText("自动应用已暂停", { exact: true })).toBeVisible();
  expect(await page.evaluate(() => window.__uiFixture.powerPolicy.record.enabled)).toBe(true);
  await expect(p.getByRole("combobox", { name: "插电时", exact: true })).toHaveText("隔离档案");
  await p.getByRole("button", { name: "保存并启用", exact: true }).click();
  expect(await calls(page, "save_power_profile_policy")).toHaveLength(1);
  await p.getByRole("button", { name: "按以上条件自动应用档案？", exact: true }).click();
  await expect(p.getByText("自动应用已启用", { exact: true })).toBeVisible();
  expect(await calls(page, "apply_profile")).toHaveLength(0);
});

test("恢复初始规则单独预览确认并暂停，不恢复硬件现值", async ({ page }) => {
  await enable(page);
  const p = panel(page);
  await p.getByText("查看初始规则", { exact: true }).click();
  await p.getByRole("button", { name: "恢复初始规则", exact: true }).click();
  expect(await calls(page, "restore_power_profile_policy")).toHaveLength(0);
  await p.getByRole("button", { name: "恢复初始规则并暂停？", exact: true }).click();
  await expect(p.getByText("自动应用已暂停", { exact: true })).toBeVisible();
  await expect(p.getByRole("combobox", { name: "插电时", exact: true })).toHaveText("不应用档案");
  expect(await calls(page, "restore_power_profile_policy")).toHaveLength(1);
  expect(await calls(page, "apply_profile")).toHaveLength(0);
});

test("确认后文件变化被后端拒绝，重读后重新预览新内容", async ({ page }) => {
  await choose(page, "插电时");
  const p = panel(page);
  await p.getByRole("checkbox", { name: "启用自动档案", exact: true }).check();
  await p.getByRole("button", { name: "保存并启用", exact: true }).click();
  await page.evaluate(() => { window.__uiFixture.profile.settings.pl1W = 77; });
  await p.getByRole("button", { name: "按以上条件自动应用档案？", exact: true }).click();
  await expect(p.getByRole("alert")).toContainText("内容已变化");
  await p.getByRole("button", { name: "重新读取并暂停", exact: true }).click();
  await expect(p.getByRole("combobox", { name: "插电时", exact: true })).toHaveText("不应用档案");
  await enable(page);
  expect((await calls(page, "save_power_profile_policy")).at(-1).args.request.ac.file.settings.pl1W).toBe(77);
});

test("供电轮询更新事实并保留正在编辑的草稿", async ({ page }) => {
  await choose(page, "使用电池时");
  await page.evaluate(() => { window.__uiFixture.powerPolicy.source = "battery"; });
  await expect(panel(page).getByText("电池供电", { exact: true })).toBeVisible();
  await expect(panel(page).getByRole("combobox", { name: "使用电池时", exact: true })).toHaveText("隔离档案");
  expect(await calls(page, "save_power_profile_policy")).toHaveLength(0);
});

test("旧的状态请求晚返回不能覆盖已经保存的规则", async ({ page }) => {
  await choose(page, "插电时");
  await panel(page).getByRole("checkbox", { name: "启用自动档案", exact: true }).check();
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
  await panel(page).getByRole("button", { name: "保存并启用", exact: true }).click();
  await panel(page).getByRole("button", { name: "按以上条件自动应用档案？", exact: true }).click();
  await expect.poll(async () => (await calls(page, "save_power_profile_policy")).length).toBe(1);
  await page.evaluate(() => window.__releasePowerPolicy());
  await expect(panel(page).getByText("自动应用已启用", { exact: true })).toBeVisible();
  await expect(panel(page).getByRole("combobox", { name: "插电时", exact: true })).toHaveText("隔离档案");
});

test("最近自动结果随全局应用更新，后续手动报告不抹掉它", async ({ page }) => {
  await page.evaluate(() => {
    const report = { id: 900, revision: 1, origin: "battery", name: "电池配置", targetsApplied: true, pending: null,
      steps: [{ step: "功耗与电压已保存", ok: true, message: "已确认" }] };
    window.__uiFixture.profileApplication = report;
    window.__uiFixture.lastAutomaticProfileApplication = structuredClone(report);
  });
  const latest = panel(page).getByRole("region", { name: "最近自动应用", exact: true });
  await expect(latest).toContainText("电池配置");
  await expect(latest).toContainText("电池 · 已应用");
  await page.evaluate(() => { window.__uiFixture.profileApplication = { id: 901, revision: 1, origin: "manual", name: "手动档案", targetsApplied: true, pending: null, steps: [] }; });
  await expect(latest).toContainText("电池配置");
  await expect(latest).not.toContainText("手动档案");
});

test("通知离线时禁用启用操作，仍可读取和暂停软件规则", async ({ page }) => {
  await choose(page, "插电时");
  await panel(page).getByRole("checkbox", { name: "启用自动档案", exact: true }).check();
  await page.evaluate(() => { window.__uiFixture.powerPolicy.listenerError = "隔离通知线程退出"; window.__uiFixture.powerPolicy.route = null; });
  await expect(panel(page).getByRole("alert")).toContainText("通知线程退出");
  await expect(panel(page).getByRole("button", { name: "保存并启用", exact: true })).toBeDisabled();
});

test("外部策略变化需要重读，重读不自动接受其启用意图", async ({ page }) => {
  await enable(page);
  await page.evaluate(() => {
    window.__uiFixture.powerPolicyExternalRecord = structuredClone(window.__uiFixture.powerPolicy.record);
    window.__uiFixture.powerPolicy.lastIssue = "自动档案策略已被外部修改，请重新读取";
  });
  await expect(panel(page).getByRole("alert")).toContainText("外部修改");
  await panel(page).getByRole("button", { name: "重新读取并暂停", exact: true }).click();
  await expect(panel(page).getByText("自动应用已暂停", { exact: true })).toBeVisible();
  await expect(panel(page).getByRole("checkbox", { name: "启用自动档案", exact: true })).toBeChecked();
  expect(await calls(page, "apply_profile")).toHaveLength(0);
});

test("模块边框、内部左右布局与窄屏纵排在深浅主题下保持有效", async ({ page }) => {
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    for (const width of [360, 640, 1000, 1440]) {
      await page.setViewportSize({ width, height: 960 });
      await panel(page).scrollIntoViewIfNeeded();
      const geometry = await panel(page).evaluate(el => {
        const columns = el.querySelector(".ccc-power-policy-columns"), left = el.querySelector(".ccc-power-policy-current"), right = el.querySelector(".ccc-power-policy-editor");
        return { width: el.clientWidth, scroll: el.scrollWidth, border: getComputedStyle(el).borderTopWidth,
          gap: parseFloat(getComputedStyle(columns).gap), left: left.getBoundingClientRect().toJSON(), right: right.getBoundingClientRect().toJSON() };
      });
      expect(geometry.scroll).toBeLessThanOrEqual(geometry.width + 1);
      expect(parseFloat(geometry.border)).toBeGreaterThan(0); expect(geometry.gap).toBeGreaterThan(0);
      if (width <= 640) expect(geometry.right.top).toBeGreaterThan(geometry.left.top);
      if (width === 1440) expect(geometry.right.left).toBeGreaterThan(geometry.left.right);
      if (width === 1000) await panel(page).screenshot({ path: `target/ui-check/power-profile-policy-${theme}.png` });
    }
  }
});
