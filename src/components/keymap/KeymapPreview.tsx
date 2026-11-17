import type { DiffItem } from "../control/DiffPreview";
import { Surface } from "../Surface";

export function KeymapPreview({ items }: { items: DiffItem[] }) {
  if (!items.length) return null;
  return <div className="ccc-keymap-preview" aria-label="待应用变更">
    <strong>待应用变更 · {items.length}</strong>
    {items.map(item => <Surface as="div" variant="inset" className="ccc-keymap-preview-card" key={item.label}>
      <strong>{item.label}</strong>
      <div className="ccc-keymap-preview-columns">
        <div><span className="ccc-control-caption">当前配置</span><p className="ccc-keymap-preview-value ccc-mono">{item.from}</p></div>
        <div><span className="ccc-control-caption">待应用草稿</span><p className="ccc-keymap-preview-value ccc-mono">{item.to}</p></div>
      </div>
    </Surface>)}
  </div>;
}
