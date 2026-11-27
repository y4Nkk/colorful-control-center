import type { ReactNode } from "react";
import { Icon, type IconName } from "./Icon";
import { Surface } from "./Surface";

export interface Fact {
  label: string;
  value: ReactNode;
  note?: string;
  icon?: IconName;
  tone?: "active" | "warning";
  format?: "code" | "number";
}

type ComparisonFact = Omit<Fact, "format"> & { current: ReactNode };
type FactListProps = { label?: string; compact?: boolean } & (
  { facts: readonly Fact[]; layout?: "rows" | "tiles" | "inline" }
  | { facts: readonly ComparisonFact[]; layout: "comparison" }
);

export function FactStatus({ children, tone = "neutral" }: {
  children: string;
  tone?: "neutral" | "success" | "warning";
}) {
  return <span className="ccc-fact-status" data-tone={tone}>{children}</span>;
}

export function FactList(props: FactListProps) {
  const comparing = props.layout === "comparison";
  const facts: readonly Fact[] = comparing ? props.facts.map(({ current, value, ...fact }) => ({
    ...fact, value: <ComparisonValues current={current} target={value} />,
  })) : props.facts;
  const list = (
    <Surface as="dl" variant="plain" className="ccc-facts" aria-label={props.label} data-layout={props.layout ?? "rows"}
      data-density={props.compact ? "compact" : undefined}>
      {facts.map(({ label, value, note, icon, tone, format }, index) => (
        <div key={`${label}-${index}`} className="ccc-fact-row">
          <dt className="ccc-fact-label" data-icon={icon}>
            {icon && <span className="ccc-fact-icon" data-tone={tone}><Icon name={icon} size={16} /></span>}
            {icon || note ? <span>{label}{note && <span className="ccc-fact-note">{note}</span>}</span> : label}
          </dt>
          <dd className="ccc-fact-value">
            {format === "code" ? <code className="ccc-fact-code">{value}</code>
              : format === "number" ? <strong className="ccc-fact-number">{value}</strong> : value}
          </dd>
        </div>
      ))}
    </Surface>
  );
  if (!comparing) return list;
  return <div className="ccc-fact-comparison">
    <div className="ccc-fact-comparison-head" aria-hidden="true">
      <span>设置</span>
      <span className="ccc-fact-comparison-values"><span>当前值</span><span>目标值</span></span>
    </div>
    {list}
  </div>;
}

function ComparisonValues({ current, target }: { current: ReactNode; target: ReactNode }) {
  return <span className="ccc-fact-comparison-values">
    <span role="group" aria-label="当前值">
      <span className="ccc-fact-comparison-caption" aria-hidden="true">当前值</span>
      {current}
    </span>
    <span role="group" aria-label="目标值">
      <span className="ccc-fact-comparison-caption" aria-hidden="true">目标值</span>
      <strong>{target}</strong>
    </span>
  </span>;
}
