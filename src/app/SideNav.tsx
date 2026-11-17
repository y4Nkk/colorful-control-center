import { Button } from "../components/Button";
import { PAGES, type PageDefinition, type PageId } from "./navigation";
import { Icon } from "../components/Icon";
import { ScrollArea } from "../components/ScrollArea";

interface SideNavProps {
  page: PageId;
  onPageChange: (page: PageId) => void;
  collapsed: boolean;
  compact: boolean;
  onToggle: () => void;
}

const WORKSPACE_PAGES = PAGES.filter((page) => page.id !== "settings");
const FOOTER_PAGES = PAGES.filter((page) => page.id === "settings");

export function SideNav({ page, onPageChange, collapsed, compact, onToggle }: SideNavProps) {
  const toggleLabel = collapsed ? "展开侧边栏" : "收起侧边栏";
  return (
    <aside className="ccc-sidebar">
      <div className="ccc-sidebar-top" data-tauri-drag-region>
        {!compact && <Button variant="ghost" size="icon" type="button" className="ccc-sidebar-toggle" aria-label={toggleLabel}
          title={toggleLabel} aria-expanded={!collapsed} aria-controls="ccc-primary-navigation" onClick={onToggle}>
          <Icon name="collapse" />
        </Button>}
      </div>
      <nav id="ccc-primary-navigation" className="ccc-nav" aria-label="主导航">
        <ScrollArea className="ccc-nav-pages" aria-label="硬件与档案导航"><div className="ccc-nav-group">
          {WORKSPACE_PAGES.map((definition) => (
            <SideNavItem key={definition.id} definition={definition} active={page === definition.id}
              onSelect={() => onPageChange(definition.id)} />
          ))}
        </div></ScrollArea>
        <div className="ccc-nav-footer">
          {FOOTER_PAGES.map((definition) => (
            <SideNavItem key={definition.id} definition={definition} active={page === definition.id}
              onSelect={() => onPageChange(definition.id)} />
          ))}
        </div>
      </nav>
    </aside>
  );
}

function SideNavItem({ definition, active, onSelect }: {
  definition: PageDefinition;
  active: boolean;
  onSelect: () => void;
}) {
  return (
    <Button variant="navigation" type="button" className="ccc-nav-item" aria-label={definition.title} title={definition.title}
      aria-current={active ? "page" : undefined} onClick={onSelect}>
      <Icon name={definition.icon} size={24} />
      <span className="ccc-nav-label" aria-hidden="true">{definition.title}</span>
    </Button>
  );
}
