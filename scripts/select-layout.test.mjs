import assert from "node:assert/strict";
import { test } from "node:test";
import { selectLayout } from "../src/lib/select-layout.ts";

const layout = overrides => selectLayout({ anchor: { left: 200, top: 100, bottom: 140 },
  viewport: { width: 900, height: 560 }, naturalHeight: 260, menuWidth: 200, heightLimit: 384, gap: 4, edge: 8, scale: 1, ...overrides });

test("足够空间时向下展开，保留触发器间隔", () => {
  assert.deepEqual(layout(), { left: 200, top: 144, maxHeight: 384, side: "bottom" });
});
test("下方空间不足时向上展开", () => {
  const result = layout({ anchor: { left: 200, top: 480, bottom: 520 } });
  assert.equal(result.side, "top"); assert.equal(result.top, 216); assert.equal(result.maxHeight, 384);
});
test("两侧都不足时选择较大一侧并限制高度", () => {
  const result = layout({ naturalHeight: 900, anchor: { left: 200, top: 300, bottom: 340 } });
  assert.equal(result.side, "top"); assert.equal(result.maxHeight, 288); assert.equal(result.top, 8);
});
test("右侧、左侧、窄窗口与超宽触发器不越界", () => {
  for (const [left, width, viewportWidth] of [[850, 100, 900], [-20, 150, 900], [300, 80, 320], [0, 700, 900]]) {
    const menuWidth = Math.min(width, viewportWidth - 16);
    const result = layout({ anchor: { left, top: 100, bottom: 140 }, menuWidth, viewport: { width: viewportWidth, height: 560 } });
    assert.ok(result.left >= 8); assert.ok(result.left + menuWidth <= viewportWidth - 8);
  }
});
for (const scale of [0.9, 1, 1.1, 1.25, 2]) {
  test(`${scale} 倍根缩放输出正确 CSS 坐标且重复计算稳定`, () => {
    const result = layout({ scale });
    assert.equal(result.left * scale, 200); assert.equal(result.top * scale, 144);
    assert.deepEqual(result, layout({ scale }));
  });
}
