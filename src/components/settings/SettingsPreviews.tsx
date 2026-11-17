import { BrandLogo } from "../BrandLogo";
import type { ThemePreference, MotionPreference, CloseBehavior, StartupPage, RuntimeLogLevel } from "../../api/client";

function MiniWindow() {
  return (
    <span className="ccc-preview-window">
      <span className="ccc-preview-titlebar"><BrandLogo /><span /><i /><i /><i /></span>
      <span className="ccc-preview-sidebar"><i /><i /><i /></span>
      <span className="ccc-preview-content">
        <span className="ccc-preview-reading"><i /><i /></span>
        <svg viewBox="0 0 160 48" fill="none" aria-hidden="true" focusable="false">
          <path d="M0 42H160M0 21H160" stroke="var(--ccc-hairline)" />
          <path d="m0 35 16-3 14 5 18-18 16 6 16-15 18 7 16-12 18 7 28-5" stroke="var(--ccc-action)" strokeWidth="2" />
          <path d="m0 41 20-5 18 3 20-7 18 4 20-11 22 3 20-8 22 2" stroke="var(--ccc-safe)" strokeWidth="2" />
        </svg>
      </span>
    </span>
  );
}

/** 预览画的是界面结构，不包含模拟的硬件读数。 */
export function ThemePreview({ theme, scale = 100 }: { theme: ThemePreference; scale?: number }) {
  return (
    <span className="ccc-theme-art" aria-hidden="true">
      <span className="ccc-theme-scene" style={{ transform: `scale(${scale / 100})` }} data-preview-theme={theme === "dark" ? "dark" : "light"}><MiniWindow /></span>
      {theme === "system" && <span className="ccc-theme-scene ccc-theme-scene-half" data-preview-theme="dark"><MiniWindow /></span>}
    </span>
  );
}

export function ClosePreview({ behavior }: { behavior: CloseBehavior }) {
  const exit = behavior === "exit";
  return (
    <svg className="ccc-preference-illustration" viewBox="0 0 160 80" fill="none">
      <g opacity={exit ? 0.35 : 1}><rect x="4" y="5" width="88" height="56" rx="12" fill="var(--ccc-content)" stroke="var(--ccc-hairline)" />
        <path d="M4 22h88M24 22v39m52-49 5 5m0-5-5 5" stroke="var(--ccc-text-muted)" /></g>
      {exit ? <g stroke="var(--ccc-text-muted)" strokeWidth="2"><path d="M126 23v17m-9-11a17 17 0 1 0 18 0" /></g> : <>
        <path d="M98 31h8q15 0 15 15v11m-5-5 5 5 5-5" stroke="var(--ccc-action)" strokeWidth="1.5" strokeLinecap="round" />
        <rect x="101" y="61" width="54" height="16" rx="8" fill="var(--ccc-content)" stroke="var(--ccc-hairline)" />
        <BrandLogo x={106} y={62} width={14} height={14} />
        <circle cx="130" cy="69" r="2" fill="var(--ccc-text-muted)" /><circle cx="141" cy="69" r="2" fill="var(--ccc-text-muted)" />
      </>}
    </svg>
  );
}

export function MotionPreview({ mode }: { mode: MotionPreference }) {
  return <svg className="ccc-preference-illustration" viewBox="0 0 160 80" fill="none">
    <rect x="12" y="12" width="136" height="56" rx="12" fill="var(--ccc-content)" stroke="var(--ccc-hairline)" />
    <path d="M32 40h90" stroke="var(--ccc-hairline)" strokeWidth="4" />
    <circle cx="122" cy="40" r="7" stroke="var(--ccc-text-muted)" />
    <circle cx={mode === "reduce" ? 122 : 32} cy="40" r="7" fill="var(--ccc-action)" className={mode === "system" ? "ccc-motion-preview" : undefined} />
  </svg>;
}

export function StartupPagePreview({ page }: { page: StartupPage }) {
  return <svg className="ccc-preference-illustration" viewBox="0 0 160 80" fill="none">
    <rect x="12" y="8" width="136" height="64" rx="12" fill="var(--ccc-content)" stroke="var(--ccc-hairline)" />
    <path d="M38 8v64" stroke="var(--ccc-hairline)" />
    <rect x="19" y={page === "dashboard" ? 21 : 38} width="12" height="4" rx="2" fill="var(--ccc-action)" />
    <path d={page === "dashboard" ? "M54 52l16-13 18 5 22-20 20 5" : "M54 25h72m-72 15h56m-56 15h64"} stroke="var(--ccc-action)" strokeWidth="3" />
  </svg>;
}

export function LogPreview({ level }: { level: RuntimeLogLevel }) {
  const levels: RuntimeLogLevel[] = ["debug", "info", "warn", "error", "silent"];
  const rank = levels.indexOf(level);
  return <svg className="ccc-preference-illustration" viewBox="0 0 160 80" fill="none">
    <rect x="12" y="8" width="136" height="64" rx="12" fill="var(--ccc-content)" stroke="var(--ccc-hairline)" />
    {rank === 4 ? <g stroke="var(--ccc-text-muted)" strokeWidth="2"><circle cx="80" cy="40" r="18" /><path d="m66 54 28-28" /></g>
      : levels.slice(0, 4).map((item, index) => index >= rank && <g key={item}>
        <circle cx="28" cy={20 + index * 13} r="3" fill={`var(--ccc-${index === 3 ? "danger" : index === 2 ? "warning" : "action"})`} />
        <path d={`M42 ${20 + index * 13}h${80 - index * 8}`} stroke="var(--ccc-hairline)" strokeWidth="4" />
      </g>)}
  </svg>;
}
