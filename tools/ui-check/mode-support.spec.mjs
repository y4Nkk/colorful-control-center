import { openWorkspace } from "./workspace.mjs";
import { availableAction } from "./configuration-review.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "预设模式", exact: true });
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); });

for (const product of [20720, 20724]) test(`未支持机型${product}保留当前模式，关闭编辑并说明范围`, async ({ page }) => {
  await page.addInitScript(product => { window.__uiFixture.unsupportedModeProduct = product; }, product);
  await page.goto("/"); await openWorkspace(page, "性能调校");
  await expect(panel(page)).toContainText(`尚未支持${product}机型`);
  await expect(panel(page).getByText("当前使用 · 性能", { exact: true })).toBeVisible();
  for (const radio of await panel(page).getByRole("radio").all()) await expect(radio).toBeDisabled();
  await expect(availableAction(page, "预设模式")).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|restore_)/.test(call.command)))).toEqual([]);
});

test("机型变化后后端拒绝先前可用的模式目标", async ({ page }) => {
  await page.goto("/"); await openWorkspace(page, "性能调校");
  const result = await page.evaluate(async () => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    const before = await invoke("read_oem_power_mode", {});
    window.__uiFixture.unsupportedModeProduct = 20720;
    const result = await invoke("apply_oem_power_mode", { expected: before.targets.find(target => target.mode === 3) });
    return { result, initial: window.__uiFixture.modeInitial ?? null, current: window.__uiFixture.performanceOptions.currentMode };
  });
  expect(result.result.ok).toBe(false);
  expect(result.result.message).toContain("尚未支持");
  expect(result.initial).toBeNull(); expect(result.current).toBe(2);
});
