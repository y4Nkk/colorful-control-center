import { availableAction, configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openWorkspace, openPowerArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { supplyModeFixture, supplyRecoveryFixture } from "./supply-mode-fixture.mjs";

const card = page => page.getByRole("region", { name: "原厂供电模式", exact: true });
const alert = (page, text) => card(page).getByRole("alert").filter({ hasText: text });
const toggle = page => card(page).getByRole("switch", { name: "启用原厂自动切换", exact: true });
const reread = page => card(page).getByRole("button", { name: "重新读取供电模式", exact: true }).click();
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
async function setup(page, view = supplyModeFixture()) {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(value => { window.__uiFixture.supplyMode = value; window.__uiFixture.supplyModeRevision = 0; }, view);
  await page.goto("/"); await openPowerArea(page, "电源设置");
  await expect(card(page).getByText(/^当前模式 · /)).toBeVisible();
}
async function enable(page) {
  await toggle(page).click();
  await expect(toggle(page)).toBeChecked();
}
async function heldRead(page) {
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__supplyReading = 0; window.__supplyMaxReading = 0;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command !== "read_oem_supply_mode") return invoke(command, args);
      window.__supplyMaxReading = Math.max(window.__supplyMaxReading, ++window.__supplyReading);
      try {
        const value = await invoke(command, args);
        if (!window.__heldSupplyRead) {
          window.__heldSupplyRead = true;
          return await new Promise(resolve => { window.__releaseSupplyRead = () => resolve(value); });
        }
        return value;
      } finally { --window.__supplyReading; }
    };
  });
  await expect.poll(() => page.evaluate(() => Boolean(window.__releaseSupplyRead))).toBe(true);
}

test("唤醒跳过读数与初始恢复目标分开，仍能明确暂停监听", async ({ page }) => {
  const view = supplyModeFixture();
  view.enabled = true; view.active = true; view.canEnable = false; view.canRestore = true;
  view.recovery = supplyRecoveryFixture();
  view.recovery.currentGpuClocks = { status: "skipped", value: "resume" };
  view.error = "部分GPU条件暂不可用，唤醒按原厂规则跳过调频";
  await setup(page, view);
  await expect(toggle(page)).toBeChecked();
  await expect(toggle(page)).toBeEnabled();
  const recovery = await previewConfiguration(page, "原厂供电模式", true);
  const current = recovery.getByRole("heading", { name: "当前观察", exact: true }).locator("..");
  await expect(current).toContainText("唤醒时按原厂规则跳过频率调校");
  await expect(current).toContainText("不沿用此前读数");
  await expect(current).not.toContainText("MHz");
  const initial = recovery.getByRole("heading", { name: "初始目标", exact: true }).locator("..");
  await expect(initial).toContainText("987.654 MHz");
  expect(await calls(page, "restore_oem_supply_mode")).toHaveLength(0);
});

test("GPU恢复读失败时在卡内说明原因，不提供恢复确认", async ({ page }) => {
  const view = supplyModeFixture();
  view.enabled = true; view.active = true; view.canEnable = false; view.canRestore = false;
  view.recovery = supplyRecoveryFixture();
  view.recovery.currentGpuClocks = { status: "unavailable", value: "隔离驱动暂不可读" };
  view.recoveryError = "GPU频率恢复前态不可用：隔离驱动暂不可读";
  await setup(page, view);
  await expect(availableAction(page, "原厂供电模式", true)).toHaveCount(0);
  await expect(alert(page, "GPU频率恢复前态不可用：隔离驱动暂不可读")).toBeVisible();
  expect(await calls(page, "restore_oem_supply_mode")).toHaveLength(0);
});

test("切换开关直接发送", async ({ page }) => {
  await setup(page);
  await enable(page);
  await expect(toggle(page)).toBeChecked();
  expect((await calls(page, "enable_oem_supply_mode"))[0].args).toEqual({ expectedRevision: "supply-0" });
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("关闭暂停并保留已保存策略，重新打开直接继续，未执行时恢复仅撤销软件规则", async ({ page }) => {
  await setup(page); await enable(page);
  await toggle(page).click();
  await expect(card(page).getByText("已暂停", { exact: true })).toBeVisible();
  expect(await page.evaluate(() => window.__uiFixture.supplyMode.enabled)).toBe(true);
  await toggle(page).click();
  await expect(toggle(page)).toBeChecked();
  await expect(await previewConfiguration(page, "原厂供电模式", true)).toContainText("尚无设备设置需要恢复");
  expect(await calls(page, "restore_oem_supply_mode")).toEqual([]);
  await configurationView(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await expect(toggle(page)).not.toBeChecked();
  await expect(card(page).getByText(/^(已启用|未启用)$/)).toHaveCount(0);
});

test("供电方案重新读取后，一次应用使用最新修订", async ({ page }) => {
  await setup(page);
  await expect(card(page)).toContainText("当前模式 · 娱乐");
  await page.evaluate(() => {
    const { supplyMode } = window.__uiFixture;
    supplyMode.revision = "changed";
    supplyMode.review.currentMode = 0;
  });
  await reread(page);
  await expect(card(page)).toContainText("当前模式 · 安静");
  expect(await calls(page, "enable_oem_supply_mode")).toEqual([]);
  await enable(page);
  expect((await calls(page, "enable_oem_supply_mode"))[0].args.expectedRevision).toBe("changed");
});

test("后端拒绝失效切换，错误经过定时重读仍保留", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => { window.__uiFixture.supplyModeFault = "隔离来源已变化，未执行"; });
  await toggle(page).click();
  await expect(alert(page, "隔离来源已变化")).toBeVisible();
  const reads = (await calls(page, "read_oem_supply_mode")).length;
  await expect.poll(async () => (await calls(page, "read_oem_supply_mode")).length).toBeGreaterThan(reads);
  await expect(alert(page, "隔离来源已变化")).toBeVisible();
  await reread(page);
  await expect(alert(page, "隔离来源已变化")).toHaveCount(0);
});

test("读失败清除可执行快照，暂停仍能发送，恢复后重读当前事实", async ({ page }) => {
  await setup(page); await enable(page);
  await page.evaluate(() => { window.__uiFixture.supplyModeReadError = "隔离状态读取失败"; });
  await expect(alert(page, "隔离状态读取失败")).toBeVisible();
  await expect(toggle(page)).toBeDisabled();
  await card(page).getByRole("button", { name: "暂停自动切换", exact: true }).click();
  expect(await calls(page, "pause_oem_supply_mode")).toHaveLength(1);
  await page.evaluate(() => { window.__uiFixture.supplyModeReadError = null; });
  await expect(card(page).getByText("已暂停", { exact: true })).toBeVisible();
});

test("暂停不等待读取，旧读不覆盖新状态且始终只有一个在途读请求", async ({ page }) => {
  await setup(page); await enable(page); await heldRead(page);
  await toggle(page).click();
  await expect.poll(async () => (await calls(page, "pause_oem_supply_mode")).length).toBe(1);
  expect(await page.evaluate(() => window.__uiFixture.supplyMode.active)).toBe(false);
  expect(await page.evaluate(() => window.__supplyReading)).toBe(1);
  await page.evaluate(() => window.__releaseSupplyRead());
  await expect(card(page).getByText("已暂停", { exact: true })).toBeVisible();
  expect(await page.evaluate(() => window.__supplyMaxReading)).toBe(1);
});

test("启用正在等待授权时也可暂停，不重复发送启用", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = (command, args) => {
      if (command === "enable_oem_supply_mode") {
        window.__uiFixture.calls.push({ command, args });
        return new Promise((_, reject) => { window.__rejectSupplyEnable = () => reject(new Error("隔离许可已撤销")); });
      }
      return invoke(command, args);
    };
  });
  await toggle(page).click();
  await card(page).getByRole("button", { name: "暂停自动切换", exact: true }).click();
  await expect(card(page).getByText("已暂停后续操作，等待当前操作结束。", { exact: true })).toBeVisible();
  expect(await calls(page, "enable_oem_supply_mode")).toHaveLength(1);
  expect(await calls(page, "pause_oem_supply_mode")).toHaveLength(1);
  await page.evaluate(() => window.__rejectSupplyEnable());
  await expect(alert(page, "许可已撤销")).toBeVisible();
});

test("恢复完整预览展示稀疏风扇模式、计划、独显与原始精度频率", async ({ page }) => {
  const view = supplyModeFixture();
  view.enabled = true; view.active = true; view.canEnable = false; view.canRestore = true; view.recovery = supplyRecoveryFixture();
  await setup(page, view);
  const review = await previewConfiguration(page, "原厂供电模式", true);
  const recovery = review.getByLabel("原厂模式初始恢复预览", { exact: true });
  for (const text of ["性能 → 娱乐", "自动 → 寂静模式", "5% → 0%", "高性能计划", "平衡计划", "节能计划", "固件节能值 0 → 固件节能值 2", "自动关断允许", "开启独显后取得", "-12.5 MHz", "987.654 MHz"]) {
    await expect(recovery).toContainText(text);
  }
  await expect(recovery).toContainText("当前频率未知，未补用保存值");
  await recovery.getByRole("button", { name: "查看完整风扇恢复参数", exact: true }).click();
  const raw = recovery.locator("pre").last();
  await expect(raw).toContainText('"query": 40');
  await expect(raw).toContainText('"mode": 8');
  await recovery.getByRole("button", { name: "查看完整风扇恢复参数", exact: true }).click();
  await expect(raw).toHaveCount(0);
  expect(await calls(page, "restore_oem_supply_mode")).toEqual([]);
  await configurationView(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  expect((await calls(page, "restore_oem_supply_mode"))[0].args.expectedRevision).toBe("supply-0");
});

test("设备条件阻止启用或恢复时在卡内显示原因", async ({ page }) => {
  const view = supplyModeFixture(); view.canEnable = false; view.canRestore = false; view.recovery = supplyRecoveryFixture();
  view.issue = "隔离自动档案已启用"; view.recoveryError = "隔离独显用于当前显示，不能关闭";
  await setup(page, view);
  await expect(toggle(page)).toBeDisabled();
  await expect(availableAction(page, "原厂供电模式", true)).toHaveCount(0);
  await expect(alert(page, "隔离自动档案已启用")).toBeVisible();
  await expect(alert(page, "隔离独显用于当前显示")).toBeVisible();
});

test("离页期间不读状态，返回后丢弃旧结果并重读；后台通知刷新其他模块", async ({ page }) => {
  await setup(page);
  const nav = page.getByRole("navigation", { name: "主导航" });
  await openWorkspace(page, "概览");
  const currentMode = page.getByRole("region", { name: "当前模式", exact: true });
  await expect(currentMode.getByRole("status")).toHaveText("性能");
  await nav.getByRole("button", { name: "电源与档案", exact: true }).click();
  await heldRead(page);
  await nav.getByRole("button", { name: "设置", exact: true }).click();
  const reads = (await calls(page, "read_oem_supply_mode")).length;
  await page.evaluate(() => {
    window.__uiFixture.supplyMode.review.onAc = false;
    window.__uiFixture.supplyMode.review.currentMode = 0;
    window.__uiFixture.performanceOptions.currentMode = 0;
    window.__uiFixture.fan.currentMode = 8;
    window.__uiEmit("hardware-controls-changed", null);
    window.__releaseSupplyRead();
  });
  await expect.poll(() => page.evaluate(() => window.__supplyReading)).toBe(0);
  expect((await calls(page, "read_oem_supply_mode")).length).toBe(reads);
  await nav.getByRole("button", { name: "电源与档案", exact: true }).click();
  await expect(card(page)).toContainText("当前模式 · 安静");
  await openWorkspace(page, "概览");
  await expect(currentMode.getByRole("status")).toHaveText("安静");
  expect(await page.evaluate(() => window.__supplyMaxReading)).toBe(1);
});

test("无原厂能力时隐藏模块，不发送策略写入", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); await openPowerArea(page, "电源设置");
  await expect.poll(async () => (await calls(page, "read_oem_supply_mode")).length).toBeGreaterThan(0);
  await expect(card(page)).toHaveCount(0);
  expect(await calls(page, "enable_oem_supply_mode")).toEqual([]);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("关闭节点时保留恢复预览的基线来源，未来条件不可用原因可见", async ({ page }) => {
  const view = supplyModeFixture(); view.canEnable = false; view.canRestore = true; view.recovery = supplyRecoveryFixture();
  view.review.gpu.current.enabled = false; view.review.gpu.current.automaticPowerOff = true; view.review.clocks.driver = null;
  view.review.clocks.deferredReason = "独显关闭，确认后在受控会话读取";
  view.review.gpu.resumeBattery.unavailableReason = "隔离电池唤醒关断条件不可用";
  view.issue = view.review.gpu.resumeBattery.unavailableReason;
  view.recovery.initialGpuClocks.origin = "poweredOff";
  await setup(page, view);
  await expect(toggle(page)).toBeDisabled();
  await expect(alert(page, "隔离电池唤醒关断条件不可用")).toBeVisible();
  await expect(await previewConfiguration(page, "原厂供电模式", true)).toContainText("独显节点关闭时，经本次授权的控制会话实读取得");
  await page.screenshot({ path: "target/ui-check/supply-mode-recovery-light-1000.png" });
});
