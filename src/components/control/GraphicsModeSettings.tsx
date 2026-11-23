import { applyGraphicsMode, readGraphicsMode, rebootForGraphicsMode, restoreGraphicsMode } from "../../api/client";
import { Button } from "../Button";
import { ConfirmButton } from "../ConfirmButton";
import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { GraphicsModeArtwork } from "./GraphicsModeArtwork";
import { useDeviceSettings } from "./useDeviceSettings";

const MODES = [
  { value: 1, label: "仅集显", route: "integrated" },
  { value: 2, label: "仅独显", route: "discrete" },
  { value: 3, label: "混合显卡", route: "hybrid" },
  { value: 4, label: "动态切换", route: "dynamic" },
] as const;
const modeName = (mode: number) => MODES.find(item => item.value === mode)?.label ?? "未知";
const supports = (mask: number, mode: number) => (mask & (1 << (mode - 1))) !== 0;

export function GraphicsModeSettings({ active, revision }: { active: boolean; revision: number }) {
  const { profile, busy, error, run, reread } = useDeviceSettings(readGraphicsMode, active, revision);
  const current = profile?.current, last = profile?.lastRequest;
  const pending = last?.verification === "pendingReboot";
  const retry = last?.verification === "uncertain" || last?.verification === "notApplied";
  const choices = current ? MODES.filter(item => supports(current.availableMask, item.value)) : [];
  const selected = pending ? last.target : current?.mode;
  const showReport = current && (selected !== current.mode || !choices.some(choice => choice.value === current.mode));
  const choose = (target: number) => {
    if (profile && (target !== selected || retry)) {
      void run(() => applyGraphicsMode(profile.revision, target));
    }
  };

  return <Surface as="section" variant="group" className="ccc-panel" aria-label="显卡模式" aria-busy={busy || undefined}>
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="gpu" />显卡模式</h2>
      <div className="ccc-module-tools"><Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取显卡模式" disabled={busy} onClick={reread} /></div></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {profile && current ? <>
      {showReport && <div className="ccc-module-summary">
        <span className="ccc-control-caption">固件报告</span><strong>{modeName(current.mode)}</strong>
      </div>}
      {choices.length > 0 && <div className="ccc-graphics-choices" role="group" aria-label="切换显卡模式">
        {choices.map(({ value, label, route }) => <Button key={value} size="compact" variant="selection" disabled={busy}
          aria-pressed={selected === value} onClick={() => choose(value)}>
          <GraphicsModeArtwork route={route} />
          <span>{label}</span>
        </Button>)}
      </div>}
      {pending && <div className="ccc-restart-notice">
        <span className="ccc-control-caption" role="status"><strong>{modeName(last.target)}</strong> · 重启 Windows 后生效</span>
        <ConfirmButton key={`reboot-${profile.revision}`} label="重启 Windows" confirmLabel="已保存工作，确认重启 Windows"
          size="compact" busy={busy} icon="refresh" onConfirm={() => run(() => rebootForGraphicsMode(profile.revision), "Windows 重启请求已发送")}>
          <p>重启后切换至{modeName(last.target)}。请先保存工作。</p>
        </ConfirmButton>
      </div>}
      {retry && <p className="ccc-control-caption" role="status">
        {last.verification === "uncertain" ? "上次操作未完成。" : "重启后未匹配目标。"}
        {supports(current.availableMask, last.target) ? `可再次选择${modeName(last.target)}。` : "固件已不提供上次选择的模式。"}
      </p>}
      {last?.verification === "confirmed" && <p className="ccc-control-caption" role="status">重启后已确认</p>}
      {last && !supports(current.availableMask, last.initialMode) && <p className="ccc-control-caption">当前固件未提供原模式，无法恢复。</p>}
      {profile.canRestore && last && <div className="ccc-actions">
        <ConfirmButton key={profile.revision} icon="undo" size="compact" label="恢复初始设置" confirmLabel="确认恢复" busy={busy}
          onConfirm={() => run(() => restoreGraphicsMode(profile.revision))}>
          <p>恢复{modeName(last.initialMode)}，重启 Windows 后生效。</p>
        </ConfirmButton>
      </div>}
    </> : <p className="ccc-control-caption">{profile ? "本机未提供模式切换接口。" : error ? "请重新读取显卡模式。" : "正在读取显卡模式。"}</p>}
  </Surface>;
}
