import { DetailsDialog } from "../DetailsDialog";

export interface ParameterDescription { name: string; description: string }

export function ParameterHelp({ title, items }: { title: string; items: readonly ParameterDescription[] }) {
  return <DetailsDialog title={title}><ParameterDescriptions items={items} /></DetailsDialog>;
}

export function ParameterDescriptions({ items }: { items: readonly ParameterDescription[] }) {
  return <ul className="ccc-parameter-descriptions">
    {items.map(item => <li key={item.name}><strong>{item.name}</strong><p>{item.description}</p></li>)}
  </ul>;
}
