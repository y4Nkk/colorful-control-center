import { Collapsible } from "../Collapsible";

export interface ParameterDescription { name: string; description: string }

export function ParameterHelp({ title, items }: { title: string; items: readonly ParameterDescription[] }) {
  return <Collapsible summary={title} className="ccc-parameter-help">
    <ul className="ccc-parameter-descriptions">
      {items.map(item => <li key={item.name}><strong>{item.name}</strong><p>{item.description}</p></li>)}
    </ul>
  </Collapsible>;
}
