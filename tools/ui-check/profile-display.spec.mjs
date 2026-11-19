import { changeButton, configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openProfileResults, openProfileSave, openStartupParameters, profilePanel as panel, saveProfileButton, showProfileMenu, showProfileSettings } from "./profile-review.mjs";
import { openWorkspace, openSystemArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const host = page => page.getByRole("region", { name: "档案显示确认", exact: true });
async function select(page) {
  await openSystemArea(page, "配置档案");
  await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
  await showProfileSettings(page);
}
async function begin(page) {
  await previewConfiguration(page);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(host(page)).toBeVisible();
}
async function frequencies(page) {
  return page.evaluate(() => window.__uiFixture.display.displays.map(display => display.current.frequency));
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    const data = window.__uiFixture;
    const first = structuredClone(data.display.displays[0]);
    first.current.name = "内置屏";
    const second = structuredClone(first);
    second.current.device = "DISPLAY2"; second.current.identity = "isolated-panel-2";
    second.current.name = "外接屏"; second.current.x = 1920;
    data.display.displays = [first, second];
    data.profile.settings.display = { displays: [first, second].map(display => ({ ...display.current, frequency: 144 })) };
    data.profile.settings.pl1W = 77;
  });
  await page.goto("/"); await openWorkspace(page, "设备配置");
});

test("双屏预览左右呈现当前与目标，初次确认只进入显示试用", async ({ page }) => {
  await select(page);
  await expect(panel(page).getByLabel("档案显示配置")).toHaveCount(1);
  const review = await previewConfiguration(page);
  const preview = panel(page).getByLabel("档案显示配置");
  await expect(preview).toContainText("切换前 60 Hz → 目标");
  await expect(preview).toContainText("内置屏"); await expect(preview).toContainText("外接屏");
  expect(await frequencies(page)).toEqual([60, 60]);
  await review.getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(host(page)).toContainText("再继续应用其余档案设置");
  expect(await frequencies(page)).toEqual([144, 144]);
  expect(await page.evaluate(() => window.__uiFixture.profileApplication.steps)).toEqual([]);
  await expect(panel(page).getByRole("button", { name: /^执行明细/ })).toHaveCount(0);
});

test("整组保留才继续档案并展示真正完成的步骤", async ({ page }) => {
  await select(page); await begin(page);
  await host(page).getByRole("button", { name: "保留显示并继续档案", exact: true }).click();
  await expect(host(page)).toHaveCount(0);
  await expect((await openProfileResults(page)).getByRole("listitem").filter({ hasText: "显示刷新率复核" }).getByRole("img", { name: "成功", exact: true })).toBeVisible();
  expect(await frequencies(page)).toEqual([144, 144]);
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "keep_profile_display").length)).toBe(1);
});

test("取消显示试用恢复两屏，未执行其余档案设置", async ({ page }) => {
  await select(page); await begin(page);
  await host(page).getByRole("button", { name: "恢复本次显示并停止", exact: true }).click();
  await expect(host(page)).toHaveCount(0);
  await expect(await openProfileResults(page)).toContainText("取消档案应用");
  expect(await frequencies(page)).toEqual([60, 60]);
  expect(await page.evaluate(() => window.__uiFixture.calls.some(call => call.command === "keep_profile_display"))).toBe(false);
});

test("超时自动恢复，两屏试用不能被报告为已应用", async ({ page }) => {
  await select(page); await begin(page);
  await page.evaluate(() => window.__uiFixture.profileTrialDeadline = Date.now() - 1);
  await expect(host(page)).toHaveCount(0);
  await expect(await openProfileResults(page)).toContainText("未执行其余档案设置");
  expect(await frequencies(page)).toEqual([60, 60]);
});

test("试用期间文件变化会恢复显示并拒绝继续执行旧档案", async ({ page }) => {
  await select(page); await begin(page);
  await page.evaluate(() => window.__uiFixture.profile.settings.pl1W = 88);
  await host(page).getByRole("button", { name: "保留显示并继续档案", exact: true }).click();
  await expect(host(page)).toHaveCount(0);
  await expect(await openProfileResults(page)).toContainText("档案已变化");
  expect(await frequencies(page)).toEqual([60, 60]);
});

test("后续设置失败恢复本次显示，保留具体失败步骤", async ({ page }) => {
  await select(page); await begin(page);
  await page.evaluate(() => window.__uiFixture.profileSteps = [{ step: "GPU 偏移", ok: false, message: "隔离驱动拒绝" }]);
  await host(page).getByRole("button", { name: "保留显示并继续档案", exact: true }).click();
  await expect(host(page)).toHaveCount(0);
  const results = await openProfileResults(page);
  await expect(results).toContainText("隔离驱动拒绝");
  await expect(results).toContainText("恢复本次显示变更");
  expect(await frequencies(page)).toEqual([60, 60]);
});

test("显示器身份变化保留离线目标且禁止手动应用", async ({ page }) => {
  await page.evaluate(() => window.__uiFixture.display.displays[1].current.identity = "replacement-panel");
  await select(page);
  await expect(changeButton(page)).toHaveCount(0);
  await expect((await showProfileMenu(page)).getByRole("button", { name: "导出档案", exact: true })).toBeEnabled();
  await expect(panel(page).getByLabel("档案显示配置")).toContainText("当前未读取");
});

test("没有刷新率变化时直接执行，无需显示试用确认", async ({ page }) => {
  await page.evaluate(() => { for (const mode of window.__uiFixture.profile.settings.display.displays) mode.frequency = 60; });
  await select(page);
  await previewConfiguration(page);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect((await openProfileResults(page)).getByRole("listitem").filter({ hasText: "显示刷新率复核" }).getByRole("img", { name: "成功", exact: true })).toBeVisible();
  await expect(host(page)).toHaveCount(0);
  expect(await frequencies(page)).toEqual([60, 60]);
});

test("切换页面保留唯一全局确认，显示页不能单独保留档案屏幕", async ({ page }) => {
  await select(page); await begin(page);
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: "设备配置", exact: true }).click();
  await expect(host(page)).toHaveCount(1);
  const display = page.getByRole("region", { name: "屏幕显示", exact: true });
  await expect(display.getByRole("button", { name: "保留刷新率", exact: true })).toHaveCount(0);
  await expect(host(page)).toContainText(/显示试用 · \d+ 秒/);
  await expect(display.getByRole("status")).toHaveText("请在「档案显示确认」中保留或恢复本次试用");
  await host(page).getByRole("button", { name: "恢复本次显示并停止", exact: true }).click();
  await expect(host(page)).toHaveCount(0);
});

test("捕获与保存包含完整双屏身份和频率", async ({ page }) => {
  await select(page);
  await openProfileSave(page, "显示捕获", ["显示与显卡"]);
  await saveProfileButton(page).click();
  await expect(panel(page).getByRole("heading", { name: "显示捕获", exact: true })).toBeVisible();
  const target = await page.evaluate(() => window.__uiFixture.calls.find(call => call.command === "save_profile").args.args.settings.display);
  expect(target.displays).toHaveLength(2);
  expect(target.displays.map(mode => mode.frequency)).toEqual([60, 60]);
  expect(target.displays[1].identity).toBe("isolated-panel-2");
});

test("启动档案显示共享预览和必须保留的说明，保存不立即切换", async ({ page }) => {
  await openWorkspace(page, "电源");
  const startup = page.getByRole("region", { name: "启动档案", exact: true });
  await startup.getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  const parameters = await openStartupParameters(page);
  await expect(parameters.getByLabel("档案显示配置")).toContainText("开机同样确认");
  await page.keyboard.press("Escape");
  await configurationView(page).getByRole("button", { name: "授权启动应用档案", exact: true }).click();
  const selection = await page.evaluate(() => window.__uiFixture.calls.find(call => call.command === "set_startup_profile").args.expected);
  expect(selection.settings.display.displays).toHaveLength(2);
  expect(await frequencies(page)).toEqual([60, 60]);
});

test("双屏确认卡深浅宽窄均有模块边界，内容不溢出", async ({ page }) => {
  await select(page); await begin(page);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 1440]) {
      await page.setViewportSize({ width, height: 1800 });
      await host(page).scrollIntoViewIfNeeded();
      await expect(host(page)).toBeVisible();
      expect(await host(page).evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true);
      expect(await host(page).locator('[data-surface="inset"]').first().evaluate(element => parseFloat(getComputedStyle(element).borderWidth))).toBeGreaterThan(0);
      await page.screenshot({ path: `target/ui-check/profile-display-${theme}-${width}.png` });
    }
  }
});

test("保留请求执行期间显示处理状态，阻止重复确认", async ({ page }) => {
  await select(page); await begin(page);
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === "keep_profile_display") await new Promise(resolve => { window.__uiFixture.releaseKeep = resolve; });
      return invoke(command, args);
    };
  });
  await host(page).getByRole("button", { name: "保留显示并继续档案", exact: true }).click();
  await expect(host(page)).toContainText("正在处理档案");
  await expect(host(page).getByRole("button", { name: "保留显示并继续档案", exact: true })).toBeDisabled();
  await expect(host(page).getByRole("button", { name: "恢复本次显示并停止", exact: true })).toBeDisabled();
  await page.evaluate(() => window.__uiFixture.releaseKeep());
  await expect(host(page)).toHaveCount(0);
  expect(await frequencies(page)).toEqual([144, 144]);
});

test("应用前未完成的旧状态读取不能清除新的整组确认", async ({ page }) => {
  await select(page);
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    let hold = true;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === "read_profile_application" && hold) {
        hold = false;
        await new Promise(resolve => { window.__uiFixture.releaseOldRead = resolve; });
        window.__uiFixture.oldReadCompleted = true;
        return null;
      }
      return invoke(command, args);
    };
  });
  await expect.poll(() => page.evaluate(() => !!window.__uiFixture.releaseOldRead)).toBe(true);
  await begin(page);
  await page.evaluate(() => window.__uiFixture.releaseOldRead());
  await expect.poll(() => page.evaluate(() => !!window.__uiFixture.oldReadCompleted)).toBe(true);
  await expect(host(page)).toBeVisible();
  await expect(host(page)).toContainText("内置屏");
  await host(page).getByRole("button", { name: "恢复本次显示并停止", exact: true }).click();
  await expect(host(page)).toHaveCount(0);
});

test("启动档案在显示保留后更新当前执行结果，无需切页重读", async ({ page }) => {
  await select(page); await begin(page);
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.profileApplication.origin = "startup";
    data.startupLastRun = "隔离档案 等待显示确认";
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      const result = await invoke(command, args);
      if (command === "keep_profile_display") data.startupLastRun = "已完成 隔离档案 的全部步骤";
      return result;
    };
  });
  await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: "设备配置", exact: true }).click();
  await openWorkspace(page, "电源");
  const startup = page.getByRole("region", { name: "启动档案", exact: true });
  await expect(startup.getByRole("status")).toContainText("等待显示确认");
  await host(page).getByRole("button", { name: "保留显示并继续档案", exact: true }).click();
  await expect(host(page)).toHaveCount(0);
  await expect(startup.getByRole("status")).toContainText("已完成 隔离档案 的全部步骤");
});

test("后台自动恢复失败显示具体错误并禁止继续，仍可重试恢复", async ({ page }) => {
  await select(page); await begin(page);
  await page.evaluate(() => {
    const report = window.__uiFixture.profileApplication;
    report.pending.error = "隔离显示恢复失败";
    report.revision += 1;
  });
  await expect(host(page).getByRole("alert")).toContainText("隔离显示恢复失败");
  await expect(host(page)).toContainText("显示恢复待处理");
  await expect(host(page).getByRole("button", { name: "保留显示并继续档案", exact: true })).toBeDisabled();
  await host(page).getByRole("button", { name: "恢复本次显示并停止", exact: true }).click();
  await expect(host(page)).toHaveCount(0);
  expect(await frequencies(page)).toEqual([60, 60]);
});

test("保留后仅收尾失败时保持整组目标，明确报告设置已应用", async ({ page }) => {
  await select(page); await begin(page);
  await page.evaluate(() => {
    const data = window.__uiFixture;
    data.profileTargetsApplied = true;
    data.profileSteps = [{ step: "网络驱动策略", ok: true, message: "已确认" }, { step: "网络授权收尾", ok: false, message: "隔离收尾失败" }];
  });
  await host(page).getByRole("button", { name: "保留显示并继续档案", exact: true }).click();
  await expect(host(page)).toHaveCount(0);
  await expect(page.getByText("目标设置已应用，部分收尾待处理", { exact: true })).toBeVisible();
  const results = await openProfileResults(page);
  await expect(results).toContainText("隔离收尾失败");
  await expect(results).not.toContainText("恢复本次显示变更");
  expect(await frequencies(page)).toEqual([144, 144]);
});

test("无需显示试用的档案也区分目标确认和收尾错误", async ({ page }) => {
  await page.evaluate(() => {
    const data = window.__uiFixture;
    for (const mode of data.profile.settings.display.displays) mode.frequency = 60;
    data.profileTargetsApplied = true;
    data.profileSteps = [{ step: "目标设置", ok: true, message: "已确认" }, { step: "网络授权收尾", ok: false, message: "隔离收尾失败" }];
  });
  await select(page);
  await previewConfiguration(page);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(host(page)).toHaveCount(0);
  await expect(page.getByRole("alert")).toContainText("目标设置已应用");
  await expect(await openProfileResults(page)).toContainText("隔离收尾失败");
  expect(await frequencies(page)).toEqual([60, 60]);
});

test("保存新的预览保留唯一最近执行结果，并显示实际执行的档案名", async ({ page }) => {
  await page.evaluate(() => { for (const mode of window.__uiFixture.profile.settings.display.displays) mode.frequency = 60; });
  await select(page);
  await previewConfiguration(page);
  await configurationView(page).getByRole("button", { name: "应用档案", exact: true }).click();
  await expect(panel(page).getByRole("region", { name: "最近应用记录", exact: true })).toContainText("上次应用 · 隔离档案");
  await openProfileSave(page, "新预览档案", ["显示与显卡"]);
  await saveProfileButton(page).click();
  await expect(panel(page).getByRole("heading", { name: "新预览档案", exact: true })).toBeVisible();
  await expect(panel(page).getByRole("region", { name: "最近应用记录", exact: true })).toContainText("上次应用 · 隔离档案");
  await expect(panel(page).getByRole("button", { name: /^执行明细/ })).toHaveCount(1);
  const notices = page.getByRole("button", { name: "关闭通知", exact: true });
  while (await notices.count()) {
    const count = await notices.count();
    await notices.first().click();
    await expect(notices).toHaveCount(count - 1);
  }
  const record = panel(page).getByRole("region", { name: "最近应用记录", exact: true });
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    for (const width of [360, 1440]) {
      await page.setViewportSize({ width, height: 1800 });
      await record.scrollIntoViewIfNeeded();
      expect(await record.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true);
      await page.screenshot({ path: `target/ui-check/profile-display-result-${theme}-${width}.png` });
    }
  }
});
