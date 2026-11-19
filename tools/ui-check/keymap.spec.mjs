import { availableAction, configurationView, previewConfiguration } from "./configuration-review.mjs";
import { openWorkspace } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const region = page => page.getByRole("region", { name: "按键映射与宏", exact: true });
const editorOf = page => page.getByRole("dialog", { name: "编辑按键映射与宏", exact: true });
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_keymap|restore_keymap)$/.test(call.command)));
const choose = async (page, label, option) => {
  const editor = editorOf(page);
  await editor.getByRole("combobox", { name: label, exact: true }).click();
  await editor.getByRole("option", { name: option, exact: true }).click();
};
async function openEditor(page) {
  await region(page).getByRole("button", { name: "编辑映射", exact: true }).click();
  return editorOf(page);
}
const finish = editor => editor.getByRole("button", { name: "完成编辑", exact: true }).click();
const reread = page => region(page).getByRole("button", { name: "重新读取映射", exact: true }).click();
async function openDetails(page) {
  await region(page).getByRole("button", { name: "详情", exact: true }).click();
  return page.getByRole("dialog", { name: "映射详情", exact: true });
}
async function navigateFromHotkey(page) {
  await page.evaluate(() => {
    const status = window.__uiFixture.hotkeys.status;
    status.running = true;
    status.navigationRequest = { id: 92, kind: "displayRotation", ready: true };
    window.__uiEmit("hotkeys-state", status);
  });
  await expect(page.getByRole("region", { name: "显示控制区域", exact: true })).toBeFocused();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(() => {
    window.__uiFixture.keymap = { revision: "keymap-0", configuration: { devices: [
      { device: "keyboard", enabled: true, bindings: [{ trigger: 30, action: { kind: "disable" } }] },
      { device: "mouse", enabled: false, bindings: [] }] }, enabledFlags: [1,0], sourceName: "隔离原厂活动配置", baselineName: null,
      canRestore: false, recoveryRequired: false, blockedReason: null, warnings: ["未提供驱动映射表回读"], programsRequired:false, programs:{ running:false,error:null } };
  });
  await page.goto("/"); await openWorkspace(page, "键盘与灯光");
});

test("改键宏在模块内预览并应用，恢复保留原厂来源", async ({ page }) => {
  const panel = await openEditor(page);
  await panel.getByRole("button", { name: "A 禁用按键", exact: true }).click();
  await choose(page, "执行动作", "按键与鼠标宏");
  await panel.getByRole("spinbutton", { name: "第1步延时", exact: true }).fill("123");
  await finish(panel);
  expect(await writes(page)).toHaveLength(0);
  const review = await previewConfiguration(page, "按键映射与宏");
  await expect(review.getByLabel("待应用变更")).toContainText("随后123ms");
  expect(await writes(page)).toHaveLength(0);
  await configurationView(page).getByRole("button", { name: "应用按键映射", exact: true }).click();
  const calls = await writes(page); expect(calls).toHaveLength(1);
  expect(calls[0].args.request.configuration.devices.map(group => group.device)).toEqual(["keyboard"]);
  expect(calls[0].args.request.configuration.devices[0].bindings[0].action.steps[0].delayAfterMs).toBe(123);
  await previewConfiguration(page, "按键映射与宏", true); expect(await writes(page)).toHaveLength(1);
  await expect(configurationView(page, true)).toContainText("隔离原厂配置");
  await configurationView(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await openEditor(page);
  await expect(panel.getByRole("combobox", { name: "执行动作", exact: true })).toHaveText("禁用按键");
});

test("鼠标映射及文本前后动作延时均进入草稿，未平衡按键不能应用", async ({ page }) => {
  const panel = await openEditor(page);
  await panel.getByRole("radio", { name: "鼠标", exact: true }).check();
  await panel.getByRole("button", { name: "添加映射", exact: true }).click();
  await choose(page, "执行动作", "发送文本");
  await expect(availableAction(page, "按键映射与宏")).toHaveCount(0);
  await panel.getByRole("textbox", { name: "文本宏内容" }).fill("你好🌈\n第二行");
  await panel.getByRole("button", { name: "添加前置按键", exact: true }).click();
  await panel.getByRole("spinbutton", { name: "前置按键第1键按下延时", exact: true }).fill("42");
  await panel.getByRole("button", { name: "添加后置按键", exact: true }).click();
  await panel.getByRole("spinbutton", { name: "后置按键第1键释放延时", exact: true }).fill("75");
  await finish(panel);
  const review = await previewConfiguration(page, "按键映射与宏");
  await expect(review.getByLabel("待应用变更")).toContainText("你好🌈");
  await configurationView(page).getByRole("button", { name: "应用按键映射", exact: true }).click();
  const target = (await writes(page))[0].args.request.configuration.devices[0];
  expect(target.device).toBe("mouse"); expect(target.bindings[0].trigger).toBe(1);
  expect(target.bindings[0].action.before[0].makeDelayMs).toBe(42); expect(target.bindings[0].action.after[0].breakDelayMs).toBe(75);
  await openEditor(page);
  await choose(page, "执行动作", "按键与鼠标宏");
  await panel.getByRole("button", { name: "删除第2步", exact: true }).click();
  await finish(panel);
  await expect(region(page)).toContainText("宏结束前须释放所有按键");
  await expect(availableAction(page, "按键映射与宏")).toHaveCount(0);
});

test("Pause使用驱动E11D而非24位值，触发与录制草稿编辑保持同一编码", async ({ page }) => {
  const panel = await openEditor(page);
  await panel.getByRole("button", { name:"A 禁用按键",exact:true }).click();
  await choose(page,"触发按键","Pause"); await choose(page,"执行动作","按键与鼠标宏");
  await choose(page,"第1步按键","Pause"); await choose(page,"第2步按键","Pause");
  await finish(panel);
  const review = await previewConfiguration(page, "按键映射与宏");
  await expect(review.getByLabel("待应用变更")).toContainText("按下 Pause");
  await review.getByRole("button", { name: "应用按键映射", exact: true }).click();
  const binding = (await writes(page))[0].args.request.configuration.devices[0].bindings[0];
  expect(binding.trigger).toBe(0xe11d); expect(binding.action.steps.map(step => step.scanCode)).toEqual([0xe11d,0xe11d]);
});

test("映射开关直接提交保存的绑定，保留编辑中新宏", async ({ page }) => {
  const panel = await openEditor(page);
  await panel.getByRole("button", { name: "A 禁用按键", exact: true }).click();
  await choose(page, "执行动作", "按键与鼠标宏");
  const enabled = panel.getByRole("checkbox", { name: "启用键盘映射", exact: true });
  await enabled.click();
  await expect(enabled).not.toBeChecked();
  const request = (await writes(page))[0].args.request.configuration.devices;
  expect(request).toHaveLength(1);
  expect(request[0]).toMatchObject({ device: "keyboard", enabled: false, bindings: [{ action: { kind: "disable" } }] });
  await finish(panel);
  await page.evaluate(() => { window.__uiFixture.keymap.revision = "outside"; });
  await reread(page);
  await expect(availableAction(page, "按键映射与宏")).toBeVisible();
  await page.evaluate(() => { window.__uiFixture.keymapReadFault = "隔离映射读取失败"; });
  await reread(page);
  await expect(availableAction(page, "按键映射与宏")).toHaveCount(0);
  await expect(await openDetails(page)).toContainText("隔离映射读取失败");
  await page.keyboard.press("Escape");
  await page.evaluate(() => { window.__uiFixture.keymapReadFault = null; });
  await reread(page);
  await openEditor(page);
  await expect(enabled).not.toBeChecked();
  await expect(panel.getByRole("combobox", { name: "执行动作", exact: true })).toHaveText("按键与鼠标宏");
  expect(await writes(page)).toHaveLength(1);
});

test("直接关闭映射部分失败保留恢复入口", async ({ page }) => {
  const panel = await openEditor(page);
  await page.evaluate(() => { window.__uiFixture.keymapFault = "隔离注册失败，原配置尚需恢复"; });
  await panel.getByRole("checkbox", { name: "启用键盘映射", exact: true }).click();
  await finish(panel);
  await expect(region(page)).toContainText("请先恢复初始配置");
  await expect(availableAction(page, "按键映射与宏")).toHaveCount(0);
  await previewConfiguration(page, "按键映射与宏", true);
  await configurationView(page, true).getByRole("button", { name: "确认恢复", exact: true }).click();
  await openEditor(page);
  await expect(panel.getByRole("checkbox", { name: "启用键盘映射", exact: true })).toBeChecked();
});

test("配置导入只预览完整草稿，导出使用同一领域契约", async ({ page }) => {
  const panel = await openEditor(page), configuration = { devices: [
    { device: "keyboard", enabled: false, bindings: [] }, { device: "mouse", enabled: true, bindings: [] }] };
  await panel.getByLabel("导入映射配置", { exact: true }).setInputFiles({ name: "work.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify({ version:1, name:"工作映射", configuration })) });
  await expect(panel.getByRole("textbox", { name: "映射配置名称" })).toHaveValue("工作映射");
  await expect(panel.getByRole("checkbox", { name: "启用键盘映射", exact: true })).toBeChecked();
  expect(await writes(page)).toHaveLength(0);
  const download = page.waitForEvent("download"); await panel.getByRole("button", { name: "导出映射草稿" }).click();
  expect((await download).suggestedFilename()).toBe("工作映射.keymap.json");
  await panel.getByLabel("导入映射配置", { exact: true }).setInputFiles({ name:"bad.json", mimeType:"application/json", buffer:Buffer.from("bad-json") });
  await expect(panel.getByRole("alert")).toBeVisible();
  await expect(panel.getByRole("textbox", { name: "映射配置名称" })).toHaveValue("工作映射");
  expect(await writes(page)).toHaveLength(0);
});

test("录制结果需显式采用，切页停止且不会应用过滤器", async ({ page }) => {
  const panel = await openEditor(page);
  await panel.getByRole("button", { name: "A 禁用按键", exact: true }).click();
  await choose(page, "执行动作", "按键与鼠标宏");
  await panel.getByRole("button", { name: "录制按键", exact: true }).click();
  await expect(panel).toContainText("正在录制");
  await expect(panel.getByRole("button", { name: "完成编辑" })).toBeDisabled();
  await expect(availableAction(page, "按键映射与宏")).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.keymapCapture = { active:false, reason:"已按Esc停止录制", elapsedMs:1000,
    steps:[{ kind:"key", scanCode:48, pressed:true, delayAfterMs:20 },{ kind:"key", scanCode:48, pressed:false, delayAfterMs:0 }] }; });
  await expect(panel).toContainText("已按Esc停止录制");
  await expect(panel.getByRole("combobox", { name: "第1步按键", exact: true })).toHaveText("A");
  await panel.getByRole("button", { name: "采用录制", exact: true }).click();
  await expect(panel.getByRole("combobox", { name: "第1步按键", exact: true })).toHaveText("B");
  await panel.getByRole("button", { name: "录制按键", exact: true }).click();
  await navigateFromHotkey(page);
  await expect.poll(() => page.evaluate(() => window.__uiFixture.keymapCapture.active)).toBe(false);
  expect(await writes(page)).toHaveLength(0);
});

test("程序完整路径进入确认预览，监听失败可独立重试且保留草稿", async ({ page }) => {
  const panel = await openEditor(page);
  await panel.getByRole("button", { name:"A 禁用按键", exact:true }).click();
  await choose(page, "执行动作", "启动程序");
  await expect(availableAction(page, "按键映射与宏")).toHaveCount(0);
  await panel.getByRole("textbox", { name:"宏程序路径" }).fill("C:\\Apps\\Editor.exe");
  await finish(panel);
  const review = await previewConfiguration(page, "按键映射与宏");
  await expect(review.getByLabel("待应用变更")).toContainText("C:\\Apps\\Editor.exe");
  expect(await writes(page)).toHaveLength(0);
  await review.getByRole("button", { name: "应用按键映射", exact: true }).click();
  await openEditor(page);
  await expect(panel.getByRole("button", { name: "程序宏监听中", exact: true })).toBeDisabled();
  await expect(panel.getByText("程序宏监听已启用", { exact: true })).toHaveCount(0);
  expect((await writes(page))[0].args.request.configuration.devices[0].bindings[0].action).toEqual({ kind:"launch",path:"C:\\Apps\\Editor.exe",eventId:256 });
  await panel.getByRole("textbox", { name:"宏程序路径" }).fill("C:\\Apps\\Draft.exe");
  await finish(panel);
  await page.evaluate(() => { window.__uiFixture.keymap.programs = { running:false,error:"事件日志已清空，监听停止" }; });
  await reread(page);
  await openEditor(page);
  await expect(panel.getByRole("alert")).toContainText("事件日志已清空");
  await panel.getByRole("button", { name:"启用程序宏监听", exact:true }).click();
  await expect(panel.getByRole("textbox", { name:"宏程序路径" })).toHaveValue("C:\\Apps\\Draft.exe");
  await expect(panel.getByRole("button", { name: "程序宏监听中", exact: true })).toBeDisabled();
  expect(await writes(page)).toHaveLength(1);
});

test("录制启动响应晚于离页时仍清理本次捕获", async ({ page }) => {
  const panel = await openEditor(page);
  await panel.getByRole("button", { name:"A 禁用按键", exact:true }).click();
  await choose(page, "执行动作", "按键与鼠标宏");
  await page.evaluate(() => { window.__uiFixture.keymapRecordingDelay = 700; });
  await panel.getByRole("button", { name:"录制按键", exact:true }).click();
  await navigateFromHotkey(page);
  await expect.poll(() => page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "stop_keymap_recording").length)).toBeGreaterThanOrEqual(2);
  await expect.poll(() => page.evaluate(() => window.__uiFixture.keymapCapture?.active)).toBe(false);
  expect(await writes(page)).toHaveLength(0);
});

test("能力关闭隐藏，来源缺失阻止应用，卡片左右布局窄屏回流", async ({ page }) => {
  const panel = await openEditor(page);
  await panel.getByRole("button", { name: "A 禁用按键", exact: true }).click();
  await choose(page, "执行动作", "发送文本"); await panel.getByRole("textbox", { name: "文本宏内容" }).fill("布局检查");
  for (const width of [1440,760,360]) {
    await page.setViewportSize({ width, height:1000 });
    expect(await panel.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  }
  await page.setViewportSize({ width:1440, height:1800 });
  await panel.scrollIntoViewIfNeeded(); await panel.screenshot({ path:"target/ui-check/keymap.png", animations:"disabled" });
  await finish(panel);
  await page.evaluate(() => { window.__uiFixture.keymap.configuration = null; });
  await reread(page);
  await expect(region(page).getByRole("button", { name: "编辑映射", exact: true })).toHaveCount(0);
  await expect(availableAction(page, "按键映射与宏")).toHaveCount(0);
  await page.evaluate(() => { window.__uiFixture.keymap = null; });
  await reread(page);
  await expect(region(page)).toHaveCount(0); expect(await writes(page)).toHaveLength(0);
});
