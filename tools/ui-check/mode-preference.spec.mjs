import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const modes = page => page.getByRole("region", { name: "预设模式", exact: true });
const options = page => page.getByRole("region", { name: "性能选项", exact: true });
const calls = (page, command) => page.evaluate(command => window.__uiFixture.calls.filter(call => call.command === command), command);
async function setup(page, mode = 2, ac = 3) {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(({ mode, ac }) => {
    window.__uiFixture.performanceOptions.currentMode = mode;
    window.__uiFixture.acMode = ac;
    window.__uiFixture.battery.power.acOnline = false;
  }, { mode, ac });
  await page.goto("/"); await openWorkspace(page, "性能调校");
  await expect(modes(page).getByRole("button", { name: "确认应用模式", exact: true })).toBeDisabled();
  await expect(modes(page).getByLabel("当前插电偏好", { exact: true })).toContainText("插电偏好");
}
async function selectMode(page, name) {
  await modes(page).getByRole("radio", { name: new RegExp(name) }).click();
}
async function applyMode(page) {
  const before = (await calls(page, "apply_oem_power_mode")).length;
  expect(await calls(page, "apply_oem_power_mode")).toHaveLength(before);
  await modes(page).getByRole("button", { name: "确认应用模式", exact: true }).click();
  await expect.poll(async () => (await calls(page, "apply_oem_power_mode")).length).toBe(before + 1);
  await expect(modes(page).getByRole("button", { name: "确认应用模式", exact: true })).toBeDisabled();
}
async function enableTurbo(page) {
  const p = options(page);
  await p.getByRole("switch", { name: "Turbo 风扇", exact: true }).click();
  await p.getByRole("button", { name: "应用性能选项", exact: true }).click();
  await p.getByRole("button", { name: "确认应用性能选项？", exact: true }).click();
  await expect(p.getByText("当前开启", { exact: true })).toBeVisible();
}
async function restore(page, panel, label) {
  await panel.getByText("初始模式恢复预览", { exact: true }).click();
  await panel.getByRole("button", { name: label, exact: true }).click();
  expect(await calls(page, "restore_oem_power_mode")).toHaveLength(0);
  await panel.getByRole("button", { name: "恢复以上初始模式、选项及关联设置？", exact: true }).click();
}

test("电池场景读取不写入，点击当前模式也能预览保存插电偏好", async ({ page }) => {
  await setup(page, 0, 3);
  expect(await calls(page, "apply_oem_power_mode")).toEqual([]);
  await selectMode(page, "安静");
  await expect(modes(page)).toContainText("插电偏好记为安静");
  await expect(modes(page)).toContainText("娱乐");
  await applyMode(page);
  expect((await calls(page, "apply_oem_power_mode"))[0].args.expected.acMode).toBe(0);
  expect(await page.evaluate(() => window.__uiFixture.acMode)).toBe(0);
  await expect(options(page)).toContainText("当前为安静");
  await restore(page, modes(page), "恢复初始");
  await expect(modes(page).getByLabel("当前插电偏好", { exact: true })).toContainText("插电偏好 · 娱乐");
  const restored = (await calls(page, "restore_oem_power_mode"))[0];
  expect(typeof restored.args.expectedRevision).toBe("string");
  expect(restored.args.expectedRevision.length).toBeGreaterThan(10);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("明确重选当前模式可重新应用联动，打开页面不自动创建草稿", async ({ page }) => {
  await setup(page, 2, 2);
  await expect(modes(page)).not.toContainText("重新应用 ·");
  await selectMode(page, "性能");
  await expect(modes(page)).toContainText("重新应用 ·");
  await applyMode(page);
  expect((await calls(page, "apply_oem_power_mode"))[0].args.expected.mode).toBe(2);
  await expect(modes(page)).not.toContainText("重新应用 ·");
});

test("仅选项不接管插电偏好，恢复时也保留后来的偏好", async ({ page }) => {
  await setup(page, 2, 1); await enableTurbo(page);
  expect(await page.evaluate(() => window.__uiFixture.acMode)).toBe(1);
  await options(page).getByText("初始模式恢复预览", { exact: true }).click();
  await expect(options(page).getByRole("region", { name: "初始模式恢复目标", exact: true })).toContainText("保持当前偏好");
  await page.evaluate(() => { window.__uiFixture.acMode = 3; });
  await options(page).getByRole("button", { name: "恢复模式与选项", exact: true }).click();
  await options(page).getByRole("button", { name: "恢复以上初始模式、选项及关联设置？", exact: true }).click();
  await expect(options(page).getByRole("switch", { name: "Turbo 风扇", exact: true })).not.toBeChecked();
  expect(await page.evaluate(() => window.__uiFixture.acMode)).toBe(3);
});

test("选项之后再选择模式会扩充共享恢复目标，并撤销另一个入口的旧确认", async ({ page }) => {
  await setup(page); await enableTurbo(page);
  await options(page).getByText("初始模式恢复预览", { exact: true }).click();
  await options(page).getByRole("button", { name: "恢复模式与选项", exact: true }).click();
  await expect(options(page).getByRole("button", { name: "恢复以上初始模式、选项及关联设置？", exact: true })).toBeVisible();
  await selectMode(page, "娱乐"); await applyMode(page);
  await expect(options(page).getByRole("button", { name: "恢复模式与选项", exact: true })).toBeVisible();
  await expect(options(page)).toContainText("当前为娱乐");
  expect(await calls(page, "restore_oem_power_mode")).toEqual([]);
  expect(await page.evaluate(() => window.__uiFixture.modeInitial.acMode)).toBe(3);
  await options(page).getByRole("button", { name: "恢复模式与选项", exact: true }).click();
  await options(page).getByRole("button", { name: "恢复以上初始模式、选项及关联设置？", exact: true }).click();
  await expect(options(page).getByRole("switch", { name: "Turbo 风扇", exact: true })).not.toBeChecked();
  await expect(modes(page)).toContainText("当前使用 · 性能");
});

test("后端拒绝已经改变的初始快照摘要，不执行未预览恢复", async ({ page }) => {
  await setup(page); await selectMode(page, "省电"); await applyMode(page);
  await modes(page).getByText("初始模式恢复预览", { exact: true }).click();
  await modes(page).getByRole("button", { name: "恢复初始", exact: true }).click();
  await page.evaluate(() => { window.__uiFixture.modeInitial.acMode = 0; });
  await modes(page).getByRole("button", { name: "恢复以上初始模式、选项及关联设置？", exact: true }).click();
  await expect(modes(page).getByRole("alert")).toContainText("恢复目标已变化");
  expect(await page.evaluate(() => window.__uiFixture.performanceOptions.currentMode)).toBe(1);
  expect(await page.evaluate(() => window.__uiFixture.acMode)).toBe(1);
});

test("性能选项读取失败会清除旧的可执行恢复快照", async ({ page }) => {
  await setup(page); await enableTurbo(page);
  await options(page).getByText("初始模式恢复预览", { exact: true }).click();
  await expect(options(page).getByRole("button", { name: "恢复模式与选项", exact: true })).toBeVisible();
  await page.evaluate(() => { window.__uiFixture.performanceReadFault = "隔离读取失败"; });
  await options(page).getByRole("button", { name: "重新读取性能选项", exact: true }).click();
  await expect(options(page).getByRole("alert")).toContainText("隔离读取失败");
  await expect(options(page).getByRole("button", { name: "恢复模式与选项", exact: true })).toHaveCount(0);
  expect(await calls(page, "restore_oem_power_mode")).toEqual([]);
});

test("初始恢复完整目标适配宽窄和深浅主题，折叠原始风扇参数", async ({ page }) => {
  await setup(page); await selectMode(page, "安静"); await applyMode(page);
  await modes(page).getByText("初始模式恢复预览", { exact: true }).click();
  const target = modes(page).getByRole("region", { name: "初始模式恢复目标", exact: true });
  for (const theme of ["light", "dark"]) {
    await page.evaluate(value => { document.documentElement.dataset.theme = value; }, theme);
    for (const width of [360, 1000, 1440]) {
      await page.setViewportSize({ width, height: 950 }); await target.scrollIntoViewIfNeeded();
      const geometry = await target.evaluate(element => {
        const panel = element.closest('[aria-label="预设模式"]'), style = getComputedStyle(panel);
        return { width: element.clientWidth, scroll: element.scrollWidth, border: parseFloat(getComputedStyle(element).borderTopWidth),
          available: panel.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight) };
      });
      expect(geometry.scroll).toBeLessThanOrEqual(geometry.width + 1); expect(geometry.border).toBeGreaterThan(0);
      expect(geometry.width).toBeGreaterThan(geometry.available * 0.94);
      await expect(target.getByRole("heading", { name: "模式与性能", exact: true })).toBeVisible();
      await expect(target.getByRole("heading", { name: "散热策略", exact: true })).toBeVisible();
      expect(await target.locator('.ccc-facts[data-layout="tiles"] .ccc-fact-row').evaluateAll(rows => rows.every(row => parseFloat(getComputedStyle(row).borderTopWidth) === 0))).toBe(true);
      await expect(target).toContainText("Turbo风扇偏好"); await expect(target).toContainText("DTT偏好");
      if (width !== 1000) await page.screenshot({ path: `target/ui-check/mode-restore-${theme}-${width}.png` });
    }
  }
  await target.getByText("查看完整风扇恢复参数", { exact: true }).click();
  await expect(target.locator("pre")).toContainText('"fields"');
  expect(await calls(page, "restore_oem_power_mode")).toEqual([]);
});
