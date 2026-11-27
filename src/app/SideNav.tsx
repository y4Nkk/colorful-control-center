import { useLayoutEffect, useRef } from "react";
import { openExternalUrl, toCommandError } from "../api/client";
import { Button } from "../components/Button";
import { Icon, NavigationIcon } from "../components/Icon";
import { ScrollArea } from "../components/ScrollArea";
import { Tooltip } from "../components/Tooltip";
import { toast } from "../components/use-toast";
import { PAGES, type PageDefinition, type PageId } from "./navigation";
import { useApplicationUpdate } from "./useApplicationUpdate";
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
const FEEDBACK_URL = "https://github.com/y4Nkk/colorful-control-center/issues/new/choose";

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
        <ScrollArea className="ccc-nav-pages" aria-label="硬件与档案导航">
          <SideNavList pages={WORKSPACE_PAGES} current={page === "settings" ? null : page}
            collapsed={collapsed} onPageChange={onPageChange} />
        </ScrollArea>
        <SideNavList footer pages={FOOTER_PAGES} current={page === "settings" ? page : null}
          collapsed={collapsed} onPageChange={onPageChange} />
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
      {footer && <>
        <li className="ccc-nav-row"><SidebarUpdate collapsed={collapsed} /></li>
        <li className="ccc-nav-row"><SidebarFeedback collapsed={collapsed} /></li>
      </>}
      {pages.map(definition => <li className="ccc-nav-row" key={definition.id}>
        <SideNavItem definition={definition} active={current === definition.id} collapsed={collapsed}
          onSelect={() => onPageChange(definition.id)} />
      </li>)}
      {current && <li ref={indicator} className="ccc-nav-indicator" aria-hidden="true" />}
    </ul>
  );
}

function SidebarFeedback({ collapsed }: { collapsed: boolean }) {
  const openFeedback = () => {
    void openExternalUrl(FEEDBACK_URL).catch(error =>
      toast({ kind: "error", title: "无法打开反馈页面", description: toCommandError(error).message }));
  };
  return <Tooltip content={collapsed ? "反馈" : undefined} side="right">
    <Button variant="navigation" className="ccc-nav-item" aria-label="反馈" onClick={openFeedback}>
      <span className="ccc-nav-glyph" aria-hidden="true"><NavigationIcon name="feedback" weight="regular" /></span>
      <span className="ccc-nav-label" aria-hidden="true">反馈</span>
    </Button>
  </Tooltip>;
}

const UPDATE_STATUS = {
  idle: ["检查更新", "refresh"],
  checking: ["正在检查更新", "loading"],
  current: ["已是最新版本", "check-circle"],
  available: ["发现新版本", "download"],
  starting: ["正在开始下载", "loading"],
  downloading: ["正在下载", "loading"],
  verifying: ["正在校验签名", "loading"],
  launching: ["正在打开安装向导", "loading"],
  installerOpen: ["安装向导已打开", "confirm"],
  cancelled: ["更新已取消", "refresh"],
  finished: ["安装向导已完成", "check-circle"],
  error: ["更新失败", "error"],
} as const;

function SidebarUpdate({ collapsed }: { collapsed: boolean }) {
  const { state, candidate, cancelling, activate } = useApplicationUpdate();
  const { phase } = state;
  const checking = phase === "checking";
  const [text, icon] = UPDATE_STATUS[phase];
  const status = phase === "error" && !candidate ? "检查更新失败" : text;
  const label = cancelling && (phase === "downloading" || phase === "verifying") ? "正在取消下载" : status;
  const percent = phase === "downloading" && state.totalBytes
    ? Math.round(Math.min(state.downloadedBytes / state.totalBytes, 1) * 100) : null;
  const detail = percent !== null ? `${percent}%` : phase === "available" && candidate ? `v${candidate.version}` : "";
  const title = phase === "current" ? `${label}，重新检查` : [label, detail].filter(Boolean).join(" ");

  return <>
    <Tooltip content={collapsed ? title : undefined} side="right">
      <Button variant="navigation" className="ccc-update-nav" data-phase={phase}
        aria-label={title} aria-disabled={checking || undefined} aria-busy={checking || undefined}
        aria-haspopup="dialog" onClick={() => activate("sidebar")}>
        <span className="ccc-nav-glyph ccc-update-nav-glyph" aria-hidden="true">
          {percent === null ? <Icon name={icon} /> : <>
            <svg className="ccc-update-nav-ring" viewBox="0 0 24 24" focusable="false">
              <circle cx="12" cy="12" r="10" />
              <circle cx="12" cy="12" r="10" pathLength="100" strokeDasharray={`${percent} 100`} />
            </svg>
            <Icon name="download" size={16} />
          </>}
        </span>
        <span className="ccc-nav-label ccc-update-nav-copy" aria-hidden="true">
          <span>{label}</span>
          {detail && <span className="ccc-mono">{detail}</span>}
        </span>
      </Button>
    </Tooltip>
    <span className="ccc-update-nav-announcement" role="status">
      {phase === "available" ? title : phase !== "idle" ? label : ""}
    </span>
  </>;
}

function SideNavItem({ definition, active, collapsed, onSelect }: {
  definition: PageDefinition;
  active: boolean;
  collapsed: boolean;
  onSelect: () => void;
}) {
  const glyph = useRef<HTMLSpanElement>(null);
  const wasActive = useRef(active);
  useLayoutEffect(() => {
    glyph.current?.toggleAttribute("data-entering", active && !wasActive.current);
    wasActive.current = active;
  }, [active]);

  return (
    <Tooltip content={collapsed ? definition.title : undefined} side="right">
      <Button variant="navigation" className="ccc-nav-item" aria-label={definition.title}
        aria-current={active ? "page" : undefined} onClick={onSelect}>
        <span className="ccc-nav-glyph" ref={glyph} aria-hidden="true"
          onAnimationEnd={event => event.currentTarget.removeAttribute("data-entering")}>
          <NavigationIcon name={definition.icon} weight="regular" />
          <NavigationIcon name={definition.icon} weight="fill" />
        </span>
        <span className="ccc-nav-label" aria-hidden="true">{definition.title}</span>
      </Button>
    </Tooltip>
  );
}
