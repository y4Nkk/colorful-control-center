import { Icon } from "../Icon";
import { applyMultiZoneLighting, restoreMultiZoneLighting } from "../../api/client";
import { Button } from "../Button";
import { ConfigurationAction } from "../ConfigurationAction";
import { Surface } from "../Surface";
import { MultiZoneColors } from "./MultiZoneColors";
import { MultiZoneEditor } from "./MultiZoneEditor";
import { multiZoneSummary, multiZoneChanges } from "./multizone-values";
import { useMultiZoneLighting } from "./useMultiZoneLighting";

export function MultiZoneLighting({ active, revision }: { active: boolean; revision: number }) {
  const model = useMultiZoneLighting(active, revision), { profile, draft, busy, run } = model;
  if (model.loaded && !profile && !model.error) return null;
  const current = profile?.current;
  const compatible = !!profile && !!draft && JSON.stringify(profile.capabilities) === JSON.stringify(draft.capabilities);
  const dirty = !!current && !!draft && JSON.stringify(current) !== JSON.stringify(draft.values);
  const disabled = busy || !!profile?.blockedReason || !compatible || !current;
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="多区键盘灯">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="lighting" />多区键盘灯</h2>
      <Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={model.reread}>重新读取多区键盘</Button></div>
    {model.error && <p className="ccc-control-error" role="alert">{model.error}</p>}
    {profile ? <div className="ccc-multi-zone-columns">
      <Surface variant="group" className="ccc-multi-zone-section">
        <strong>当前保存配置</strong>
        {current ? <><p className="ccc-control-caption">{multiZoneSummary(current)}</p><MultiZoneColors values={current} label="当前多区颜色预设" /></>
          : <p className="ccc-control-error">{profile.unavailableReason ?? "保存值未知，未开放新设置"}</p>}
        {!profile.capabilities.lightBarEnabled && <p className="ccc-control-caption">平台已关闭灯条区，保留其原有颜色。</p>}
        {profile.blockedReason && <p className="ccc-control-error">{profile.blockedReason}</p>}
        <ConfigurationAction label="多区键盘灯" revision={profile.revision} restore disabled={busy || !profile.canRestore || !!profile.blockedReason}
          onConfirm={() => run(() => restoreMultiZoneLighting(profile.revision), "多区键盘初始配置已恢复")}>
          {profile.initial ? <><p>{multiZoneSummary(profile.initial)}</p><MultiZoneColors values={profile.initial} label="初始多区颜色预设" /></>
            : <p>恢复首次修改前的多区设置。</p>}
        </ConfigurationAction>
      </Surface>
      <Surface variant="group" className="ccc-multi-zone-section">
        {current && compatible && draft ? <>
          <MultiZoneEditor current={current} draft={draft.values} disabled={disabled} onChange={values => model.setDraft({ ...draft, values })} />
          <ConfigurationAction revision={JSON.stringify([profile.revision, draft])} label="多区键盘灯"
            disabled={disabled || !dirty} onConfirm={() => run(() => applyMultiZoneLighting(profile.revision, draft), "多区命令已发送并核对保存值")}>
            <p>{multiZoneChanges(current, draft.values)}</p>
            <MultiZoneColors values={draft.values} label="目标多区颜色预设" />
          </ConfigurationAction>
        </> : current ? <><p className="ccc-control-caption">能力已变化，旧草稿保留；请载入当前配置后编辑。</p>
          <Button variant="secondary" disabled={busy} onClick={() => model.setDraft(structuredClone({ capabilities: profile.capabilities, values: current }))}>载入当前多区草稿</Button></>
          : <p className="ccc-control-caption">有效初始快照仍可用于恢复。</p>}
      </Surface>
    </div> : <p className="ccc-control-caption">{model.error ? "读取失败，请重试。" : "读取中…"}</p>}
  </Surface>;
}
