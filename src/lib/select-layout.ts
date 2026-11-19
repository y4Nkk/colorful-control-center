export interface SelectLayoutInput {
  anchor: { left: number; top: number; bottom: number };
  viewport: { width: number; height: number };
  naturalHeight: number;
  menuWidth: number;
  heightLimit: number;
  gap: number;
  edge: number;
  scale: number;
}

/** 输入为实际视口坐标；输出换回菜单所在根缩放下的 CSS 坐标。 */
export function selectLayout({ anchor, viewport, naturalHeight, menuWidth, heightLimit, gap, edge, scale }: SelectLayoutInput) {
  const availableWidth = Math.max(0, viewport.width - edge * 2);
  const width = Math.min(availableWidth, menuWidth);
  const desiredHeight = Math.min(naturalHeight, heightLimit);
  const below = Math.max(0, viewport.height - edge - anchor.bottom - gap);
  const above = Math.max(0, anchor.top - edge - gap);
  const side = below >= desiredHeight || below >= above ? "bottom" : "top";
  const maxHeight = Math.min(heightLimit, side === "bottom" ? below : above);
  const height = Math.min(desiredHeight, maxHeight);
  const left = Math.max(edge, Math.min(anchor.left, viewport.width - edge - width));
  const top = side === "bottom" ? anchor.bottom + gap : Math.max(edge, anchor.top - gap - height);
  return { left: left / scale, top: top / scale, maxHeight: maxHeight / scale, side };
}
