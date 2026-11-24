import { Icon } from "../Icon";
import { applyLightBar, restoreLightBar, type LightBarValues } from "../../api/client";
import { Button } from "../Button";
import { DetailsDialog } from "../DetailsDialog";
import { ConfigurationAction } from "../ConfigurationAction";
import { OemConflictAction } from "../OemConflictAction";
import { Surface } from "../Surface";
import { X170LightBarEditor, X58LightBarEditor } from "./LightBarEditor";
import { lightBarSummary, lightBarDraftValid } from "./lightbar-values";
import { useLightBarSettings } from "./useLightBarSettings";

export function LightBarSettings({ active, revision }: { active: boolean; revision: number }) {
  const model = useLightBarSettings(active, revision), { profile, draft, busy, error, run } = model;
  if (model.loaded && !profile && !error) return null;
  const current = profile?.current, blocked = busy || !!profile?.blockedReason;
  const sameKind = !!current && !!draft && current.kind === profile?.kind && draft.kind === current.kind;
  const dirty = sameKind && JSON.stringify(current) !== JSON.stringify(draft);
  const apply = (target: LightBarValues) => {
    if (!profile || !sameKind || blocked || !lightBarDraftValid(target)) return;
    void run(() => applyLightBar(profile.revision, target), "独立灯条已回读", false);
  };
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="独立灯条">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="lighting" />独立灯条</h2>
      <div className="ccc-module-tools">{profile && <DetailsDialog title="灯条诊断" label="详情">
        <p className="ccc-control-caption">灯条型号 {profile.kind.toUpperCase()}</p>{profile.warnings.map(warning => <p key={warning} className="ccc-control-caption is-warn"><Icon name="warning" size={16} />{warning}</p>)}
      </DetailsDialog>}
      <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取独立灯条" disabled={busy} onClick={model.reread} /></div>
    </div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    <OemConflictAction issues={[error, profile?.blockedReason]} disabled={busy} onClosed={model.reread} />
    {profile ? <div className="ccc-light-bar-columns">
      <Surface variant="inset" className="ccc-light-bar-section">
        <strong>当前保存配置</strong><p className="ccc-control-caption">{lightBarSummary(current ?? null, !sameKind)}</p>
        {profile.blockedReason && <p className="ccc-control-caption">{profile.blockedReason}</p>}
        <ConfigurationAction label="独立灯条" revision={profile.revision} restore disabled={blocked || !profile.initial}
          onConfirm={() => run(() => restoreLightBar(profile.revision), "独立灯条初始配置已恢复")}>
          <p>{lightBarSummary(profile.initial)}</p>
        </ConfigurationAction>
      </Surface>
      <Surface variant="inset" className="ccc-light-bar-section">
        {sameKind && current && draft ? <>
          {current.kind === "x170" && draft.kind === "x170" && <X170LightBarEditor current={current.settings} draft={draft.settings} busy={busy} blocked={blocked}
            presets={profile.presets.flatMap(value => value.kind === "x170" ? [value.settings] : [])}
            onChange={settings => model.setDraft({ kind: "x170", settings })} onApply={settings => apply({ kind: "x170", settings })} />}
          {current.kind === "x58" && draft.kind === "x58" && <X58LightBarEditor current={current.settings} draft={draft.settings} busy={busy} blocked={blocked}
            revision={profile.revision}
            presets={profile.presets.flatMap(value => value.kind === "x58" ? [value.settings] : [])}
            onChange={settings => model.setDraft({ kind: "x58", settings })} onApply={settings => apply({ kind: "x58", settings })} />}
          <ConfigurationAction label="独立灯条" applyLabel="应用灯条参数" revision={JSON.stringify([profile.revision, draft])} disabled={!dirty || blocked || !lightBarDraftValid(draft)}
            onConfirm={() => run(() => applyLightBar(profile.revision, draft), "独立灯条已发送并核对保存值")}>
            <p>{lightBarSummary(current)} → {lightBarSummary(draft)}</p>
          </ConfigurationAction>
        </> : current ? <>
          <p className="ccc-control-caption">灯条型号已变化，旧草稿已保留；载入当前配置后再编辑。</p>
          <Button variant="secondary" disabled={busy} onClick={() => model.setDraft(structuredClone(current))}>载入当前灯条草稿</Button>
        </> : <p className="ccc-control-caption">保存值未知，未开放新设置。已有有效初始快照时仍可确认恢复。</p>}
      </Surface>
    </div> : <p className="ccc-control-caption">{error ? "无法读取，请重试。" : "正在读取灯条配置。"}</p>}
  </Surface>;
}
