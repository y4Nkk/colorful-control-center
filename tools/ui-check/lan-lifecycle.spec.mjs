import { openWorkspace, openSystemArea } from "./workspace.mjs";
import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
const panel = page => page.getByRole("region", { name: "网络驱动策略", exact: true });
const navigate = (page, name) => page.getByRole("navigation", { name: "主导航" }).getByRole("button", { name, exact: true }).click();
async function install(page, mode) {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(mode => {
    const data = window.__uiFixture;
    const string = text => ({ kind: 1, bytes: [...text, "\0"].flatMap(letter => [letter.charCodeAt(0), 0]) });
    data.lan = { revision: "lan-0", initial: [], lastError: null, blockedReason: null, frame: {
      capabilities: { powerSaving: true, wakePatch: false }, recommendation: null, devices: [{
        identity: { instance: "PCI\\ISOLATED\\1", driver: "{4d36e972-e325-11ce-bfc1-08002be10318}\\0001", netCfgId: string("id"), driverVersion: string("1"), matchingId: null },
        name: "旧读取网卡", values: { powerSavingMode: string("0"), pnpCapabilities: null, wakeOnLink: null, s5WakeOnLan: null, shutdownWake: null, wakeOnSlot: null },
      }] } };
    data.pendingLanReads = []; data.lanReadCount = 0; data.deferAfterWrite = false;
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      const result = await invoke(command, args);
      if (command === "apply_lan_policy" && mode === "after-write") data.deferAfterWrite = true;
      if (command === "read_lan_policy") {
        ++data.lanReadCount;
        if (mode === "reads" && data.lanReadCount === 1 || data.deferAfterWrite) {
          return new Promise(resolve => data.pendingLanReads.push(() => resolve(result)));
        }
      }
      return result;
    };
  }, mode);
  await page.goto("/"); await openWorkspace(page, "电源");
}

test("严格模式及返回页面允许新读取，过时响应不能阻塞或覆盖新网卡快照", async ({ page }) => {
  await install(page, "reads");
  await expect.poll(() => page.evaluate(() => window.__uiFixture.pendingLanReads.length)).toBe(1);
  await openSystemArea(page, "配置档案");
  await page.evaluate(() => window.__uiFixture.lan.frame.devices[0].name = "新读取网卡");
  await navigate(page, "电源");
  await expect(panel(page)).toContainText("新读取网卡");
  await page.evaluate(() => window.__uiFixture.pendingLanReads.splice(0).forEach(resolve => resolve()));
  await expect(panel(page)).toContainText("新读取网卡");
  await expect(panel(page)).not.toContainText("旧读取网卡");
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "apply_lan_policy"))).toEqual([]);
});

test("写入回读期间切页返回仍发布确认后的状态", async ({ page }) => {
  await install(page, "after-write");
  await expect(panel(page)).toContainText("旧读取网卡");
  await panel(page).getByRole("combobox", { name: "网卡节能", exact: true }).click();
  await page.getByRole("option", { name: "开启网卡节能", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__uiFixture.pendingLanReads.length)).toBe(1);
  await openSystemArea(page, "配置档案"); await navigate(page, "电源");
  await page.evaluate(() => {
    window.__uiFixture.deferAfterWrite = false;
    window.__uiFixture.pendingLanReads.splice(0).forEach(resolve => resolve());
  });
  await expect(panel(page).getByRole("combobox", { name: "网卡节能", exact: true })).toHaveText("开启网卡节能");
  await expect(panel(page).getByRole("alert")).toHaveCount(0);
  expect(await page.evaluate(() => window.__uiFixture.calls.filter(call => call.command === "apply_lan_policy").length)).toBe(1);
});
