import { expect, test } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";

function installUpdateFixture() {
  const ipc = window.__TAURI_INTERNALS__;
  const invoke = ipc.invoke;
  const register = ipc.transformCallback;
  const callbacks = new Map();
  window.__updateFixture = { outcome: "current", checks: 0, starts: [], cancels: [] };
  const fixture = window.__updateFixture;
  ipc.transformCallback = callback => {
    const id = register(callback);
    callbacks.set(id, callback);
    return id;
  };
  ipc.invoke = async (command, args) => {
    if (command === "check_application_update") {
      fixture.checks++;
      if (fixture.outcome === "failed") throw "隔离网络错误";
      return fixture.outcome === "current" ? null : { id: 17, version: "2.0.0", notes: "### 新增\n\n- 隔离更新说明" };
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

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: `${MOCK_IPC}\n(${installUpdateFixture.toString()})();` });
  await page.goto("/");
  await openWorkspace(page, "设置");
});

test("软件更新显示本机版本、检查失败和重新检查结果", async ({ page }) => {
  await page.evaluate(() => { window.__updateFixture.outcome = "failed"; });
  await page.getByRole("button", { name: "检查更新", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "软件更新" });
  await expect(dialog.getByText("当前版本 v1.0.1")).toBeVisible();
  await expect(dialog.getByRole("alert")).toHaveText("隔离网络错误");
  await expect(dialog.getByRole("button", { name: "GitHub 发布页" })).toBeVisible();
  await page.evaluate(() => { window.__updateFixture.outcome = "current"; });
  await dialog.getByRole("button", { name: "重新检查" }).click();
  await expect(dialog.getByRole("status")).toHaveText("当前已是最新版本。");
  await expect(dialog.getByRole("button", { name: "下载并打开安装向导" })).toHaveCount(0);
});

test("下载显示进度，取消或签名失败后可重试并打开向导", async ({ page }) => {
  await page.evaluate(() => { window.__updateFixture.outcome = "available"; });
  await page.getByRole("button", { name: "检查更新", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "软件更新" });
  await expect(dialog.getByText("新版本 v2.0.0")).toBeVisible();
  await expect(dialog.getByRole("article", { name: "新版本更新说明" })).toContainText("隔离更新说明");
  expect(await page.evaluate(() => window.__updateFixture.starts)).toEqual([]);
  const download = dialog.getByRole("button", { name: "下载并打开安装向导" });
  await download.click();
  await expect(dialog.getByRole("progressbar", { name: "下载进度" })).toHaveAttribute("value", "25");
  await dialog.getByRole("button", { name: "取消下载" }).click();
  await expect(dialog.getByRole("status")).toHaveText("更新已取消。");
  await download.click();
  await page.evaluate(() => window.__updateFixture.fail());
  await expect(dialog.getByRole("alert")).toHaveText("隔离签名校验失败");
  await dialog.getByRole("button", { name: "重新下载" }).click();
  await page.evaluate(() => window.__updateFixture.report({ phase: "verifying" }));
  await expect(dialog.getByRole("status")).toHaveText("正在校验安装包签名…");
  await page.evaluate(() => window.__updateFixture.report({ phase: "installerOpen" }));
  await expect(dialog.getByRole("status")).toHaveText("请在安装向导中确认升级。");
  await expect(dialog.getByRole("button", { name: "取消下载" })).toHaveCount(0);
  await page.evaluate(() => window.__updateFixture.finish(true));
  await expect(dialog.getByRole("status")).toHaveText("安装向导已完成。");
  expect(await page.evaluate(() => ({ starts: window.__updateFixture.starts, cancels: window.__updateFixture.cancels })))
    .toEqual({ starts: [17, 17, 17], cancels: [17] });
});
