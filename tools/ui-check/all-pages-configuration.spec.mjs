import { test, expect } from "@playwright/test";
import { MOCK_IPC } from "./mock-ipc.mjs";
import { openWorkspace, openSystemArea } from "./workspace.mjs";
import { supplyModeFixture } from "./supply-mode-fixture.mjs";
import { installConfigurationFixture } from "./configuration-fixture.mjs";
import { previewConfiguration } from "./configuration-review.mjs";
import { colorButton, setPickerColor } from "./color-picker.mjs";
const views = [
  ["系统", "概览"], ["系统", "风扇散热"], ["系统", "性能调校"], ["系统", "配置档案"],
  ["键盘与灯光"], ["电源"], ["设备配置"], ["设置"],
];
async function setup(page) {
  await page.addInitScript({ content: MOCK_IPC });
  await page.addInitScript(installConfigurationFixture, supplyModeFixture());
  await page.goto("/");
}
const writes = page => page.evaluate(() => window.__uiFixture.calls.filter(call => /^(apply_|set_|enable_|restore_|save_|start_fan|start_battery)/.test(call.command) && call.command !== "set_shortcut_recording"));
async function ready(page, name) {
  if (name === "风扇散热") await expect(page.locator("[data-dust-running]")).toBeVisible();
  if (name === "性能调校") await expect(page.getByRole("slider", { name: "持续功耗上限", exact: true })).toBeVisible();
  if (name === "设备配置") await expect(page.locator('[data-mode-frequency="60"]').first()).toBeVisible();
  if (name === "键盘与灯光") {
    await expect(page.getByRole("combobox", { name: "灯效", exact: true })).toBeVisible();
    await expect(page.getByRole("region", { name: "按键映射与宏", exact: true })).toBeVisible();
    await expect(page.getByRole("region", { name: "原厂热键与屏显", exact: true })).toBeVisible();
  }
  if (name === "设备配置") await expect(page.getByRole("switch", { name: "Fn 锁", exact: true })).toBeVisible();
  if (name === "电源") {
    await expect(page.getByRole("img", { name: "PD 适配器报告功率 · 100 W", exact: true })).toBeVisible();
    await expect(page.getByRole("switch", { name: "启用原厂自动切换", exact: true })).toBeEnabled();
    await expect(page.getByRole("main", { name: "电源", exact: true })).toContainText("保存一条规则后可开启自动档案");
    const power = page.getByRole("main", { name: "电源", exact: true });
    await expect(power.getByText("外接电源 · 未在充电", { exact: true })).toHaveCount(1);
    await expect(power.getByText("已插电", { exact: true })).toHaveCount(0);
  }
  if (name === "配置档案") await expect(page.getByRole("region", { name: "档案预览", exact: true }).getByRole("heading", { name: "隔离档案", exact: true })).toBeVisible();
}

async function misplacedModules(main) {
  return main.locator(".ccc-control, .ccc-module-grid").evaluateAll(grids => {
    const visibleGrids = grids.filter(grid => grid.checkVisibility());
    if (!visibleGrids.length) return ["没有可见的模块网格"];
    return visibleGrids.flatMap(grid => {
      const style = getComputedStyle(grid);
      const widths = style.gridTemplateColumns.split(" ").map(Number.parseFloat).filter(width => width > 0);
      if (!widths.length) return [grid.className];

      let nextLeft = grid.getBoundingClientRect().left;
      const tracks = widths.map(width => {
        const track = { left: nextLeft, right: nextLeft + width };
        nextLeft = track.right + Number.parseFloat(style.columnGap);
        return track;
      });
      return [...grid.children].filter(child => child.checkVisibility()).flatMap((child, index) => {
        if (!child.matches("[data-surface], [role=region], .ccc-control, .ccc-module-grid")) return [];
        const track = tracks[index % tracks.length];
        const { left, right } = child.getBoundingClientRect();
        if (Math.abs(left - track.left) <= 1 && Math.abs(right - track.right) <= 1) return [];
        return [child.getAttribute("aria-label") ?? child.className];
      });
    });
  });
}

for (const theme of ["light", "dark"]) {
  test(`各工作区与系统区域 ${theme} 宽窄、模块边界与只读浏览`, async ({ page }) => {
    await setup(page);
    await openWorkspace(page, "设置");
    await page.getByRole("radio", { name: theme === "light" ? "浅色" : "深色", exact: true }).check();
    const before = (await writes(page)).length;
    for (const width of [1440, 360]) {
      await page.setViewportSize({ width, height: 1000 });
      for (const [index, [name, area]] of views.entries()) {
        if (area) await openSystemArea(page, area);
        else await openWorkspace(page, name);
        if (area === "配置档案") await page.getByRole("region", { name: "档案列表" }).getByRole("button", { name: /隔离档案/ }).click();
        await ready(page, area ?? name);
        const main = page.getByRole("main", { name, exact: true });
        await expect(main.getByRole("heading").first()).toBeVisible();
        const wrongInfoButtons = await main.locator('button:has(> svg[data-icon="info"])').evaluateAll(buttons => buttons
          .filter(button => button.checkVisibility() && (button.textContent.trim() || !button.getAttribute("aria-label")
            || !button.title || button.dataset.size !== "icon-compact"))
          .map(button => button.outerHTML));
        expect(wrongInfoButtons, `${area ?? name} · 信息按钮`).toEqual([]);
        expect(await misplacedModules(main), `${area ?? name} · ${width}px 模块边缘`).toEqual([]);
        const misaligned = await main.locator(".ccc-panel-head, .ccc-control-status").evaluateAll(rows => rows
          .filter(row => row.checkVisibility())
          .flatMap(row => {
            const children = [...row.children].filter(child => child.checkVisibility());
            const boxes = children.map(child => child.getBoundingClientRect());
            return boxes.flatMap((box, index) => boxes.slice(index + 1)
              .filter(next => box.top < next.bottom && next.top < box.bottom)
              .filter(next => Math.abs(box.top + box.height / 2 - next.top - next.height / 2) > 1)
              .map(() => children[index].textContent));
          }));
        expect(misaligned).toEqual([]);
        const overflow = await main.locator("section,button,input:not([hidden]),.ccc-surface").evaluateAll(elements => elements.filter(element => element.checkVisibility())
          .filter(element => element.getBoundingClientRect().right > innerWidth + 1).map(element => element.getAttribute("aria-label") ?? element.className));
        expect(overflow).toEqual([]);
        const viewport = page.locator(".ccc-workspace > .ccc-scroll-viewport");
        await viewport.evaluate(element => { element.scrollTop = 0; });
        await page.screenshot({ path: `target/ui-check/all-pages-${index}-${theme}-${width}-top.png` });
        await viewport.evaluate(element => { element.scrollTop = element.scrollHeight; });
        await page.screenshot({ path: `target/ui-check/all-pages-${index}-${theme}-${width}-bottom.png` });
      }
    }
    expect((await writes(page)).length).toBe(before);
    expect(await page.evaluate(() => window.__uiFixture.faults)).toEqual([]);
  });
}

test("显示SVG跟随IPC变化，读失败撤销画面与执行资格，重新读取显示实际值", async ({ page }) => {
  await setup(page); await openWorkspace(page, "设备配置");
  const panel = page.getByRole("region", { name: "屏幕显示", exact: true });
  await expect(panel.locator('[data-mode-frequency="60"]').first()).toBeVisible();
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__displayReadFail = true;
    window.__TAURI_INTERNALS__.invoke = (command, args) => command === "read_display_modes" && window.__displayReadFail ? Promise.reject(new Error("隔离读取失败")) : invoke(command, args);
  });
  await expect(panel.getByRole("alert")).toContainText("隔离读取失败");
  await expect(panel.locator("[data-mode-frequency]")).toHaveCount(0);
  await expect(panel.getByRole("combobox", { name: "目标刷新率", exact: true })).toHaveCount(0);
  await panel.getByRole("button", { name: "重新读取屏幕", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("隔离读取失败");
  await page.evaluate(() => {
    window.__displayReadFail = false;
    Object.assign(window.__uiFixture.display.displays[0].current, { width: 1080, height: 1920, orientation: 1, frequency: 144 });
  });
  await expect(panel.getByRole("combobox", { name: "目标刷新率", exact: true })).toHaveText("144 Hz");
  await expect(panel.locator('[data-mode-orientation="1"]')).toBeVisible();
  await expect(panel.getByRole("alert")).toHaveCount(0);
  expect(await writes(page)).toEqual([]);
});

test("白光SVG区分保存值和草稿，跨页与外部读数更新不混同", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => { window.__uiFixture.family = "white"; });
  await openWorkspace(page, "键盘与灯光");
  const panel = page.getByRole("region", { name: "键盘灯光", exact: true });
  await expect(panel.getByRole("img", { name: "当前保存亮度：3 级", exact: true })).toBeVisible();
  await expect(panel.getByText("3 级", { exact: true })).toHaveCount(1);
  await panel.getByRole("slider", { name: "亮度", exact: true }).fill("5");
  await expect(panel.getByRole("img", { name: "目标亮度：5 级", exact: true })).toBeVisible();
  await expect(panel.locator(".ccc-backlight-visual strong")).toHaveCount(0);
  await expect(panel.locator(".ccc-slider-read")).toHaveText("3 → 5 级");
  await page.evaluate(() => { window.__uiFixture.keyboard.current.brightnessLevel = 2; });
  await openSystemArea(page, "概览"); await openWorkspace(page, "键盘与灯光");
  await expect(panel.getByRole("img", { name: "目标亮度：5 级", exact: true })).toBeVisible();
  await expect(await previewConfiguration(page, "键盘灯光")).toContainText("2 级 → 5 级");
  expect(await writes(page)).toEqual([]);
});

test("供电SVG、适配器瓦数与除尘状态只取各自报告，未知不冒充零", async ({ page }) => {
  await setup(page); await openWorkspace(page, "电源");
  const supply = page.getByRole("region", { name: "原厂供电模式", exact: true });
  const details = page.getByRole("dialog", { name: "供电状态与切换规则", exact: true });
  await supply.getByRole("button", { name: "详情", exact: true }).click();
  await expect(details.locator('[data-power-source="ac"]')).toBeVisible();
  await details.getByRole("button", { name: "关闭弹窗", exact: true }).click();
  await page.evaluate(() => { window.__uiFixture.supplyMode.review.onAc = null; });
  await supply.getByRole("button", { name: "重新读取供电模式", exact: true }).click();
  await supply.getByRole("button", { name: "详情", exact: true }).click();
  await expect(details.locator('[data-power-source="unknown"]')).toBeVisible();
  await details.getByRole("button", { name: "关闭弹窗", exact: true }).click();
  const pd = page.getByRole("region", { name: "平台电源", exact: true });
  await expect(pd.getByRole("img", { name: "PD 适配器报告功率 · 100 W", exact: true })).toBeVisible();
  await page.evaluate(() => { window.__uiFixture.platform.powerDelivery.adapterWatts = null; });
  await pd.getByRole("button", { name: "重新读取平台电源", exact: true }).click();
  await expect(pd.getByRole("img", { name: "PD 适配器报告功率 · 未报告功率", exact: true })).toBeVisible();
  await expect(pd).toContainText("未报告"); await expect(pd).not.toContainText("0 W"); await expect(pd).not.toContainText("已插电");
  await openSystemArea(page, "风扇散热");
  const dust = page.getByRole("region", { name: "风扇除尘", exact: true });
  await expect(dust.locator('[data-dust-running="false"]')).toBeVisible();
  await page.evaluate(() => { window.__uiFixture.dust.running = true; });
  await dust.getByRole("button", { name: "重新读取除尘状态", exact: true }).click();
  await expect(dust.locator('[data-dust-running="true"]')).toBeVisible();
  await expect(dust.getByRole("button", { name: "立即除尘", exact: true })).toBeDisabled();
});

test("关灯保留真实档位，读取失败移除白光图形，重试保留亮度草稿", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => { const data = window.__uiFixture; data.family = "white"; data.keyboard.current.enabled = false; data.keyboard.current.brightnessLevel = 4; });
  await openWorkspace(page, "键盘与灯光");
  const panel = page.getByRole("region", { name: "键盘灯光", exact: true });
  await expect(panel.getByRole("img", { name: "当前保存亮度：已关闭 · 保留 4 级", exact: true })).toBeVisible();
  await expect(panel.locator(".ccc-backlight-visual strong")).toHaveText("保留 4 级");
  await expect(panel.locator('[data-backlight-level="4"]')).toHaveAttribute("data-backlight-enabled", "false");
  await panel.getByRole("slider", { name: "亮度", exact: true }).fill("5");
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__lightingReadFail = true;
    window.__TAURI_INTERNALS__.invoke = (command, args) => command === "read_keyboard_lighting" && window.__lightingReadFail ? Promise.reject(new Error("隔离白光读取失败")) : invoke(command, args);
  });
  await openSystemArea(page, "概览"); await openWorkspace(page, "键盘与灯光");
  await expect(panel.getByRole("alert")).toContainText("隔离白光读取失败");
  await expect(panel.locator("[data-backlight-level]")).toHaveCount(0);
  await page.evaluate(() => { window.__lightingReadFail = false; });
  await panel.getByRole("button", { name: "重新读取键盘灯", exact: true }).click();
  await expect(panel.getByRole("img", { name: "目标亮度：5 级", exact: true })).toBeVisible();
  await expect(panel.getByRole("alert")).toHaveCount(0);
  expect(await writes(page)).toEqual([]);
});

test("全键盘预览配色，开关、灯效和亮度按钮直接回读", async ({ page }) => {
  await setup(page); await openWorkspace(page, "键盘与灯光");
  const panel = page.getByRole("region", { name: "键盘灯光", exact: true });
  const board = panel.getByRole("img", { name: "全键盘背光示意" });
  await expect(board.locator(".ccc-lighting-board-key")).toHaveCount(102);
  await expect(board).toContainText("Num");
  await expect(panel.locator("[data-keyboard-zone]")).toHaveCount(3);
  await panel.getByRole("button", { name: "编辑右区", exact: true }).click();
  await setPickerColor(panel, "右区颜色", "#123456");
  const right = panel.locator('[data-keyboard-zone="2"]');
  await expect(right).toHaveAttribute("data-selected", "true");
  await expect(right).toHaveAttribute("data-zone-color", "#123456");
  const key = right.locator(".ccc-lighting-board-key").first();
  const litEdge = await key.evaluate(el => getComputedStyle(el).borderTopColor);
  await panel.getByRole("checkbox", { name: "右区开关" }).click();
  await expect(right).toHaveAttribute("data-zone-brightness", "0");
  await expect(panel.locator('[data-keyboard-zone="0"]')).toHaveAttribute("data-zone-brightness", "3");
  await expect(key).not.toHaveCSS("border-top-color", litEdge);
  await panel.getByRole("combobox", { name: "灯效", exact: true }).click();
  await page.getByRole("option", { name: "呼吸", exact: true }).click();
  await expect(await previewConfiguration(page, "键盘灯光")).toContainText("关闭 · #123456");
  await page.keyboard.press("Escape");
  const levels = panel.getByRole("group", { name: "亮度", exact: true });
  await expect(levels.getByRole("button", { pressed: true })).toHaveAccessibleName("3 级");
  await expect(levels.getByRole("button")).toHaveCount(5);
  await expect(panel.getByRole("slider", { name: "亮度", exact: true })).toHaveCount(0);
  await levels.getByRole("button", { name: "关闭", exact: true }).click();
  await expect(levels.getByRole("button", { pressed: true })).toHaveAccessibleName("关闭");
  await expect(panel.locator('[data-zone-brightness="0"]')).toHaveCount(3);
  await panel.getByRole("combobox", { name: "灯效", exact: true }).click();
  await page.getByRole("option", { name: "波浪", exact: true }).click();
  await expect(panel.locator('[data-zone-color="firmware"]')).toHaveCount(3);
  await expect(panel.getByText("固件配色", { exact: true })).toBeVisible();
  await expect(panel.getByRole("slider", { name: "色相" })).toHaveCount(0);
  const applied = (await writes(page)).filter(call => call.command === "apply_keyboard_lighting");
  expect(applied).toHaveLength(4);
  expect(applied[0].args.request.zones[2].color).toEqual({ r: 0, g: 100, b: 255 });
});

test("色环支持拖动和键盘，同色同步到整块键盘且先预览", async ({ page }) => {
  await setup(page); await openWorkspace(page, "键盘与灯光");
  const panel = page.getByRole("region", { name: "键盘灯光", exact: true });
  await panel.getByRole("combobox", { name: "灯效", exact: true }).click();
  await page.getByRole("option", { name: "同色", exact: true }).click();
  const hue = panel.getByRole("slider", { name: "色相" });
  const color = colorButton(panel, "键盘颜色");
  await expect(color).toBeEnabled();
  const { x, y, width, height } = await hue.boundingBox();
  await page.mouse.move(x + width / 2, y + height / 10);
  await page.mouse.down();
  await expect(color).toHaveAccessibleDescription("#FF0000");
  await page.mouse.move(x + width / 2, y + height * 0.9, { steps: 4 });
  await page.mouse.up();
  await expect(color).toHaveAccessibleDescription("#00FFFF");
  await expect(panel.locator('[data-zone-color="#00ffff"]')).toHaveCount(3);
  await hue.press("Home");
  await hue.press("ArrowRight");
  await expect(color).toHaveAccessibleDescription("#FF0400");
  await expect(hue).toBeFocused();
  await expect(await previewConfiguration(page, "键盘灯光")).toContainText("#ff0400");
  expect((await writes(page)).map(call => call.command)).toEqual(["apply_keyboard_lighting"]);
});

test("色块是唯一的精确选色入口，色环与灯区草稿同步", async ({ page }) => {
  await setup(page); await openWorkspace(page, "键盘与灯光");
  const panel = page.getByRole("region", { name: "键盘灯光", exact: true });
  const mode = panel.getByRole("combobox", { name: "灯效", exact: true });
  await mode.click();
  await page.getByRole("option", { name: "同色", exact: true }).click();
  const hue = panel.getByRole("slider", { name: "色相" });
  const swatch = colorButton(panel, "键盘颜色");
  await expect(panel.locator('input[type="color"]')).toHaveCount(0);
  await expect(panel.getByRole("spinbutton")).toHaveCount(0);
  await setPickerColor(panel, "键盘颜色", "#123456");
  await expect(hue).toHaveAttribute("aria-valuenow", "210");
  await expect(panel.locator('[data-zone-color="#123456"]')).toHaveCount(3);

  await hue.press("Home");
  await expect(swatch).toHaveAccessibleDescription("#FF0000");

  await mode.click();
  await page.getByRole("option", { name: "呼吸", exact: true }).click();
  await panel.getByRole("button", { name: "编辑右区", exact: true }).click();
  await setPickerColor(panel, "右区颜色", "#ff64ff");
  await expect(panel.locator('[data-keyboard-zone="2"]')).toHaveAttribute("data-zone-color", "#ff64ff");
  await expect(panel.locator('[data-zone-color="#0064ff"]')).toHaveCount(2);
  await panel.getByRole("button", { name: "编辑左区", exact: true }).click();
  await expect(colorButton(panel, "左区颜色")).toHaveAccessibleDescription("#0064FF");
  await mode.click();
  await page.getByRole("option", { name: "分区", exact: true }).click();
  await expect(panel.locator('[data-keyboard-zone="2"]')).toHaveAttribute("data-zone-color", "#ff64ff");
  await expect(await previewConfiguration(page, "键盘灯光")).toContainText("#ff64ff");
  await mode.click();
  await page.getByRole("option", { name: "波浪", exact: true }).click();
  await expect(colorButton(panel, "左区颜色")).toHaveCount(0);
  expect((await writes(page)).map(call => call.command)).toEqual(Array(4).fill("apply_keyboard_lighting"));
});

test("键盘灯光与设置卡片在深浅主题及不同宽度对齐", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => Object.assign(window.__uiFixture.keyboard.current,
    { effect: "solid", brightnessLevel: 4, allColor: { r: 132, g: 0, b: 205 } }));
  await openWorkspace(page, "键盘与灯光");
  const main = page.getByRole("main", { name: "键盘与灯光", exact: true });
  const board = main.getByRole("img", { name: "全键盘背光示意" });
  const panels = ["键盘灯光", "按键映射与宏", "原厂热键与屏显", "灯光策略与外围灯区"]
    .map(name => main.getByRole("region", { name, exact: true }));
  const swatch = colorButton(main, "键盘颜色");
  const brightness = panels[0].getByRole("group", { name: "亮度", exact: true });
  await expect(swatch).toHaveAccessibleDescription("#8400CD");
  for (const theme of ["light", "dark"]) {
    await page.evaluate(value => document.documentElement.dataset.theme = value, theme);
    for (const width of [1440, 900, 600, 480, 360]) {
      await page.setViewportSize({ width, height: width === 1440 ? 1200 : 960 });
      await expect(board).toBeVisible();
      const [lighting, keymap, hotkeys, options, color, levels] = await Promise.all([...panels, swatch, brightness].map(panel => panel.boundingBox()));
      expect(keymap.y).toBeGreaterThanOrEqual(lighting.y + lighting.height);
      expect(keymap.x).toBeCloseTo(lighting.x);
      expect(options.width).toBeCloseTo(lighting.width);
      expect(options.y).toBeGreaterThanOrEqual(Math.max(keymap.y + keymap.height, hotkeys.y + hotkeys.height));
      for (const control of [color, levels]) {
        expect(control.x).toBeGreaterThanOrEqual(lighting.x);
        expect(control.x + control.width).toBeLessThanOrEqual(lighting.x + lighting.width);
      }
      if (width === 1440) {
        expect(keymap.y).toBeCloseTo(hotkeys.y);
        expect(keymap.width).toBeCloseTo(hotkeys.width);
        expect(keymap.height).toBeCloseTo(hotkeys.height);
        expect(hotkeys.x + hotkeys.width).toBeCloseTo(lighting.x + lighting.width);
        const edits = [panels[1].getByRole("button", { name: "编辑映射", exact: true }), panels[2].getByRole("button", { name: "配置热键", exact: true })];
        const [mapEdit, hotkeyEdit] = await Promise.all(edits.map(edit => edit.boundingBox()));
        expect(mapEdit.y).toBeCloseTo(hotkeyEdit.y);
        expect(keymap.x + keymap.width - mapEdit.x - mapEdit.width).toBeCloseTo(hotkeys.x + hotkeys.width - hotkeyEdit.x - hotkeyEdit.width);
        const [idle, boot] = await Promise.all(["启用闲置关灯", "覆盖开机灯效"].map(name => panels[3].getByRole("switch", { name }).boundingBox()));
        expect(idle.y).toBeCloseTo(boot.y);
      } else if (width === 360) {
        expect(hotkeys.y).toBeGreaterThanOrEqual(keymap.y + keymap.height);
        expect(hotkeys.width).toBeCloseTo(lighting.width);
      }
      expect(await main.evaluate(el => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
      expect(await board.evaluate(el => {
        const frame = el.getBoundingClientRect();
        return Array.from(el.querySelectorAll(".ccc-lighting-board-key")).every(key => {
          const bounds = key.getBoundingClientRect();
          return bounds.width > 0 && bounds.height > 0 && bounds.left >= frame.left && bounds.right <= frame.right + 1
            && bounds.top >= frame.top && bounds.bottom <= frame.bottom + 1;
        });
      })).toBe(true);
      if (width === 1440 || width === 360 && theme === "light") {
        await page.screenshot({ path: `target/ui-check/keyboard-rgb-${theme}-${width}.png` });
      }
    }
  }
  for (const [index, action, title] of [[1, "编辑映射", "编辑按键映射与宏"], [2, "配置热键", "配置热键与屏显"], [3, "详情", "灯光策略说明"]]) {
    await panels[index].getByRole("button", { name: action, exact: true }).click();
    const dialog = page.getByRole("dialog", { name: title, exact: true });
    await expect(dialog).toBeVisible();
    await dialog.getByRole("button", { name: "关闭弹窗", exact: true }).click();
  }
  expect(await writes(page)).toEqual([]);
});
