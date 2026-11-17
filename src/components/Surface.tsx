import type { HTMLAttributes } from "react";

export type SurfaceVariant = "plain" | "panel" | "card" | "group" | "workspace" | "inset";
interface SurfaceProps extends HTMLAttributes<HTMLElement> {
  as?: "div" | "section" | "label" | "main";
  variant?: SurfaceVariant;
}

export function Surface({ as: Tag = "div", variant = "panel", className, ...props }: SurfaceProps) {
  return <Tag {...props} data-surface={variant}
    className={["ccc-surface", className].filter(Boolean).join(" ")} />;
}
