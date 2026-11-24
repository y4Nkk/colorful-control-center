import { execFile, spawn } from "node:child_process";
import { once } from "node:events";
import { copyFile, mkdir, writeFile } from "node:fs/promises";
import { createWriteStream } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { setTimeout as delay } from "node:timers/promises";
import { chromium, expect } from "@playwright/test";
import { openWorkspace, openPowerArea } from "./workspace.mjs";
import { pageActions } from "./configuration-review.mjs";
import { PROFILE_REFRESH_IPC, prepareProfileRefresh, refreshCases } from "./profile-refresh-cases.mjs";

const root = fileURLToPath(new URL("../..", import.meta.url));
const output = `${root}/target/ui-check/native-${Date.now()}`;
await mkdir(output, { recursive: true });
const checks = [], processes = [];
let browser;

function start(command, args, name) {
  const child = spawn(command, args, { cwd: root, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
  const log = createWriteStream(`${output}/${name}.log`);
  child.stdout.pipe(log); child.stderr.pipe(log, { end: false });
  child.on("error", error => log.write(String(error)));
  processes.push(child);
  return child;
}
async function waitUrl(url, child) {
  for (let i = 0; i < 120; i++) {
    if (child.exitCode !== null) throw new Error(`${url} 的宿主已退出 (${child.exitCode})，诊断见 ${output}`);
    try { const response = await fetch(url); if (response.ok) return; } catch { /* 只等待本次子进程的本地服务就绪。 */ }
    await delay(250);
  }
  throw new Error(`${url} 未就绪，诊断见 ${output}`);
}
async function packageLoader() {
  const { stdout } = await promisify(execFile)("rustc", ["-vV"], { cwd: root, windowsHide: true });
  const target = /^host: (.+)$/m.exec(stdout)?.[1].trim();
  if (!target) throw new Error("无法识别原生 Rust 工具链目标");
  const metadata = start("cargo", ["metadata", "--offline", "--locked", "--filter-platform", target,
    "--format-version", "1", "--manifest-path", "tools/ui-webview2/Cargo.toml"], "metadata");
  let json = "";
  metadata.stdout.on("data", data => { json += data; });
  const [code] = await once(metadata, "exit");
  if (code !== 0) throw new Error(`无法解析原生宿主依赖：${output}/metadata.log`);
  const packages = JSON.parse(json).packages.filter(pkg => pkg.name === "webview2-com-sys");
  if (packages.length !== 1) throw new Error("原生验收宿主必须有唯一 WebView2 Loader 依赖");
  const arch = { x64: "x64", ia32: "x86", arm64: "arm64" }[process.arch];
  if (!arch) throw new Error(`不支持的原生验收架构：${process.arch}`);
  await copyFile(join(dirname(packages[0].manifest_path), arch, "WebView2Loader.dll"),
    `${root}/target/ui-webview2-build/debug/WebView2Loader.dll`);
}
async function record(name, action) { await action(); checks.push(name); console.log(`ok: ${name}`); }
async function fixtureChecks(page) {
  await page.goto("http://127.0.0.1:1450/tools/ui-check/index.html");
  const mode = page.getByRole("combobox", { name: "选择模式", exact: true });
  await record("原生 WebView2 普通菜单键盘与焦点", async () => {
    await mode.press("ArrowDown"); await mode.press("End"); await mode.press("Enter");
    await expect(page.getByLabel("已选模式")).toHaveText("d"); await expect(mode).toBeFocused();
  });
  await record("原生 WebView2 搜索选择与退出", async () => {
    const field = page.getByRole("combobox", { name: "选择字体", exact: true });
    await field.click(); const search = page.getByRole("searchbox", { name: "搜索字体" });
    await expect(search).toBeFocused(); await search.fill("Mono"); await search.press("Enter");
    await expect(page.getByLabel("已选字体")).toHaveText("Monospace"); await expect(field).toBeFocused();
  });
  for (const scale of [0.9, 1, 1.1, 1.25]) await record(`原生 WebView2 ${scale} 缩放与边缘避让`, async () => {
    await page.evaluate(scale => {
      document.documentElement.style.zoom = String(scale);
      const area = document.getElementById("anchor-area");
      Object.assign(area.style, { position: "fixed", zIndex: "10", left: `${(innerWidth - 170) / scale}px`, top: `${(innerHeight - 70) / scale}px`, width: `${150 / scale}px` });
    }, scale);
    await mode.click(); const menu = page.locator("body > .ccc-select-menu");
    await expect(menu).toHaveAttribute("data-side", "top");
    const box = await menu.boundingBox(), viewport = await page.evaluate(() => ({ width: innerWidth, height: innerHeight }));
    expect(box.x).toBeGreaterThanOrEqual(0); expect(box.x + box.width).toBeLessThanOrEqual(viewport.width + 1);
    expect(box.y + box.height).toBeLessThanOrEqual(viewport.height); await mode.press("Escape");
  });
  await page.evaluate(() => { document.documentElement.style.zoom = "1"; document.documentElement.dataset.theme = "dark"; });
  await mode.click(); await page.screenshot({ path: `${output}/native-dark.png`, animations: "disabled", timeout: 15000 });
}
async function nativeZoomChecks(page) {
  await page.keyboard.press("Escape");
  const width = await page.evaluate(() => innerWidth);
  for (const scale of [0.9, 1, 1.1, 1.25]) await record(`WebView2 ZoomFactor ${scale} 原生缩放`, async () => {
    await page.evaluate(value => window.ipc.postMessage(String(value)), scale);
    await page.waitForFunction(({ scale, width }) => window.__nativeScale === scale && Math.abs(innerWidth - width / scale) <= 1, { scale, width });
    await page.evaluate(() => {
      Object.assign(document.getElementById("anchor-area").style, { left: `${innerWidth - 170}px`, top: `${innerHeight - 70}px`, width: "150px" });
    });
    const field = page.getByRole("combobox", { name: "选择模式", exact: true });
    await field.click();
    const menu = page.locator("body > .ccc-select-menu");
    await expect(menu).toHaveAttribute("data-side", "top");
    const metrics = await menu.evaluate(el => {
      const rect = el.getBoundingClientRect();
      return { left: rect.left, right: rect.right, bottom: rect.bottom, width: innerWidth, height: innerHeight };
    });
    expect(metrics.left).toBeGreaterThanOrEqual(0); expect(metrics.right).toBeLessThanOrEqual(metrics.width);
    expect(metrics.bottom).toBeLessThanOrEqual(metrics.height);
    await field.press("Escape"); await expect(field).toBeFocused();
  });
  await page.evaluate(() => window.ipc.postMessage("1"));
  await page.waitForFunction(() => window.__nativeScale === 1);
}
async function appChecks(page) {
  await page.context().addInitScript({ content: PROFILE_REFRESH_IPC });
  await page.goto("http://127.0.0.1:1450/");
  await record("原生 WebView2 工作区导航与只读浏览", async () => {
    for (const title of ["概览", "性能与散热", "键盘与灯光", "电源与档案", "设备配置", "设置"]) await openWorkspace(page, title);
    await openPowerArea(page, "配置档案");
    expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|restore_)/.test(call.command)))).toEqual([]);
  });
  await record("原生 WebView2 分类草稿和滚动保活", async () => {
    await openWorkspace(page, "键盘与灯光");
    await page.getByRole("switch", { name: "启用闲置关灯", exact: true }).check();
    await page.getByRole("spinbutton", { name: "闲置等待分钟", exact: true }).fill("2");
    await page.getByRole("spinbutton", { name: "闲置等待秒数", exact: true }).fill("5");
    const viewport = page.locator(".ccc-workspace > .ccc-scroll-viewport");
    await viewport.evaluate(element => { element.scrollTop = 120; });
    await expect.poll(() => viewport.evaluate(element => element.scrollTop)).toBe(120);
    await delay(100);
    await openWorkspace(page, "性能与散热");
    await expect.poll(() => viewport.evaluate(element => element.scrollTop)).toBe(0);
    await openWorkspace(page, "键盘与灯光");
    await expect(page.getByRole("spinbutton", { name: "闲置等待分钟", exact: true })).toHaveValue("2");
    await expect(page.getByRole("spinbutton", { name: "闲置等待秒数", exact: true })).toHaveValue("5");
    await expect.poll(() => viewport.evaluate(element => element.scrollTop)).toBe(120);
  });
  await record("原生 WebView2 显示页下拉直接试用", async () => {
    await openWorkspace(page, "设备配置");
    await page.getByRole("combobox", { name: "目标刷新率", exact: true }).click();
    await page.getByRole("option", { name: "144 Hz", exact: true }).click();
    const screen = page.getByRole("region", { name: "屏幕显示", exact: true });
    await expect(screen.getByRole("status")).toContainText("试用 144 Hz");
    await expect(pageActions(page)).toHaveCount(0);
    await screen.getByRole("button", { name: "取消本次试用", exact: true }).click();
    expect(await page.evaluate(() => window.__uiFixture.calls.filter(({ command }) => /^(apply_|restore_|set_fan)/.test(command))
      .map(call => call.command))).toEqual(["apply_display_refresh"]);
  });
  await record("原生 WebView2 设置搜索下拉接线", async () => {
    await page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name: "设置", exact: true }).click();
    await page.getByRole("combobox", { name: "选择界面字体", exact: true }).click();
    await page.getByRole("searchbox", { name: "搜索字体" }).fill("Consolas");
    await expect(page.getByRole("option", { name: /Consolas/ })).toBeVisible();
    await page.keyboard.press("Escape"); expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
  });
  await record("原生 WebView2 自启失败保持权限未知", async () => {
    const panel = page.getByRole("region", { name: "登录自启", exact: true });
    await panel.getByRole("combobox", { name: "登录自启方式" }).click();
    await page.getByRole("option", { name: "管理员登录启动", exact: true }).click();
    await page.evaluate(() => { window.__uiFixture.autostartFault = "原生隔离授权取消"; });
    await panel.getByRole("button", { name: "保存 · 管理员登录启动", exact: true }).click();
    await panel.getByRole("button", { name: "确认 · 管理员登录启动", exact: true }).click();
    await expect(panel.getByRole("alert")).toContainText("原生隔离授权取消");
    await expect(panel).toContainText("本软件当前的权限 · 未知");
  });
  await page.screenshot({ path: `${output}/native-settings.png`, animations: "disabled", timeout: 15000 });
}

try {
  const build = start("cargo", ["build", "--offline", "--locked", "--manifest-path", "tools/ui-webview2/Cargo.toml", "--target-dir", "target/ui-webview2-build"], "build");
  const [code] = await once(build, "exit"); if (code !== 0) throw new Error(`原生宿主编译失败：${output}/build.log`);
  await packageLoader();
  const server = start(process.execPath, ["tools/ui-check/server.mjs"], "server");
  await waitUrl("http://127.0.0.1:1450/tools/ui-check/index.html", server);
  const host = start(`${root}/target/ui-webview2-build/debug/ccc-ui-webview2.exe`, [`${output}/webview-data`], "webview2");
  await waitUrl("http://127.0.0.1:9450/json/version", host);
  browser = await chromium.connectOverCDP("http://127.0.0.1:9450");
  const page = browser.contexts()[0].pages()[0];
  await expect(page).toHaveURL("http://127.0.0.1:1450/tools/ui-check/index.html", { timeout: 30000 });
  const errors = []; page.on("pageerror", error => errors.push(String(error)));
  await fixtureChecks(page); await nativeZoomChecks(page); await appChecks(page);
  for (const item of refreshCases) await record(`原生 WebView2 ${item.name}`, async () => {
    await prepareProfileRefresh(page, "http://127.0.0.1:1450/"); await item.run(page);
  });
  expect(errors).toEqual([]);
  await writeFile(`${output}/result.json`, JSON.stringify({ checks, errors, userAgent: await page.evaluate(() => navigator.userAgent) }, null, 2));
  console.log(`passed: ${checks.length}; evidence: ${output}`);
} catch (error) {
  await writeFile(`${output}/failure.json`, JSON.stringify({ checks, error: String(error) }, null, 2));
  throw error;
} finally {
  if (browser) await browser.close().catch(error => console.log(`CDP 关闭：${error}`));
  for (const child of processes.reverse()) if (child.pid && child.exitCode === null && child.signalCode === null) {
    child.kill(); await once(child, "exit");
  }
}
