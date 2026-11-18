import { availableAction, configurationDialog, openConfigurationReview } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const panel = page => page.getByRole("region", { name: "启动档案", exact: true });
const saves = page => page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "set_startup_profile"));
async function setup(page, pending = false) {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(pending => {
    const data = window.__uiFixture, invoke = window.__TAURI_INTERNALS__.invoke;
    data.profile.settings.oemPowerMode = 3;
    data.startupReadFault = "启动档案设置版本已过期，请重新预览并保存启动选择";
    window.__TAURI_INTERNALS__.invoke = async (command, args = {}) => {
      if (command === "get_startup_profile") {
        if (pending) await new Promise(resolve => window.__releaseStartupRead = resolve);
        if (data.startupReadFault) throw new Error(data.startupReadFault);
      }
      if (command === "set_startup_profile" && data.startupSaveFault) throw new Error(data.startupSaveFault);
      return invoke(command, args);
    };
  }, pending);
  await page.goto("/"); await openWorkspace(page, "自动策略");
  await expect(panel(page)).toBeVisible();
}
async function confirmClear(page) {
  await openConfigurationReview(page, "启动档案");
  expect(await saves(page)).toHaveLength(0);
  await configurationDialog(page).getByRole("button", { name: "确认应用", exact: true }).click();
}

test("旧启动选择不可读时保留档案列表，重新预览后才保存新增模式行为", async ({ page }) => {
  await setup(page);
  await expect(panel(page)).toContainText("当前保存的启动选择未知");
  await panel(page).getByRole("combobox", { name: "启动档案", exact: true }).click();
  await page.getByRole("option", { name: "隔离档案", exact: true }).click();
  const review = await openConfigurationReview(page, "启动档案");
  await review.getByRole("button", { name: "启动档案参数", exact: true }).click();
  const details = page.getByRole("dialog", { name: "启动档案参数", exact: true });
  await expect(details.getByLabel("模式联动目标")).toContainText("插电偏好记为娱乐");
  await expect(details).toContainText("启动时保存插电偏好");
  await page.keyboard.press("Escape");
  expect(await saves(page)).toHaveLength(0);
  await review.getByRole("button", { name: "确认应用", exact: true }).click();
  await expect(availableAction(page, "启动档案")).toHaveCount(0);
  expect((await saves(page))[0].args.expected.settings.oemPowerMode).toBe(3);
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|restore_)/.test(call.command)))).toEqual([]);
});

test("未知启动选择可以明确确认取消，不伪装为已经取消", async ({ page }) => {
  await setup(page);
  await expect(availableAction(page, "启动档案")).toBeEnabled();
  await expect(panel(page)).toContainText("当前保存的启动选择未知");
  await confirmClear(page);
  await expect(availableAction(page, "启动档案")).toHaveCount(0);
  expect((await saves(page))[0].args.expected).toBeNull();
  await expect(panel(page)).not.toContainText("当前保存的启动选择未知");
});

test("启动读取未结束不能保存或选择，完成后仍可显式修复", async ({ page }) => {
  await setup(page, true);
  await expect(availableAction(page, "启动档案")).toHaveCount(0);
  await expect(panel(page).getByRole("combobox", { name: "启动档案", exact: true })).toBeDisabled();
  expect(await saves(page)).toHaveLength(0);
  await page.evaluate(() => window.__releaseStartupRead());
  await expect(availableAction(page, "启动档案")).toBeEnabled();
  await expect(panel(page).getByRole("alert")).toContainText("版本已过期");
});

test("重新确认保存失败不丢失未知状态，重试成功才清除错误", async ({ page }) => {
  await setup(page);
  await expect(availableAction(page, "启动档案")).toBeEnabled();
  await page.evaluate(() => window.__uiFixture.startupSaveFault = "隔离文件被占用");
  await confirmClear(page);
  await expect(panel(page).getByRole("alert")).toContainText("隔离文件被占用");
  await expect(panel(page)).toContainText("当前保存的启动选择未知");
  await page.evaluate(() => window.__uiFixture.startupSaveFault = null);
  await confirmClear(page);
  await expect(availableAction(page, "启动档案")).toHaveCount(0);
  await expect(panel(page).getByRole("alert")).toHaveCount(0);
});
