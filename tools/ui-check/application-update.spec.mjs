import { expect, test } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";

function installUpdateFixture() {
  const ipc = window.__TAURI_INTERNALS__;
  const invoke = ipc.invoke;
  const register = ipc.transformCallback;
  const callbacks = new Map();
  window.__updateFixture = { outcome: "current", checks: 0, starts: [], cancels: [], notes: "### 新增\n\n- 隔离更新说明" };
  const fixture = window.__updateFixture;
  ipc.transformCallback = callback => {
    const id = register(callback);
    callbacks.set(id, callback);
    return id;
  };
  ipc.invoke = async (command, args) => {
    if (command === "check_application_update") {
      fixture.checks++;
      if (fixture.holdCheck) await new Promise(resolve => { fixture.resumeCheck = resolve; });
      if (fixture.outcome === "failed") throw "隔离网络错误";
      return fixture.outcome === "current" ? null : { id: 17, version: "2.0.0", notes: fixture.notes };
    }
    if (command === "download_application_update") {
      fixture.starts.push(args.id);
      const callback = callbacks.get(args.progress.id);
      let index = 0;
      fixture.report = message => callback({ index: index++, message });
      fixture.report({ phase: "downloading", downloadedBytes: 25, totalBytes: 100 });
      return new Promise((resolve, reject) => {
        fixture.finish = result => { callback({ index, end: true }); resolve(result); };
        fixture.fail = () => { callback({ index, end: true }); reject("隔离签名校验失败"); };
      });
    }
    if (command === "cancel_application_update") {
      fixture.cancels.push(args.id);
      fixture.finish(false);
      return true;
    }
    return invoke(command, args);
  };
}

const sidebar = page => page.getByRole("navigation", { name: "主导航" });
const updateEntry = (page, name) => sidebar(page).getByRole("button", { name, exact: true });
const updateDialog = page => page.getByRole("dialog", { name: "软件更新" });
const releaseDialog = page => page.getByRole("dialog", { name: "发现新版本" });

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: `${MOCK_IPC}\n(${installUpdateFixture.toString()})();` });
  await page.goto("/");
  await expect(page.getByRole("main", { name: "概览" })).toBeVisible();
});

test("侧栏按需检查，在原页显示结果并可查看失败原因", async ({ page }) => {
  expect(await page.evaluate(() => window.__updateFixture.checks)).toBe(0);
  await page.evaluate(() => { window.__updateFixture.holdCheck = true; });
  await updateEntry(page, "检查更新").click();
  const checking = updateEntry(page, "正在检查更新");
  await expect(checking).toBeDisabled();
  await checking.evaluate(button => button.click());
  expect(await page.evaluate(() => window.__updateFixture.checks)).toBe(1);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.evaluate(() => {
    window.__updateFixture.holdCheck = false;
    window.__updateFixture.resumeCheck();
  });
  const current = updateEntry(page, "已是最新版本，重新检查");
  await expect(current).toBeVisible();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByRole("main", { name: "概览" })).toBeVisible();

  await page.evaluate(() => { window.__updateFixture.outcome = "failed"; });
  await current.click();
  await updateEntry(page, "检查更新失败").click();
  const dialog = updateDialog(page);
  await expect(dialog.getByText("当前版本 v1.0.1", { exact: true })).toBeVisible();
  await expect(dialog.getByRole("alert")).toHaveText("隔离网络错误");
  await expect(dialog.getByRole("button", { name: "GitHub 发布页" })).toBeVisible();
  await page.evaluate(() => { window.__updateFixture.outcome = "current"; });
  await dialog.getByRole("button", { name: "重新检查" }).click();
  await expect(dialog.getByRole("status")).toHaveText("当前已是最新版本。");
  await expect(dialog.getByRole("button", { name: "确定", exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(current).toBeFocused();
  await openWorkspace(page, "设置");
  await page.getByRole("region", { name: "关于" }).getByRole("button", { name: "检查更新" }).click();
  await expect(dialog.getByRole("status")).toHaveText("当前已是最新版本。");
});

test("下载跨页面继续，两个入口共享进度、取消、重试与向导状态", async ({ page }) => {
  await page.evaluate(() => { window.__updateFixture.outcome = "available"; });
  await updateEntry(page, "检查更新").click();
  const release = releaseDialog(page);
  await expect(release).toBeVisible();
  await expect(release.getByText("v2.0.0", { exact: true })).toBeVisible();
  await expect(release.getByRole("article", { name: "新版本更新说明" })).toContainText("隔离更新说明");
  await expect(release.getByRole("button")).toHaveText(["取消", "更新"]);
  expect(await page.evaluate(() => window.__updateFixture.starts)).toEqual([]);
  await release.getByRole("button", { name: "更新", exact: true }).click();
  await expect(release).toBeHidden();
  const dialog = updateDialog(page);
  await expect(dialog).toBeHidden();
  const downloading = updateEntry(page, "正在下载 25%");
  await expect(downloading).toBeFocused();
  await openWorkspace(page, "设备配置");
  await expect(downloading).toBeVisible();
  await openWorkspace(page, "设置");
  const about = page.getByRole("region", { name: "关于" });
  await about.getByRole("button", { name: "查看更新状态" }).click();
  await expect(dialog.getByRole("progressbar", { name: "下载进度" })).toHaveAttribute("aria-valuenow", "25");
  await dialog.getByRole("button", { name: "取消下载" }).click();
  await expect(dialog.getByRole("status")).toHaveText("更新已取消。");
  const retryDownload = dialog.getByRole("button", { name: "重新下载", exact: true });
  await retryDownload.click();
  await page.evaluate(() => window.__updateFixture.fail());
  await expect(dialog.getByRole("alert")).toHaveText("隔离签名校验失败");
  await retryDownload.click();
  await page.evaluate(() => window.__updateFixture.report({ phase: "downloading", downloadedBytes: 25, totalBytes: null }));
  await expect(dialog.getByRole("progressbar", { name: "下载进度" })).not.toHaveAttribute("aria-valuenow");
  await page.evaluate(() => window.__updateFixture.report({ phase: "downloading", downloadedBytes: 0, totalBytes: 0 }));
  await expect(dialog.getByRole("progressbar", { name: "下载进度" })).not.toHaveAttribute("aria-valuemax");
  await page.evaluate(() => window.__updateFixture.report({ phase: "downloading", downloadedBytes: 120, totalBytes: 100 }));
  await expect(dialog.getByRole("progressbar", { name: "下载进度" })).toHaveAttribute("aria-valuenow", "100");
  await page.evaluate(() => window.__updateFixture.report({ phase: "verifying" }));
  await expect(dialog.getByRole("status")).toHaveText("正在校验安装包签名…");
  await expect(dialog.getByRole("status").locator('[data-icon="loading"]')).toHaveCount(1);
  await page.evaluate(() => window.__updateFixture.report({ phase: "installerOpen" }));
  await expect(dialog.getByRole("status")).toHaveText("请在安装向导中确认升级。");
  await expect(dialog.getByRole("button", { name: "取消下载" })).toHaveCount(0);
  await page.evaluate(() => window.__updateFixture.finish(true));
  await expect(dialog.getByRole("status")).toHaveText("安装向导已完成。");
  expect(await page.evaluate(() => {
    const { checks, starts, cancels } = window.__updateFixture;
    return { checks, starts, cancels };
  })).toEqual({ checks: 1, starts: [17, 17, 17], cancels: [17] });
});

test("折叠侧栏可通过键盘打开和重看新版本，取消后恢复焦点", async ({ page }) => {
  await page.setViewportSize({ width: 700, height: 220 });
  const trigger = updateEntry(page, "检查更新");
  await expect(trigger).toBeInViewport();
  await expect(updateEntry(page, "设置")).toBeInViewport();
  await page.evaluate(() => { Object.assign(window.__updateFixture, { outcome: "available", notes: null }); });
  await trigger.focus();
  await expect(page.getByRole("tooltip")).toHaveText("检查更新");
  await page.keyboard.press("Enter");
  const release = releaseDialog(page);
  await expect(release.getByRole("heading", { name: "发现新版本" })).toBeFocused();
  await expect(release.getByText("暂无更新说明。")).toBeVisible();
  await expect(release.getByRole("button", { name: "更新", exact: true })).toBeInViewport();
  await page.keyboard.press("Escape");
  await expect(release).toBeHidden();
  const available = updateEntry(page, "发现新版本 v2.0.0");
  await expect(available).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(release).toBeVisible();
  await release.getByRole("button", { name: "取消", exact: true }).click();
  await expect(release).toBeHidden();
  expect(await page.evaluate(() => ({ checks: window.__updateFixture.checks, starts: window.__updateFixture.starts })))
    .toEqual({ checks: 1, starts: [] });
});

test("关于在检查完成后显示新版本，长说明滚动时两个操作留在底部", async ({ page }) => {
  await page.setViewportSize({ width: 380, height: 400 });
  await openWorkspace(page, "设置");
  const about = page.getByRole("region", { name: "关于", exact: true });
  await page.evaluate(() => {
    Object.assign(window.__updateFixture, {
      holdCheck: true,
      outcome: "available",
      notes: ["### 新功能", "", ...Array.from({ length: 15 }, (_, i) => `- 第 ${i + 1} 条更新说明`), "",
        "### 问题修复", "", "- 最后一条修复说明"].join("\n"),
    });
  });
  await about.getByRole("button", { name: "检查更新", exact: true }).click();
  await expect(about.getByRole("button", { name: "正在检查更新", exact: true })).toBeDisabled();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.evaluate(() => window.__updateFixture.resumeCheck());

  const release = releaseDialog(page);
  const cancel = release.getByRole("button", { name: "取消", exact: true });
  const update = release.getByRole("button", { name: "更新", exact: true });
  const notes = release.getByRole("article", { name: "新版本更新说明" });
  await expect(notes.getByRole("heading", { name: "新功能" })).toBeInViewport();
  const scroll = release.locator(".ccc-scroll-viewport");
  const geometry = await scroll.evaluate(node => ({
    vertical: node.scrollHeight > node.clientHeight, horizontal: node.scrollWidth > node.clientWidth,
  }));
  expect(geometry).toEqual({ vertical: true, horizontal: false });
  const buttonPosition = await update.boundingBox();
  const cancelPosition = await cancel.boundingBox();
  expect(cancelPosition.y).toBe(buttonPosition.y);
  expect(cancelPosition.width).toBeCloseTo(buttonPosition.width);
  await scroll.evaluate(node => { node.scrollTop = node.scrollHeight; });
  await expect(notes.getByText("最后一条修复说明")).toBeInViewport();
  await expect(update).toBeInViewport();
  expect(await update.boundingBox()).toEqual(buttonPosition);

  await page.mouse.click(1, 1);
  await expect(release).toBeHidden();
  const resumed = about.getByRole("button", { name: "查看更新状态", exact: true });
  await expect(resumed).toBeFocused();
  await resumed.click();
  await expect(release).toBeVisible();
  expect(await page.evaluate(() => window.__updateFixture.checks)).toBe(1);
});
