import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";
import { previewConfiguration, submitConfiguration } from "./configuration-review.mjs";

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  // 基准机形态：固件未声明 CPUOC（page6 的 PL 从未写入）、MSR 运行时不可用
  await page.addInitScript(() => {
    const power = window.__uiFixture.power;
    power.pl1W = null; power.pl2W = null;
    power.defaultPl1W = 135; power.defaultPl2W = 135;
    power.msrRuntimeAvailable = false; power.msrRuntimeError = "打开 \\\\.\\PawnIO 失败";
    power.runtime = { pl1W: null, pl2W: null, coreOffsetMv: null, gtOffsetMv: null };
  });
});

async function openCpuPanel(page) {
  await openWorkspace(page, "性能与散热");
  const panel = page.getByRole("region", { name: "CPU 功耗与电压", exact: true });
  await expect(panel.getByRole("slider", { name: "持续功耗上限", exact: true })).toBeVisible();
  return panel;
}

test("PL 保存值未上报时锚定出厂默认并整组首写", async ({ page }) => {
  await page.goto("/");
  const panel = await openCpuPanel(page);
  await expect(panel.getByRole("tab", { name: "保存设置", exact: true })).toHaveAttribute("aria-selected", "true");
  const pl1 = panel.getByRole("slider", { name: "持续功耗上限", exact: true });
  const pl2 = panel.getByRole("slider", { name: "短时功耗上限", exact: true });
  await expect(pl1).toBeEnabled();
  await expect(pl2).toBeEnabled();
  await expect(pl1).toHaveAttribute("aria-valuetext", "135 W");
  await expect(pl2).toHaveAttribute("aria-valuetext", "135 W");
  await expect(panel.getByText("未上报", { exact: true })).toHaveCount(0);

  await pl1.fill("120");
  await expect(pl1).toHaveAttribute("aria-valuetext", "120 W");
  await panel.locator(".ccc-range-marker").hover();
  await expect(page.getByRole("tooltip")).toHaveText("当前 135 W");

  const view = await previewConfiguration(page, "保存设置 · CPU");
  await expect(view.getByText("未上报")).toHaveCount(2);
  await expect(view.getByText("120 W", { exact: true })).toBeVisible();
  await expect(view.getByText("135 W", { exact: true })).toBeVisible();
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === "apply_saved_cpu") await new Promise(resolve => { window.__finishCpuSave = resolve; });
      return invoke(command, args);
    };
  });
  const apply = view.locator("[data-configuration-apply]");
  await apply.click();
  await expect(apply).toHaveCSS("cursor", "progress");
  for (const tab of await panel.getByRole("tab").all()) {
    await expect(tab).toBeDisabled();
    await expect(tab).toHaveCSS("cursor", "not-allowed");
  }
  await expect(pl1).toHaveCSS("cursor", "not-allowed");
  await page.evaluate(() => window.__finishCpuSave());
  await expect(apply).toHaveCount(0);

  await expect(pl1).toHaveAttribute("aria-valuetext", "120 W");
  await expect(pl2).toHaveAttribute("aria-valuetext", "135 W");
  await expect(pl1).toHaveCSS("cursor", "pointer");
  await expect(panel.locator(".ccc-range-marker")).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.calls
    .filter(call => call.command === "apply_saved_cpu"))).toEqual([
    { command: "apply_saved_cpu", args: { controls: { pl1W: 120, pl2W: 135, coreOffsetMv: null, gtOffsetMv: null } } },
  ]);
});

test("MSR 不可用时即时页签整组禁用且首载停在保存页签", async ({ page }) => {
  await page.goto("/");
  const panel = await openCpuPanel(page);
  await panel.getByRole("tab", { name: "立即应用", exact: true }).click();
  await expect(panel.getByText("即时调节不可用", { exact: true })).toBeVisible();
  for (const name of ["持续功耗上限", "短时功耗上限", "核心电压偏移", "核显电压偏移"])
    await expect(panel.getByRole("slider", { name, exact: true })).toBeDisabled();
});

test("回读丢失功耗起点后清除待提交的功耗草稿", async ({ page }) => {
  await page.goto("/");
  const panel = await openCpuPanel(page);
  const slider = panel.getByRole("slider", { name: "持续功耗上限", exact: true });
  await slider.fill("120");
  await page.evaluate(() => {
    window.__uiFixture.power.defaultPl1W = null;
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(slider).toBeDisabled();
  await page.evaluate(() => {
    window.__uiFixture.power.defaultPl1W = 135;
    window.__uiEmit("hardware-controls-changed", null);
  });
  await expect(slider).toBeEnabled();
  await expect(slider).toHaveAttribute("aria-valuetext", "135 W");
  expect(await page.evaluate(() => window.__uiFixture.calls.some(call => call.command === "apply_saved_cpu"))).toBe(false);
});

for (const [label, values] of [
  ["两项缺失", { defaultPl1W: null, defaultPl2W: null }],
  ["一项缺失", { defaultPl1W: null }],
  ["默认值超界", { defaultPl2W: 180 }],
  ["默认值逆序", { defaultPl1W: 150, defaultPl2W: 120 }],
]) {
  test(`功耗起点${label}时锁定功耗滑杆，仍可独立保存电压`, async ({ page }) => {
    await page.goto("/");
    await page.evaluate(values => Object.assign(window.__uiFixture.power, values), values);
    const panel = await openCpuPanel(page);
    for (const name of ["持续功耗上限", "短时功耗上限"]) {
      const slider = panel.getByRole("slider", { name, exact: true });
      await expect(slider).toBeDisabled();
      await expect(slider).toHaveAttribute("aria-valuetext", "未上报");
    }
    await expect(panel).toContainText("缺少有效的成对功耗读数");
    await panel.getByRole("slider", { name: "核心电压偏移", exact: true }).fill("-10");
    const view = await previewConfiguration(page, "保存设置 · CPU");
    await expect(view.getByText(/功耗上限/)).toHaveCount(0);
    await submitConfiguration(view);
    expect(await page.evaluate(() => window.__uiFixture.calls
      .filter(call => call.command === "apply_saved_cpu").map(call => call.args.controls))).toEqual([
      { pl1W: null, pl2W: null, coreOffsetMv: -10, gtOffsetMv: null },
    ]);
  });
}
