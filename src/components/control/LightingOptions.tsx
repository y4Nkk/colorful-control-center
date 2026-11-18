import { Icon } from "../Icon";
import { applyLightingOptions, restoreLightingOptions, type KeyboardZone, type LightingOptionsValues } from "../../api/client";
import { Button } from "../Button";
import { ConfigurationAction } from "../ConfigurationAction";
import { FactList } from "../FactList";
import { Input } from "../Input";
import { Surface } from "../Surface";
import { Switch } from "../Switch";
import { DetailsDialog } from "../DetailsDialog";
import { hexOf, rgbOf } from "./useKeyboardLighting";
import { useLightingOptions } from "./useLightingOptions";
import { TopCoverEditor } from "./TopCoverEditor";
import { booleanText, timerText, zoneText, topCoverText } from "./lighting-option-values";

function PeripheralLight({ title, current, draft, disabled, onChange }: {
  title: string; current: KeyboardZone; draft: KeyboardZone; disabled: boolean; onChange: (value: KeyboardZone) => void;
}) {
  return <Surface as="div" variant="inset" className="ccc-lighting-option">
    <div className="ccc-keyboard-setting"><div><strong>{title}</strong>
      <p className="ccc-control-caption">当前{zoneText(current)}</p></div>
      <Switch label={`${title}开关`} checked={draft.enabled} disabled={disabled} onChange={enabled => onChange({ ...draft, enabled })} /></div>
    <label className="ccc-color-picker">{title}颜色
      <Input type="color" aria-label={`${title}颜色`} value={hexOf(draft.color)} disabled={disabled}
        onChange={event => onChange({ ...draft, color: rgbOf(event.target.value) })} /></label>
  </Surface>;
}

export function LightingOptions({ active, revision }: { active: boolean; revision: number }) {
  const { profile, draft, setDraft, busy, error, run, reread } = useLightingOptions(active, revision);
  const update = (patch: Partial<LightingOptionsValues>) => setDraft(previous => previous ? { ...previous, ...patch } : previous);
  const timer = draft?.idleTimer;
  const valid = !!profile && !!draft && timer != null && Number.isInteger(timer.seconds) && timer.seconds >= (timer.enabled ? 5 : 0) && timer.seconds <= 7199
    && (draft.bootOverride === null) === (profile.current.bootOverride === null)
    && (draft.logo === null) === (profile.current.logo === null) && (draft.lightBar === null) === (profile.current.lightBar === null)
    && (draft.topCover === null) === (profile.current.topCover === null)
    && (draft.topCover === null || Number.isInteger(draft.topCover.brightness) && draft.topCover.brightness >= 0 && draft.topCover.brightness <= 4);
  const dirty = !!profile && !!draft && JSON.stringify(profile.current) !== JSON.stringify(draft);
  const request = profile && draft ? { expectedRevision: profile.revision, values: draft } : null;
  const preview = profile && draft ? [
    ...(timerText(profile.current.idleTimer) !== timerText(draft.idleTimer) ? [{ label: "闲置关灯", value: `${timerText(profile.current.idleTimer)} → ${timerText(draft.idleTimer)}` }] : []),
    ...(draft.bootOverride !== null && profile.current.bootOverride !== null && draft.bootOverride !== profile.current.bootOverride ? [{ label: "开机灯效覆盖", value: `${booleanText(profile.current.bootOverride)} → ${booleanText(draft.bootOverride)}` }] : []),
    ...(draft.logo && profile.current.logo && zoneText(draft.logo) !== zoneText(profile.current.logo) ? [{ label: "Logo 灯", value: `${zoneText(profile.current.logo)} → ${zoneText(draft.logo)}` }] : []),
    ...(draft.lightBar && profile.current.lightBar && zoneText(draft.lightBar) !== zoneText(profile.current.lightBar) ? [{ label: "灯条", value: `${zoneText(profile.current.lightBar)} → ${zoneText(draft.lightBar)}` }] : []),
    ...(draft.topCover && profile.current.topCover && topCoverText(draft.topCover) !== topCoverText(profile.current.topCover) ? [{ label: "独立顶盖", value: `${topCoverText(profile.current.topCover)} → ${topCoverText(draft.topCover)}` }] : []),
  ] : [];
  return <Surface as="section" variant="group" className="ccc-panel ccc-lighting-options" aria-label="灯光策略与外围灯区">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="effects" />灯光策略与外围灯区</h2>
      <Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={reread}>重新读取灯光选项</Button></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {!profile || !draft || !timer ? <p className="ccc-control-caption">{error ? "无法读取灯光选项，请重新读取。" : "正在读取灯光选项。"}</p> : <>
      <div className="ccc-lighting-options-grid">
        {draft.topCover && profile.current.topCover && <TopCoverEditor current={profile.current.topCover} draft={draft.topCover} disabled={busy} onChange={topCover => update({ topCover })} />}
        <Surface as="div" variant="inset" className="ccc-lighting-option">
          <div className="ccc-keyboard-setting"><strong>闲置关灯</strong>
            <Switch label="启用闲置关灯" checked={timer.enabled} disabled={busy}
              onChange={enabled => update({ idleTimer: { enabled, seconds: enabled ? Math.max(5, timer.seconds) : timer.seconds } })} /></div>
          <label className="ccc-lighting-time">等待秒数
            <Input type="number" aria-label="闲置等待秒数" value={timer.seconds} min={timer.enabled ? 5 : 0} max={7199} step={1}
              disabled={busy || !timer.enabled} onChange={event => update({ idleTimer: { ...timer, seconds: Number(event.target.value) } })} /></label>
        </Surface>
        {draft.bootOverride !== null && <Surface as="div" variant="inset" className="ccc-keyboard-setting">
          <strong>开机灯效覆盖</strong>
          <Switch label="覆盖开机灯效" checked={draft.bootOverride} disabled={busy} onChange={bootOverride => update({ bootOverride })} />
        </Surface>}
        {draft.logo && profile.current.logo && <PeripheralLight title="Logo 灯" current={profile.current.logo} draft={draft.logo} disabled={busy} onChange={logo => update({ logo })} />}
        {draft.lightBar && profile.current.lightBar && <PeripheralLight title="灯条" current={profile.current.lightBar} draft={draft.lightBar} disabled={busy} onChange={lightBar => update({ lightBar })} />}
      </div>
      {!profile.capabilities.peripheralSupportKnown && <p className="ccc-control-caption">外围灯区能力未知。</p>}
      {!valid && <p className="ccc-control-error" role="alert">时间须为5–7199秒；设备能力变化后请按当前支持的项目重新编辑。</p>}
      <ConfigurationAction label="灯光策略与外围灯区" revision={JSON.stringify(request)} disabled={busy || !valid || !dirty}
        onConfirm={() => request ? run(() => applyLightingOptions(request), "灯光选项已写入") : undefined}>
        <FactList compact facts={preview} />
      </ConfigurationAction>
      <ConfigurationAction label="灯光策略与外围灯区" revision={profile.revision} restore disabled={busy || !profile.canRestore}
        onConfirm={() => run(() => restoreLightingOptions(profile.revision), "灯光选项已恢复")}>
      {profile.initial && <p className="ccc-control-caption">恢复目标：{timerText(profile.initial.idleTimer)}
        {profile.initial.bootOverride !== null ? `；开机灯效覆盖${booleanText(profile.initial.bootOverride)}` : ""}
        {profile.initial.topCover ? `；独立顶盖${topCoverText(profile.initial.topCover)}` : ""}
        {profile.initial.logo ? `；Logo 灯${zoneText(profile.initial.logo)}` : ""}{profile.initial.lightBar ? `；灯条${zoneText(profile.initial.lightBar)}` : ""}</p>}
      <p>只还原这些选项，保留当前键盘灯效。</p></ConfigurationAction>
      <DetailsDialog title="灯光策略说明"><p>闲置关灯可设为 5–7199 秒。</p></DetailsDialog>
    </>}
  </Surface>;
}
