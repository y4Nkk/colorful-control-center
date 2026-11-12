/**
 * 键值事实行唯一原语（DESIGN.md 组件所有权）：每行一对标签与值，
 * 值可携带状态（ok=false / stale 时由调用方传入占位文本与告警色）。
 */
import type { ReactNode } from "react";

export interface Fact {
  label: string;
  value: ReactNode;
  /** 告警语气（错误红文本） */
  danger?: boolean;
}

export function FactList({ facts }: { facts: Fact[] }) {
  return (
    <div className="ccc-facts">
      {facts.map((fact) => (
        <div key={fact.label} className="ccc-fact-row">
          <span className="ccc-fact-label">{fact.label}</span>
          <span className={fact.danger ? "ccc-fact-value is-danger" : "ccc-fact-value"}>
            {fact.value}
          </span>
        </div>
      ))}
    </div>
  );
}
