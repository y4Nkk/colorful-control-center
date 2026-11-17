import type { DetailsHTMLAttributes, ReactNode } from "react";
import { Icon } from "./Icon";

interface CollapsibleProps extends DetailsHTMLAttributes<HTMLDetailsElement> {
  summary: ReactNode;
  children: ReactNode;
}

/** 原生开合保留内容挂载与键盘语义，页面 keep-alive 保留展开状态。 */
export function Collapsible({ summary, children, className, ...props }: CollapsibleProps) {
  return (
    <details {...props} className={["ccc-collapsible", className].filter(Boolean).join(" ")}>
      <summary className="ccc-collapsible-trigger">
        <Icon name="chevron-right" className="ccc-collapsible-chevron" />
        <span>{summary}</span>
      </summary>
      <div className="ccc-collapsible-content">{children}</div>
    </details>
  );
}
