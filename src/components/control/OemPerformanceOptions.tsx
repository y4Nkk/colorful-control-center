import { Icon } from "../Icon";
import { applyOemPerformanceOptions, restoreOemPowerMode, OEM_POWER_MODES } from "../../api/client";
import { Button } from "../Button";
import { Collapsible } from "../Collapsible";
import { ConfirmButton } from "../ConfirmButton";
import { Surface } from "../Surface";
import { Switch } from "../Switch";
import { useOemPerformanceOptions } from "./useOemPerformanceOptions";
import { OemModeRestore } from "./OemModeRestore";
export const performanceState = (value: boolean | null) => value === null ? "固件未提供" : value ? "开启" : "关闭";
export const PERFORMANCE_FIELDS = [{ key: "turboFan", label: "Turbo 风扇", description: "原厂性能模式的风扇加速选项" },
  { key: "dtt", label: "动态调校 DTT", description: "使用 Dynamic Tuning 驱动调节性能" }] as const;
export function OemPerformanceOptions({ active, revision, onSettled }: { active: boolean; revision: number; onSettled: () => void }) {
  const { profile, draft, setDraft, busy, error, run, reread } = useOemPerformanceOptions(active, revision, onSettled);
  const dirty = !!profile && !!draft && JSON.stringify(profile.current) !== JSON.stringify(draft);
  const turboChanged = !!profile && !!draft && profile.current.turboFan !== draft.turboFan;
  const canApply = profile?.currentMode === 2 && !profile.softwareActive && !profile.blockedReason && (!turboChanged || !profile.customFan)
    && (draft?.dtt !== true || profile.dttDriverAvailable === true);
  return <Surface as="section" variant="group" className="ccc-panel ccc-keyboard-editor" aria-label="性能选项">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="performance" />性能选项</h2><Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={reread}>重新读取性能选项</Button></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {!profile || !draft ? <p className="ccc-control-caption">{error ? "暂时无法读取，请重试。" : "正在读取性能选项。"}</p> : <>
      <div className="ccc-setting-tiles">{PERFORMANCE_FIELDS.map(({ key, label }) => <Surface as="div" variant="inset" className="ccc-keyboard-setting" key={key}>
        <div><strong>{label}</strong>
          <p className="ccc-control-caption">当前{performanceState(profile.current[key])}{draft[key] !== profile.current[key] ? ` → ${performanceState(draft[key])}` : ""}</p></div>
        <Switch label={label} checked={draft[key] ?? false} disabled={busy || !!profile.blockedReason || profile.current[key] === null || profile.currentMode !== 2 || profile.softwareActive
          || key === "turboFan" && profile.customFan || key === "dtt" && profile.dttDriverAvailable !== true && profile.current.dtt !== true}
          onChange={value => setDraft(previous => previous ? { ...previous, [key]: value } : previous)} />
      </Surface>)}</div>
      {profile.currentMode !== 2 && <p className="ccc-control-caption">当前为{OEM_POWER_MODES[profile.currentMode] ?? "未知模式"}，切换到性能模式后可修改。</p>}
      {profile.current.dtt !== null && profile.dttDriverAvailable !== true && <p className="ccc-control-caption">{profile.dttDriverAvailable === false ? "未发现 Dynamic Tuning 驱动，不能启用 DTT。" : "驱动状态未确认，不能启用 DTT。"}</p>}
      {draft.dtt === true && profile.dttDriverAvailable !== true && <p className="ccc-control-caption">目标保留开启的 DTT；请确认驱动或先关闭 DTT，再应用其他选项。</p>}
      {profile.customFan && <p className="ccc-control-caption">退出自定义固件风扇曲线后可修改 Turbo 风扇。</p>}
      {profile.softwareActive && <p className="ccc-control-caption">退出软件风扇曲线后可修改性能选项。</p>}
      {profile.blockedReason && <p className="ccc-control-caption">{profile.blockedReason}</p>}
      {turboChanged && <p className="ccc-control-caption" role="status" aria-label="性能选项联动预览">应用 Turbo 风扇变更时，风扇切回自动。</p>}
      <div className="ccc-actions">
        <ConfirmButton key={JSON.stringify([profile.revision, draft, profile.blockedReason])} label="应用性能选项" confirmLabel="确认应用性能选项？" busy={busy} disabled={!dirty || !canApply}
          onConfirm={() => run(() => applyOemPerformanceOptions(profile.revision, draft), "性能选项已发送，保存值已回读")} />
      </div>
      {profile.restore && <OemModeRestore target={profile.restore} currentMode={profile.currentMode} currentAcMode={profile.acMode} currentEnergy={profile.energy}
        busy={busy} disabled={!profile.canRestore || !!profile.blockedReason} blockedReason={profile.restoreBlockedReason} label="恢复模式与选项"
        onRestore={revision => run(() => restoreOemPowerMode(revision), "初始模式与选项已恢复")} />}
      <Collapsible summary="性能选项说明与诊断"><p>Turbo 控制原厂风扇加速；保存值回读表示设置已接受，实际调校与散热效果需另行核对。恢复与原厂模式共用初始快照。</p>
        {PERFORMANCE_FIELDS.map(field => <p key={field.key}><strong>{field.label}</strong> · {field.description}</p>)}
        {profile.warnings.map(warning => <p key={warning}>{warning}</p>)}</Collapsible>
    </>}
  </Surface>;
}
