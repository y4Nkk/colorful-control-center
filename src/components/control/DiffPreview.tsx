import { FactList } from "../FactList";

export interface DiffItem {
  label: string;
  from: string;
  to: string;
}

export function DiffPreview({ items }: { items: DiffItem[] }) {
  if (items.length === 0) return null;
  return <FactList compact layout="tiles" label="待应用变更" facts={items.map(({ label, from, to }) => ({
    label,
    value: <>{from} → <strong>{to}</strong></>,
  }))} />;
}
