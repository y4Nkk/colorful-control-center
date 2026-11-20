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
  await expect(panel.getByRole("radio", { name: "保存设置", exact: true })).toBeChecked();
  const pl1 = panel.getByRole("slider", { name: "持续功耗上限", exact: true });
  const pl2 = panel.getByRole("slider", { name: "短时功耗上限", exact: true });
  await expect(pl1).toBeEnabled();
  await expect(pl2).toBeEnabled();
  await expect(pl1).toHaveAttribute("aria-valuetext", "135 W");
  await expect(pl2).toHaveAttribute("aria-valuetext", "135 W");
  await expect(panel.getByText("未上报", { exact: true })).toHaveCount(0);

  await pl1.fill("120");
  await expect(pl1).toHaveAttribute("aria-valuetext", "120 W");
  await expect(panel.locator(".ccc-range-marker[title='当前 135 W']")).toHaveCount(1);

  const view = await previewConfiguration(page, "保存设置 · CPU");
  await expect(view.getByText("未上报")).toHaveCount(2);
  await expect(view.getByText("120 W", { exact: true })).toBeVisible();
  await expect(view.getByText("135 W", { exact: true })).toBeVisible();
  await submitConfiguration(view);

  await expect(pl1).toHaveAttribute("aria-valuetext", "120 W");
  await expect(pl2).toHaveAttribute("aria-valuetext", "135 W");
  await expect(panel.locator(".ccc-range-marker")).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.calls
    .filter(call => call.command === "apply_saved_cpu"))).toEqual([
    { command: "apply_saved_cpu", args: { controls: { pl1W: 120, pl2W: 135, coreOffsetMv: null, gtOffsetMv: null } } },
  ]);
});

test("MSR 不可用时即时页签整组禁用且首载停在保存页签", async ({ page }) => {
  await page.goto("/");
  const panel = await openCpuPanel(page);
  await panel.getByRole("radio", { name: "立即应用", exact: true }).check();
  await expect(panel.getByText("即时调节不可用", { exact: true })).toBeVisible();
  for (const name of ["持续功耗上限", "短时功耗上限", "核心电压偏移", "核显电压偏移"])
    await expect(panel.getByRole("slider", { name, exact: true })).toBeDisabled();
});
