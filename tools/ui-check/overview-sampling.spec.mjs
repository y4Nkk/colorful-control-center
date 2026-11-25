import { expect, test } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";

const memory = page => page.getByLabel("内存已用容量");
const storage = page => page.getByRole("region", { name: "本地存储概览", exact: true });
const overviewCalls = page => page.evaluate(() => {
  const calls = window.__uiFixture.calls;
  return {
    memory: calls.filter(({ command }) => command === "read_memory_capacity").length,
    inventory: calls.filter(({ command }) => command === "read_system_inventory")
      .map(({ args }) => args.refreshVolumeMetadata),
  };
});

function installSamplingFixture() {
  const state = { label: "隔离原卷", failMemory: false, failStorage: false };
  window.__samplingFixture = state;
  let visible = true;
  Object.defineProperty(document, "visibilityState", { get: () => visible ? "visible" : "hidden" });
  window.__setOverviewVisibility = value => {
    visible = value;
    document.dispatchEvent(new Event("visibilitychange"));
  };
  const invoke = window.__TAURI_INTERNALS__.invoke;
  window.__TAURI_INTERNALS__.invoke = async (command, args) => {
    const reply = await invoke(command, args);
    if (command === "read_memory_capacity" && state.failMemory) throw new Error("隔离内存读取失败");
    if (command === "read_system_inventory") {
      const volumes = state.failStorage ? null : [{ mount: "C:", label: state.label,
        fileSystem: "NTFS", capacity: { totalBytes: 4 * 1024 ** 3, availableBytes: 3 * 1024 ** 3 } }];
      if (state.holdInventory) {
        state.holdInventory = false;
        await new Promise(resolve => { state.resumeInventory = resolve; });
      }
      return { ...reply, volumes };
    }
    return reply;
  };
}

test.beforeEach(async ({ page }) => {
  await page.clock.install();
  await page.addInitScript({ content: `${MOCK_IPC}; (${installSamplingFixture})();` });
  await page.goto("/");
  await expect(memory(page)).toHaveText("8 GB");
  await expect(storage(page)).toContainText("隔离原卷");
});

test("内存沿用五秒采样，固定卷单独刷新，两个读取失败互不覆盖", async ({ page }) => {
  const initial = await overviewCalls(page);
  expect(initial.inventory.at(-1)).toBe(true);
  await page.evaluate(() => Object.assign(window.__samplingFixture, { failMemory: true, failStorage: true }));
  await page.clock.fastForward(5_100);
  await expect(memory(page)).toHaveText("未知");
  await expect(storage(page)).toContainText("隔离原卷");
  expect(await overviewCalls(page)).toEqual({ ...initial, memory: initial.memory + 1 });

  await page.evaluate(() => { window.__samplingFixture.failMemory = false; });
  await page.clock.fastForward(25_100);
  await expect(memory(page)).toHaveText("8 GB");
  await expect(storage(page)).toContainText("容量未读取");
  expect((await overviewCalls(page)).inventory).toEqual([...initial.inventory, false]);

  await page.evaluate(() => Object.assign(window.__samplingFixture, { failStorage: false, label: "隔离新卷" }));
  await page.clock.fastForward(30_100);
  await expect(storage(page)).toContainText("隔离新卷");
  await expect(memory(page)).toHaveText("8 GB");
});

test("页面隐藏暂停两个读取，恢复时立即重读并要求刷新卷元数据", async ({ page }) => {
  const initial = await overviewCalls(page);
  await page.evaluate(() => window.__setOverviewVisibility(false));
  await page.clock.runFor(31_000);
  expect(await overviewCalls(page)).toEqual(initial);

  await page.evaluate(() => {
    window.__samplingFixture.label = "恢复后卷标";
    window.__setOverviewVisibility(true);
  });
  await expect(storage(page)).toContainText("恢复后卷标");
  expect(await overviewCalls(page)).toEqual({ memory: initial.memory + 1, inventory: [...initial.inventory, true] });
});

test("恢复后等待在途卷读取结束并丢弃上一可见周期的结果", async ({ page }) => {
  const initial = (await overviewCalls(page)).inventory;
  await page.evaluate(() => Object.assign(window.__samplingFixture, { label: "仍在读取的旧卷标", holdInventory: true }));
  await page.clock.fastForward(30_100);
  await expect.poll(async () => (await overviewCalls(page)).inventory).toEqual([...initial, false]);

  await page.evaluate(() => window.__setOverviewVisibility(false));
  await page.clock.runFor(100);
  await page.evaluate(() => {
    window.__samplingFixture.label = "重入后的新卷标";
    window.__setOverviewVisibility(true);
  });
  await page.clock.runFor(100);
  expect((await overviewCalls(page)).inventory).toEqual([...initial, false]);
  await expect(storage(page)).toContainText("隔离原卷");

  await page.evaluate(() => window.__samplingFixture.resumeInventory());
  await expect(storage(page)).toContainText("重入后的新卷标");
  expect((await overviewCalls(page)).inventory).toEqual([...initial, false, true]);
});
