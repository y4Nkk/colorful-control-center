/**
 * 占位工作面唯一原语：Phase 模块接入前的页面占位，接入后整体替换内容、
 * 壳与导航不动（DESIGN.md 组件所有权）。
 */
import { Surface } from "./Surface";
import type { ReactNode } from "react";

interface PagePlaceholderProps {
  title: string;
  phase: string;
  description: string;
  children?: ReactNode;
}

export function PagePlaceholder({ title, phase, description, children }: PagePlaceholderProps) {
  return (
    <Surface as="section" variant="group" className="ccc-placeholder">
      <span className="ccc-phase">{phase}</span>
      <h2>{title}</h2>
      <p>{description}</p>
      {children}
    </Surface>
  );
}
