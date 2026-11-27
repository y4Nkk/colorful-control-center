import { expect, test } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace } from "./workspace.mjs";

async function fontInspector(page) {
  const client = await page.context().newCDPSession(page);
  await client.send("DOM.enable");
  await client.send("CSS.enable");
  const { root } = await client.send("DOM.getDocument");

  return async (selector, familyName, weight) => {
    const target = page.locator(selector).first();
    await expect(target).toBeVisible();
    if (weight) await expect(target).toHaveCSS("font-weight", weight);
    await page.evaluate(() => document.fonts.ready);
    const { nodeId } = await client.send("DOM.querySelector", { nodeId: root.nodeId, selector });
    await expect.poll(async () => {
      const { fonts } = await client.send("CSS.getPlatformFontsForNode", { nodeId });
      return fonts.filter(font => font.glyphCount > 0);
    }, { message: selector }).toContainEqual(expect.objectContaining({ familyName, isCustomFont: true }));
  };
}

test("主界面和硬件图表实际加载随应用分发的字体", async ({ page }) => {
  await page.addInitScript({ content: MOCK_IPC });
  await page.goto("/");
  const expectFont = await fontInspector(page);
  for (const selector of [".ccc-hardware-description h3", ".ccc-hardware-description p"])
    await expectFont(selector, "WenQuanYi Zen Hei");
  for (const selector of ["output[aria-label='CPU 当前温度']", "output[aria-label='活跃频率']",
    "output[aria-label='当前频率']", "[aria-label='当前内存使用率'] output"])
    await expectFont(selector, "Google Sans Code", "600");
  await expectFont(".ccc-trend-tick", "Google Sans Code", "400");

  await openWorkspace(page, "性能与散热");
  await expectFont(".ccc-fan-dial-value", "Google Sans Code", "600");
  await expectFont(".ccc-fan-dial-scale", "Google Sans Code", "400");

  await openWorkspace(page, "设置");
  const picker = page.getByRole("combobox", { name: "选择界面字体", exact: true });
  await expect(picker).toContainText("默认（文泉驿正黑）");
  await picker.click();
  await page.getByRole("option", { name: /^Arial/ }).click();
  await expect(picker).toHaveCSS("font-family", /^(?:Arial|"Arial"),/);
  await picker.click();
  await page.getByRole("option", { name: /^默认（文泉驿正黑）/ }).click();
  await expect(picker).toHaveCSS("font-family", /^"WenQuanYi Zen Hei"/);
});

test("屏显从同一应用字体资源加载中英文", async ({ page }) => {
  await page.addInitScript({ content: `${MOCK_IPC}; window.__uiFixture.osd = {
    revision: 1, visible: true, message: { title: "屏幕亮度", detail: "Display brightness" },
    appearance: window.__uiFixture.preferences
  };` });
  await page.goto("/?surface=osd");
  const expectFont = await fontInspector(page);
  await expectFont(".ccc-osd-content strong", "WenQuanYi Zen Hei");
  await expectFont(".ccc-osd-content p", "WenQuanYi Zen Hei");
});
