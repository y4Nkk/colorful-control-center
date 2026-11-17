import { Icon } from "../Icon";
import { applyLightBar, restoreLightBar } from "../../api/client";
import { Button } from "../Button";
import { Collapsible } from "../Collapsible";
import { ConfirmButton } from "../ConfirmButton";
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
  return <Surface as="section" variant="group" className="ccc-panel ccc-light-bar-panel" aria-label="独立灯条">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="lighting" />独立灯条</h2>
      {profile && <span className="ccc-control-caption">{profile.kind === "x58" ? "X58" : "X170"}</span>}
      <Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={model.reread}>重新读取独立灯条</Button></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {profile ? <div className="ccc-light-bar-columns">
      <Surface variant="inset" className="ccc-light-bar-section">
        <strong>当前保存配置</strong><p className="ccc-control-caption">{lightBarSummary(current ?? null)}</p>
        {profile.initial && <p className="ccc-control-caption">恢复目标：{lightBarSummary(profile.initial)}</p>}
        {profile.blockedReason && <p className="ccc-control-caption">{profile.blockedReason}</p>}
        <Collapsible summary="灯条诊断">{profile.warnings.map(warning => <p key={warning}>{warning}</p>)}</Collapsible>
        <div className="ccc-actions"><ConfirmButton icon="undo" key={"restore-" + profile.revision} variant="secondary" label="恢复初始灯条" confirmLabel="确认恢复修改前的独立灯条？" busy={busy}
          disabled={!profile.initial || !!profile.blockedReason} onConfirm={() => run(() => restoreLightBar(profile.revision), "独立灯条初始配置已恢复")} /></div>
      </Surface>
      <Surface variant="inset" className="ccc-light-bar-section">
        {sameKind && current && draft ? <>
          {current.kind === "x170" && draft.kind === "x170" && <X170LightBarEditor current={current.settings} draft={draft.settings} busy={busy}
            presets={profile.presets.flatMap(value => value.kind === "x170" ? [value.settings] : [])}
            onChange={settings => model.setDraft({ kind: "x170", settings })} />}
          {current.kind === "x58" && draft.kind === "x58" && <X58LightBarEditor current={current.settings} draft={draft.settings} busy={busy}
            presets={profile.presets.flatMap(value => value.kind === "x58" ? [value.settings] : [])}
            onChange={settings => model.setDraft({ kind: "x58", settings })} />}
          {dirty && <p className="ccc-control-caption" aria-label="独立灯条变更预览">{lightBarSummary(current)} → {lightBarSummary(draft)}</p>}
          <div className="ccc-actions"><ConfirmButton key={profile.revision + JSON.stringify(draft)} label="应用独立灯条" confirmLabel="确认应用这些灯条设置？" busy={busy} disabled={!dirty || blocked || !lightBarDraftValid(draft)}
            onConfirm={() => run(() => applyLightBar(profile.revision, draft), "独立灯条已发送并核对保存值")} /></div>
        </> : current ? <>
          <p className="ccc-control-caption">灯条型号已变化，旧草稿已保留；载入当前配置后再编辑。</p>
          <Button variant="secondary" disabled={busy} onClick={() => model.setDraft(structuredClone(current))}>载入当前灯条草稿</Button>
        </> : <p className="ccc-control-caption">保存值未知，未开放新设置。已有有效初始快照时仍可确认恢复。</p>}
      </Surface>
    </div> : <p className="ccc-control-caption">{error ? "无法读取，请重试。" : "正在读取灯条配置。"}</p>}
  </Surface>;
}
