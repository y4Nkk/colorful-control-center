/**
 * 键值事实行唯一原语（DESIGN.md 组件所有权）：每行一对标签与值，
 * 值可携带状态（ok=false / stale 时由调用方传入占位文本与告警色）。
 */
import { Surface } from "./Surface";
import type { ReactNode } from "react";

export interface Fact {
  label: string;
  value: ReactNode;
  /** 告警语气（错误红文本） */
  danger?: boolean;
}

export function FactList({ facts, compact = false, layout = "rows" }: { facts: Fact[]; compact?: boolean; layout?: "rows" | "tiles" }) {
  return (
    <Surface as="dl" variant="plain" className="ccc-facts" data-layout={layout} data-density={compact ? "compact" : undefined}>
      {facts.map((fact, index) => (
        <div key={`${fact.label}-${index}`} className="ccc-fact-row">
          <dt className="ccc-fact-label">{fact.label}</dt>
          <dd className={fact.danger ? "ccc-fact-value is-danger" : "ccc-fact-value"}>
            {fact.value}
          </dd>
        </div>
      ))}
    </Surface>
  );
}
