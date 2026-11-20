export interface ParameterDescription { name: string; description: string }

export function ParameterDescriptions({ items }: { items: readonly ParameterDescription[] }) {
  return <ul className="ccc-parameter-descriptions">
    {items.map(item => <li key={item.name}><strong>{item.name}</strong><p>{item.description}</p></li>)}
  </ul>;
}
