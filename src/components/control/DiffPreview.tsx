/** 待应用变更的微型差异预览：仅变更字段，当前 → 草稿。 */

export interface DiffItem {
  label: string;
  from: string;
  to: string;
}

export function DiffPreview({ items }: { items: DiffItem[] }) {
  if (items.length === 0) return null;
  return (
    <div className="ccc-diff" aria-label="待应用变更">
      {items.map((item) => (
        <span key={item.label} className="ccc-diff-chip">
          <span>{item.label}</span>
          <span className="ccc-mono">
            {item.from} → {item.to}
          </span>
        </span>
      ))}
    </div>
  );
}
