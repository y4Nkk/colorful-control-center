import { openWorkspace as navigate, openWorkspace, openPowerArea } from "./workspace.mjs";
import { pageActions, previewConfiguration } from "./configuration-review.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openProfileSave, profilePanel, profileSaveDialog } from "./profile-review.mjs";

async function choose(page, label, option) {
  await page.getByRole("combobox", { name: label, exact: true }).click();
  await page.getByRole("option", { name: option, exact: true }).click();
}
test.beforeEach(async ({ page }) => { await page.addInitScript({ content: MOCK_IPC }); await page.goto("/"); });

test("设备下拉直接试用，固件风扇模式点选后应用", async ({ page }) => {
  await navigate(page, "键盘与灯光");
  await choose(page, "灯效", "呼吸");
  await expect(page.getByRole("checkbox", { name: "左区开关" })).toBeVisible();
  await navigate(page, "设备配置");
  await choose(page, "目标刷新率", "144 Hz");
  const screen = page.getByRole("region", { name: "屏幕显示", exact: true });
  await expect(screen.getByRole("status")).toContainText("试用 144 Hz");
  await expect(pageActions(page)).toHaveCount(0);
  await screen.getByRole("button", { name: "取消本次试用", exact: true }).click();
  await openPowerArea(page, "电源设置");
  await choose(page, "启动档案", "隔离档案");
  await navigate(page, "设备配置");
  await expect(page.getByRole("radio", { name: "关闭独显" })).toBeDisabled();
  await page.evaluate(() => window.__uiFixture.fan.availableModes.push(8));
  await openWorkspace(page, "性能与散热");
  const fanModes = page.getByRole("radiogroup", { name: "风扇模式", exact: true });
  await fanModes.getByRole("radio", { name: "寂静模式", exact: true }).click();
  await expect(fanModes.getByRole("radio", { name: "寂静模式", exact: true })).toBeChecked();
  await expect(page.getByText("固件 · 寂静模式", { exact: true })).toHaveCount(0);
  await fanModes.getByRole("radio", { name: "自动", exact: true }).click();
  await expect(fanModes.getByRole("radio", { name: "自动", exact: true })).toBeChecked();
  await expect(page.getByRole("region", { name: "散热控制", exact: true })).toHaveCount(1);
  await page.evaluate(() => window.__uiFixture.fanKind = "rpmTable");
  await navigate(page, "设置"); await openWorkspace(page, "性能与散热");
  await choose(page, "选择 RPM 级别", "第 4 级");
  await expect(page.getByRole("spinbutton", { name: "RPM 温度", exact: true })).toHaveValue("55");
  await expect(page.locator("select,option")).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.calls
    .filter(({ command }) => /^(apply_|restore_|set_fan|save_|import_|delete_|set_startup)/.test(command)).map(call => call.command)))
    .toEqual(["apply_keyboard_lighting", "apply_display_refresh", "set_fan_mode", "set_fan_mode"]);
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("档案输入/文件引用和设置搜索选择接入同一原语", async ({ page }) => {
  await openPowerArea(page, "配置档案");
  await openProfileSave(page, "配置草稿");
  await profileSaveDialog(page).getByRole("button", { name: "取消", exact: true }).click();
  const file = await page.evaluate(() => ({ ...window.__uiFixture.profile, name: "导入草稿" }));
  await page.getByLabel("导入配置档案", { exact: true }).setInputFiles({
    name: "fixture.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(file)),
  });
  const review = await previewConfiguration(page);
  await expect(review.getByRole("button", { name: "导入档案", exact: true })).toBeVisible();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await navigate(page, "设置");
  await page.evaluate(() => window.__uiFixture.delay = 200);
  const font = page.getByRole("combobox", { name: "选择界面字体", exact: true });
  await font.click(); await page.getByRole("searchbox", { name: "搜索字体" }).fill("Consolas");
  await page.getByRole("option", { name: /Consolas/ }).click();
  await expect(font).toBeFocused(); await expect(font).toHaveText("Consolas");
  await expect(font).toBeEnabled();
  await openPowerArea(page, "配置档案");
  await expect(profilePanel(page).getByRole("heading", { name: "导入草稿" })).toBeVisible();
  expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
});

test("各工作区浅深与窄宽窗口无控件溢出", async ({ page }) => {
  for (const title of ["概览", "性能与散热", "键盘与灯光", "电源与档案", "设备配置", "设置"]) {
    await navigate(page, title);
    const areas = title === "电源与档案" ? ["电源设置", "配置档案"] : [null];
    for (const area of areas) {
      if (area) await openPowerArea(page, area);
      await expect(pageActions(page)).toHaveCount(0);
      for (const theme of ["light", "dark"]) {
        await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
        for (const width of [360, 700, 1280]) {
          await page.setViewportSize({ width, height: 720 });
          await page.waitForFunction(width => document.querySelector(".ccc-shell").classList.contains("is-collapsed") === (width <= 700), width);
          const overflow = await page.locator("button,input:not([hidden]),.ccc-surface").evaluateAll(nodes => nodes
            .filter(el => el.checkVisibility() && el.closest('.ccc-page:not([hidden])'))
            .filter(el => el.getBoundingClientRect().right > innerWidth + 1).map(el => el.getAttribute("aria-label") ?? el.className));
          expect(overflow).toEqual([]);
        }
      }
    }
  }
});

test("硬件加速直接保存，重启提示只出现在运行与诊断卡内", async ({ page }) => {
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      const result = await invoke(command, args);
      if (command === "set_app_preferences" || command === "get_app_preferences")
        return { ...result, restartRequired: !result.settings.hardwareAcceleration };
      return result;
    };
  });
  await navigate(page, "设置");
  const runtime = page.getByRole("region", { name: "运行与诊断", exact: true });
  const toggle = runtime.getByRole("switch", { name: "硬件加速", exact: true });
  await expect(toggle).toBeChecked();
  await toggle.click();
  await expect(toggle).not.toBeChecked();
  await expect(runtime.getByRole("status")).toHaveText("重启应用后生效");
  await expect(runtime.getByRole("button", { name: "立即重启", exact: true })).toBeVisible();
  await expect(pageActions(page)).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "restart_application"))).toEqual([]);
  await toggle.click();
  await expect(runtime.getByRole("status")).toHaveCount(0);
});
