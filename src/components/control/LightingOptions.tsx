import { Icon } from "../Icon";
import { applyLightingOptions, restoreLightingOptions, type KeyboardZone, type LightingOptionsValues } from "../../api/client";
import { hexOf } from "../../lib/color";
import { Button } from "../Button";
import { ColorPicker } from "../ColorPicker";
import { ConfigurationAction } from "../ConfigurationAction";
import { FactList } from "../FactList";
import { Surface } from "../Surface";
import { Switch } from "../Switch";
import { DetailsDialog } from "../DetailsDialog";
import type { useLightingOptions } from "./useLightingOptions";
import { TopCoverEditor } from "./TopCoverEditor";
import { LightingIdleTimer } from "./LightingIdleTimer";
import { booleanText, timerText, zoneText, topCoverText, idleTimerSeconds, lightingOptionChanges, type LightingOptionsDraft } from "./lighting-option-values";

function PeripheralLight({ title, current, draft, disabled, onChange, onToggle }: {
  title: string; current: KeyboardZone; draft: KeyboardZone; disabled: boolean;
  onChange: (value: KeyboardZone) => void; onToggle: (enabled: boolean) => void;
}) {
  const color = hexOf(current.color);
  const difference = [current.enabled !== draft.enabled ? booleanText(current.enabled) : null,
    color !== hexOf(draft.color) ? color.toUpperCase() : null].filter(Boolean).join(" · ");
  return <Surface as="div" variant="inset" className="ccc-lighting-option">
    <div className="ccc-keyboard-setting">
      <strong>{title}</strong>
      <Switch label={`${title}开关`} checked={current.enabled} disabled={disabled} onChange={onToggle} />
    </div>
    <div className="ccc-control-status">
      {difference && <p className="ccc-control-caption">当前{difference}</p>}
      <ColorPicker label={`${title}颜色`} value={draft.color} disabled={disabled} onChange={color => onChange({ ...draft, color })} swatchOnly />
    </div>
  </Surface>;
}

export function LightingOptions({ model }: { model: ReturnType<typeof useLightingOptions> }) {
  const { profile, draft, setDraft, busy, error, run, reread } = model;
  const update = (patch: Partial<LightingOptionsDraft>) => setDraft(previous => previous ? { ...previous, ...patch } : previous);
  const timer = draft?.idleTimer;
  const seconds = timer ? idleTimerSeconds(timer) : null;
  const values: LightingOptionsValues | null = draft && timer && seconds !== null && (!timer.enabled || seconds >= 5)
    ? { ...draft, idleTimer: { enabled: timer.enabled, seconds } } : null;
  const compatible = !!profile && !!draft
    && (draft.bootOverride === null) === (profile.current.bootOverride === null)
    && (draft.logo === null) === (profile.current.logo === null) && (draft.lightBar === null) === (profile.current.lightBar === null)
    && (draft.topCover === null) === (profile.current.topCover === null)
    && (draft.topCover === null || Number.isInteger(draft.topCover.brightness) && draft.topCover.brightness >= 0 && draft.topCover.brightness <= 4);
  const dirty = !!profile && !!values && JSON.stringify(profile.current) !== JSON.stringify(values);
  const request = profile && values ? { expectedRevision: profile.revision, values } : null;
  const apply = (patch: Partial<LightingOptionsValues>) => {
    if (!profile || busy) return;
    void run(() => applyLightingOptions({ expectedRevision: profile.revision, values: { ...profile.current, ...patch } }), "灯光选项已回读", false);
  };
  return <Surface as="section" variant="group" className="ccc-panel ccc-lighting-options" aria-label="灯光策略与外围灯区">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="effects" />灯光策略与外围灯区</h2>
      <div className="ccc-module-tools">
        <DetailsDialog title="灯光策略说明" label="详情"><p>开启「闲置关灯」后，一段时间没有操作，灯光会自动熄灭。等待时间可以设为 5 秒到 1 小时 59 分 59 秒。</p></DetailsDialog>
        <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取灯光选项" disabled={busy} onClick={reread} />
      </div>
    </div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {!profile || !draft || !timer ? <p className="ccc-control-caption">无法读取灯光选项，请重新读取。</p> : <>
      <div className="ccc-lighting-options-grid">
        <LightingIdleTimer timer={timer} seconds={seconds} disabled={busy} onChange={idleTimer => update({ idleTimer })}
          onToggle={enabled => apply({ idleTimer: { enabled, seconds: enabled ? Math.max(5, profile.current.idleTimer.seconds) : profile.current.idleTimer.seconds } })} />
        {draft.bootOverride !== null && <Surface as="div" variant="inset" className="ccc-lighting-option">
          <div className="ccc-keyboard-setting">
            <strong>开机灯效覆盖</strong>
            <Switch label="覆盖开机灯效" checked={draft.bootOverride} disabled={busy} onChange={bootOverride => apply({ bootOverride })} />
          </div>
        </Surface>}
        {draft.logo && profile.current.logo && <PeripheralLight title="Logo 灯" current={profile.current.logo} draft={draft.logo} disabled={busy}
          onChange={logo => update({ logo })} onToggle={enabled => apply({ logo: { ...profile.current.logo!, enabled } })} />}
        {draft.lightBar && profile.current.lightBar && <PeripheralLight title="灯条" current={profile.current.lightBar} draft={draft.lightBar} disabled={busy}
          onChange={lightBar => update({ lightBar })} onToggle={enabled => apply({ lightBar: { ...profile.current.lightBar!, enabled } })} />}
        {draft.topCover && profile.current.topCover && <TopCoverEditor current={profile.current.topCover} draft={draft.topCover} disabled={busy} onChange={topCover => update({ topCover })} />}
      </div>
      {!profile.capabilities.peripheralSupportKnown && <p className="ccc-control-caption">外围灯区能力未知。</p>}
      {!compatible && <p className="ccc-control-error" role="alert">请按当前设备支持的项目重新编辑。</p>}
      <ConfigurationAction label="灯光策略与外围灯区" applyLabel="保存灯光参数" revision={JSON.stringify(request)} disabled={busy || !compatible || !dirty}
        onConfirm={() => request ? run(() => applyLightingOptions(request), "灯光选项已写入") : undefined}>
        {values && <FactList compact facts={lightingOptionChanges(profile.current, values)} />}
      </ConfigurationAction>
      <ConfigurationAction label="灯光策略与外围灯区" revision={profile.revision} restore disabled={busy || !profile.canRestore}
        onConfirm={() => run(() => restoreLightingOptions(profile.revision), "灯光选项已恢复")}>
      {profile.initial && <p className="ccc-control-caption">恢复目标：{timerText(profile.initial.idleTimer)}
        {profile.initial.bootOverride !== null ? `；开机灯效覆盖${booleanText(profile.initial.bootOverride)}` : ""}
        {profile.initial.topCover ? `；独立顶盖${topCoverText(profile.initial.topCover)}` : ""}
        {profile.initial.logo ? `；Logo 灯${zoneText(profile.initial.logo)}` : ""}{profile.initial.lightBar ? `；灯条${zoneText(profile.initial.lightBar)}` : ""}</p>}
      <p>只还原这些选项，保留当前键盘灯效。</p></ConfigurationAction>
    </>}
  </Surface>;
}
