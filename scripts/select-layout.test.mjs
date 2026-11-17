import assert from "node:assert/strict";
import { test } from "node:test";
import { selectLayout } from "../src/lib/select-layout.ts";

const layout = overrides => selectLayout({ anchor: { left: 200, top: 100, right: 350, bottom: 140, width: 150 },
  viewport: { width: 900, height: 560 }, naturalHeight: 260, minWidth: 200, maxWidth: 400, gap: 8, edge: 8, scale: 1, ...overrides });

test("足够空间时向下展开，保留触发器间隔", () => {
  assert.deepEqual(layout(), { left: 200, top: 148, width: 200, maxHeight: 404, side: "bottom" });
});
test("下方空间不足时向上展开", () => {
  const result = layout({ anchor: { left: 200, top: 480, right: 350, bottom: 520, width: 150 } });
  assert.equal(result.side, "top"); assert.equal(result.top, 212); assert.equal(result.maxHeight, 464);
});
test("两侧都不足时选择较大一侧并限制高度", () => {
  const result = layout({ naturalHeight: 900, anchor: { left: 200, top: 300, right: 350, bottom: 340, width: 150 } });
  assert.equal(result.side, "top"); assert.equal(result.maxHeight, 284); assert.equal(result.top, 8);
});
test("右侧、左侧、窄窗口与超宽触发器不越界", () => {
  for (const [left, width, viewportWidth] of [[850, 100, 900], [-20, 150, 900], [300, 80, 320], [0, 700, 900]]) {
    const result = layout({ anchor: { left, top: 100, right: left + width, bottom: 140, width }, viewport: { width: viewportWidth, height: 560 } });
    assert.ok(result.left >= 8); assert.ok(result.left + result.width <= viewportWidth - 8);
    assert.ok(result.width <= 400);
  }
});
for (const scale of [0.9, 1, 1.1, 1.25, 2]) {
  test(`${scale} 倍根缩放输出正确 CSS 坐标且重复计算稳定`, () => {
    const result = layout({ scale });
    assert.equal(result.left * scale, 200); assert.equal(result.top * scale, 148);
    assert.equal(result.width * scale, 200); assert.deepEqual(result, layout({ scale }));
  });
}
