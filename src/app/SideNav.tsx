import * as Tooltip from "@radix-ui/react-tooltip";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Button } from "../components/Button";
import { Icon, NavigationIcon } from "../components/Icon";
import { ScrollArea } from "../components/ScrollArea";
import { PAGES, type PageDefinition, type PageId } from "./navigation";
import { useSideNavIndicator } from "./useSideNavIndicator";

interface SideNavProps {
  page: PageId;
  onPageChange: (page: PageId) => void;
  collapsed: boolean;
  compact: boolean;
  onToggle: () => void;
}

const WORKSPACE_PAGES = PAGES.filter(({ id }) => id !== "settings");
const FOOTER_PAGES = PAGES.filter(({ id }) => id === "settings");

export function SideNav({ page, onPageChange, collapsed, compact, onToggle }: SideNavProps) {
  const toggleLabel = collapsed ? "展开侧边栏" : "收起侧边栏";
  return (
    <aside className="ccc-sidebar">
      <div className="ccc-sidebar-top" data-tauri-drag-region>
        {!compact && <Button variant="navigation" size="icon" className="ccc-sidebar-toggle"
          aria-label={toggleLabel} aria-expanded={!collapsed} aria-controls="ccc-primary-navigation" onClick={onToggle}>
          <Icon name="collapse" />
        </Button>}
      </div>
      <nav id="ccc-primary-navigation" className="ccc-nav" aria-label="主导航">
        <Tooltip.Provider delayDuration={150} disableHoverableContent>
          <ScrollArea className="ccc-nav-pages" aria-label="硬件与档案导航">
            <SideNavList pages={WORKSPACE_PAGES} current={page === "settings" ? null : page}
              collapsed={collapsed} onPageChange={onPageChange} />
          </ScrollArea>
          <SideNavList footer pages={FOOTER_PAGES} current={page === "settings" ? page : null}
            collapsed={collapsed} onPageChange={onPageChange} />
        </Tooltip.Provider>
      </nav>
    </aside>
  );
}

function SideNavList({ pages, current, collapsed, onPageChange, footer }: {
  pages: PageDefinition[];
  current: PageId | null;
  collapsed: boolean;
  onPageChange: SideNavProps["onPageChange"];
  footer?: boolean;
}) {
  const list = useRef<HTMLUListElement>(null);
  const indicator = useSideNavIndicator(list, current);
  return (
    <ul className={`ccc-nav-list${footer ? " ccc-nav-footer" : ""}`} role="list" ref={list}>
      {pages.map(definition => <li className="ccc-nav-row" key={definition.id}>
        <SideNavItem definition={definition} active={current === definition.id} collapsed={collapsed}
          onSelect={() => onPageChange(definition.id)} />
      </li>)}
      {current && <li ref={indicator} className="ccc-nav-indicator" aria-hidden="true" />}
    </ul>
  );
}

function SideNavItem({ definition, active, collapsed, onSelect }: {
  definition: PageDefinition;
  active: boolean;
  collapsed: boolean;
  onSelect: () => void;
}) {
  const [hintOpen, setHintOpen] = useState(false);
  const glyph = useRef<HTMLSpanElement>(null);
  const wasActive = useRef(active);
  useLayoutEffect(() => {
    glyph.current?.toggleAttribute("data-entering", active && !wasActive.current);
    wasActive.current = active;
  }, [active]);
  useEffect(() => {
    if (!hintOpen) return;
    const dismiss = () => setHintOpen(false);
    if (!collapsed) { dismiss(); return; }
    window.addEventListener("blur", dismiss);
    return () => window.removeEventListener("blur", dismiss);
  }, [collapsed, hintOpen]);

  return (
    <Tooltip.Root open={collapsed && hintOpen} onOpenChange={open => setHintOpen(collapsed && open)}>
      <Tooltip.Trigger asChild>
        <Button variant="navigation" className="ccc-nav-item" aria-label={definition.title}
          aria-current={active ? "page" : undefined} onClick={onSelect}>
          <span className="ccc-nav-glyph" ref={glyph} aria-hidden="true"
            onAnimationEnd={event => event.currentTarget.removeAttribute("data-entering")}>
            <NavigationIcon name={definition.icon} weight="regular" />
            <NavigationIcon name={definition.icon} weight="fill" />
          </span>
          <span className="ccc-nav-label" aria-hidden="true">{definition.title}</span>
        </Button>
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content className="ccc-nav-tooltip" side="right" sideOffset={6}>{definition.title}</Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}
