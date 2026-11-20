import { availableAction, configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openWorkspace, openPowerArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { supplyModeFixture, supplyRecoveryFixture } from "./supply-mode-fixture.mjs";

const card = page => page.getByRole("region", { name: "原厂供电模式", exact: true });
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
async function openDetails(page) {
  await card(page).getByRole("button", { name: "详情", exact: true }).click();
  return page.getByRole("dialog", { name: "供电状态与切换规则", exact: true });
}
async function openScenario(container, label) {
  await container.getByRole("region", { name: label, exact: true }).getByRole("button", { name: "联动目标", exact: true }).click();
  return container.page().getByRole("dialog", { name: "联动目标", exact: true });
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

test("GPU恢复读失败仍展示已知初始目标，不能确认执行或冒充跳过", async ({ page }) => {
  const view = supplyModeFixture();
  view.enabled = true; view.active = true; view.canEnable = false; view.canRestore = false;
  view.recovery = supplyRecoveryFixture();
  view.recovery.currentGpuClocks = { status: "unavailable", value: "隔离驱动暂不可读" };
  view.recoveryError = "GPU频率恢复前态不可用：隔离驱动暂不可读";
  await setup(page, view);
  await expect(availableAction(page, "原厂供电模式", true)).toHaveCount(0);
  const dialog = await openDetails(page);
  const recovery = dialog.getByRole("region", { name: "供电恢复详情", exact: true });
  const current = recovery.getByRole("heading", { name: "当前观察", exact: true }).locator("..");
  await expect(current).toContainText("当前频率读取失败");
  await expect(current).not.toContainText("跳过频率调校");
  await expect(current).not.toContainText("MHz");
  await expect(recovery.getByRole("heading", { name: "初始目标", exact: true }).locator("..")).toContainText("987.654 MHz");
  await page.keyboard.press("Escape");
  await expect(recovery).toHaveCount(0);
  await expect(dialog).toHaveCount(0);
  await expect(card(page).getByRole("button", { name: "详情", exact: true })).toBeFocused();
  expect(await calls(page, "restore_oem_supply_mode")).toHaveLength(0);
});

test("供电详情展示六个目标，切换开关直接发送", async ({ page }) => {
  await setup(page);
  const review = await openDetails(page);
  const conditions = { "接电时": "1700 MHz", "使用电池时": null, "启动 · 插电": "保持当前Windows电源计划",
    "启动 · 电池": null, "唤醒 · 插电": "跳过频率调校", "唤醒 · 电池": "切模式后确保独显关闭" };
  for (const [label, expected] of Object.entries(conditions)) {
    const scenario = await openScenario(review, label);
    await expect(scenario).toContainText("风扇");
    await expect(scenario).toContainText("固件节能值");
    const acPreference = scenario.getByLabel("模式联动目标").locator(".ccc-fact-row").filter({ hasText: "插电模式偏好" });
    await expect(acPreference.locator("dd")).toHaveText("保持当前");
    if (expected) await expect(scenario).toContainText(expected);
    await page.keyboard.press("Escape");
  }
  expect(await calls(page, "enable_oem_supply_mode")).toEqual([]);
  await page.keyboard.press("Escape");
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

test("供电方案重新读取后展示新目标，一次应用使用最新修订", async ({ page }) => {
  await setup(page);
  const review = await openDetails(page);
  await expect(review.getByRole("region", { name: "接电时", exact: true })).toContainText("性能");
  await page.evaluate(() => {
    const { supplyMode } = window.__uiFixture;
    supplyMode.revision = "changed";
    supplyMode.review.ac = supplyMode.review.targets[0];
  });
  await page.keyboard.press("Escape");
  await reread(page);
  await expect((await openDetails(page)).getByRole("region", { name: "接电时", exact: true })).toContainText("安静");
  expect(await calls(page, "enable_oem_supply_mode")).toEqual([]);
  await page.keyboard.press("Escape");
  await enable(page);
  expect((await calls(page, "enable_oem_supply_mode"))[0].args.expectedRevision).toBe("changed");
});

test("后端拒绝失效切换，错误经过定时重读仍保留", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => { window.__uiFixture.supplyModeFault = "隔离来源已变化，未执行"; });
  await toggle(page).click();
  const details = await openDetails(page);
  await expect(details).toContainText("隔离来源已变化");
  const reads = (await calls(page, "read_oem_supply_mode")).length;
  await expect.poll(async () => (await calls(page, "read_oem_supply_mode")).length).toBeGreaterThan(reads);
  await expect(details).toContainText("隔离来源已变化");
  await page.keyboard.press("Escape");
  await reread(page);
  await expect(await openDetails(page)).not.toContainText("隔离来源已变化");
});

test("读失败清除可执行快照，暂停仍能发送，恢复后重读当前事实", async ({ page }) => {
  await setup(page); await enable(page);
  await page.evaluate(() => { window.__uiFixture.supplyModeReadError = "隔离状态读取失败"; });
  await expect(await openDetails(page)).toContainText("隔离状态读取失败");
  await page.keyboard.press("Escape");
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
  await expect(await openDetails(page)).toContainText("许可已撤销");
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

test("设备条件阻止启用或恢复时明确显示原因，成功与收尾错误分开", async ({ page }) => {
  const view = supplyModeFixture(); view.canEnable = false; view.canRestore = false; view.recovery = supplyRecoveryFixture();
  view.issue = "隔离自动档案已启用"; view.recoveryError = "隔离独显用于当前显示，不能关闭";
  view.last = { atMs: 1000, trigger: "supplyChange", applied: true, previousMode: 3, mode: 2, onAc: true, error: "隔离收尾待重试" };
  await setup(page, view);
  await expect(toggle(page)).toBeDisabled();
  await expect(availableAction(page, "原厂供电模式", true)).toHaveCount(0);
  const details = await openDetails(page);
  await expect(details.getByRole("region", { name: "最近原厂自动切换", exact: true })).toContainText("已应用，收尾待处理");
  await expect(details).toContainText("隔离独显用于当前显示");
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
  const details = await openDetails(page);
  await expect(details.locator('[data-power-source="battery"]')).toBeVisible();
  await details.getByRole("button", { name: "关闭弹窗", exact: true }).click();
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

test("宽窄深浅布局保留模块边框和间距，不使用比例表达模式", async ({ page }) => {
  await setup(page);
  for (const theme of ["light", "dark"]) {
    await page.evaluate(value => { document.documentElement.dataset.theme = value; }, theme);
    for (const width of [360, 640, 1000, 1440]) {
      await page.setViewportSize({ width, height: 950 }); await card(page).scrollIntoViewIfNeeded();
      const details = await openDetails(page);
      const geometry = await details.getByLabel("六种条件的原厂联动").evaluate(element => ({ width: element.clientWidth, scroll: element.scrollWidth,
        border: parseFloat(getComputedStyle(element).borderTopWidth),
        cases: [...element.querySelectorAll(".ccc-supply-mode-case")].map(item => ({ border: parseFloat(getComputedStyle(item).borderTopWidth), rect: item.getBoundingClientRect().toJSON() })) }));
      expect(geometry.scroll).toBeLessThanOrEqual(geometry.width + 1);
      expect(geometry.cases).toHaveLength(6); expect(geometry.cases.every(item => item.border > 0)).toBe(true);
      if (width >= 1000) expect(geometry.cases[1].rect.left).toBeGreaterThan(geometry.cases[0].rect.right);
      if (width === 360) expect(geometry.cases[1].rect.top).toBeGreaterThan(geometry.cases[0].rect.bottom);
      await expect(details.getByRole("meter")).toHaveCount(0);
      const artwork = details.getByLabel("六种条件的原厂联动").locator("svg[data-mode-art]");
      await expect(artwork).toHaveCount(6);
      const sizes = await artwork.evaluateAll(items => items.map(svg => svg.getBoundingClientRect().toJSON()));
      expect(sizes.every(size => size.width === 40 && size.height === 40)).toBe(true);
      if (width === 360 || width === 1440) {
        await page.screenshot({ path: `target/ui-check/supply-mode-${theme}-${width}.png` });
      }
      await page.keyboard.press("Escape");
    }
  }
});

test("关闭节点时保留未知频率和基线来源，未来条件不可用原因可见", async ({ page }) => {
  const view = supplyModeFixture(); view.canEnable = false; view.canRestore = true; view.recovery = supplyRecoveryFixture();
  view.review.gpu.current.enabled = false; view.review.gpu.current.automaticPowerOff = true; view.review.clocks.driver = null;
  view.review.clocks.deferredReason = "独显关闭，确认后在受控会话读取";
  view.review.gpu.resumeBattery.unavailableReason = "隔离电池唤醒关断条件不可用";
  view.issue = view.review.gpu.resumeBattery.unavailableReason;
  view.recovery.initialGpuClocks.origin = "poweredOff";
  await setup(page, view);
  await expect(toggle(page)).toBeDisabled();
  const details = await openDetails(page);
  await expect(details.getByRole("region", { name: "当前GPU频率", exact: true })).toContainText("当前频率未知");
  await expect(details.getByRole("region", { name: "唤醒 · 电池", exact: true })).toContainText("隔离电池唤醒关断条件不可用");
  await expect(await openScenario(details, "唤醒 · 电池")).toContainText("自动关断关闭");
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await expect(await previewConfiguration(page, "原厂供电模式", true)).toContainText("独显节点关闭时，经本次授权的控制会话实读取得");
  await page.screenshot({ path: "target/ui-check/supply-mode-recovery-light-1000.png" });
});
