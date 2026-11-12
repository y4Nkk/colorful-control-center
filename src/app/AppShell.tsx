/**
 * 应用外框唯一所有者（几何契约见 DESIGN.md）：两行两列 grid——导航跨左
 * 两行、顶栏右上、工作区右下（28px 圆角，唯一滚动面）。页面不拼装外框。
 */
import type { ReactNode } from "react";
import { PAGES, pageTitle, type PageId } from "./navigation";

interface AppShellProps {
  page: PageId;
  onPageChange: (page: PageId) => void;
  globalInfo?: ReactNode;
  children: ReactNode;
}

export function AppShell({ page, onPageChange, globalInfo, children }: AppShellProps) {
  return (
    <div className="ccc-shell">
      <aside className="ccc-sidebar">
        <div className="ccc-brand">Colorful Control Center</div>
        <nav className="ccc-nav" aria-label="主导航">
          {PAGES.map((definition) => (
            <button
              key={definition.id}
              type="button"
              className="ccc-nav-item"
              aria-current={page === definition.id ? "page" : undefined}
              onClick={() => onPageChange(definition.id)}
            >
              {definition.title}
            </button>
          ))}
        </nav>
      </aside>
      <header className="ccc-topbar">
        <h1 className="ccc-topbar-title">{pageTitle(page)}</h1>
        <div className="ccc-topbar-spacer" />
        <div className="ccc-global-info">{globalInfo}</div>
      </header>
      <main className="ccc-workspace" aria-label={pageTitle(page)}>
        {children}
      </main>
    </div>
  );
}
