import { Icon } from "../Icon";
import { applyPdLevel, restorePdLevel } from "../../api/client";
import { Button } from "../Button";
import { ConfigurationAction } from "../ConfigurationAction";
import { Select } from "../Select";
import { Surface } from "../Surface";
import { usePlatformPower } from "./usePlatformPower";
import { pdLevelText as levelText } from "./platform-values";
import { PowerSourceVisual } from "./PowerSourceVisual";
import { DetailsDialog } from "../DetailsDialog";

export function PlatformPower({ active, revision }: { active: boolean; revision: number }) {
  const { profile, draft, setDraft, loaded, busy, error, run, reread } = usePlatformPower(active, revision);
  const pd = profile?.powerDelivery;
  if (loaded && profile && !pd && profile.deepS5 === null && !profile.warnings.length && !error) return null;
  const valid = !!pd && pd.currentValid && draft !== null && pd.availableLevels.includes(draft);
  const request = pd && draft !== null ? { revision: pd.revision, level: draft } : null;
  return <Surface as="section" variant="group" className={`ccc-panel${pd ? " ccc-platform-panel" : ""}`} aria-label="平台电源">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="plug" />平台电源</h2>
      <div className="ccc-module-tools"><DetailsDialog title="平台电源说明" label="详情">
        <p>适配器功率由固件报告。PD 档位与 CPU 功耗上限分别设置。</p>
        {profile?.deepS5 != null && <p>关机节能 · {profile.deepS5 ? "已启用" : "未启用"}</p>}
        {pd?.restoreBlockedReason && <p>{pd.restoreBlockedReason}</p>}
        {profile?.warnings.map(warning => <p key={warning}>{warning}</p>)}
      </DetailsDialog>
      <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取平台电源" disabled={busy} onClick={reread} /></div></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {!profile ? <p className="ccc-control-caption">{error ? "无法读取，请重试。" : "正在读取平台电源。"}</p> : <>
      {profile.warnings.length > 0 && <p className="ccc-control-caption">部分电源信息不可用 · 查看详情</p>}
      {pd && <div className="ccc-platform-pd">
        <div className="ccc-platform-current">
        <PowerSourceVisual source="adapter" watts={pd.adapterWatts} label="PD 适配器报告功率" />
        <span className="ccc-control-caption">当前 · {pd.currentValid ? levelText(pd.currentLevel, pd.zeroBased) : "档位未知"}</span>
        {pd.blockedReason && <p className="ccc-control-caption">{pd.blockedReason}</p>}
        </div>
        <div className="ccc-display-controls">
        <label className="ccc-config-field">目标 PD 档位
          <Select label="PD 档位" value={draft === null ? "" : String(draft)} disabled={busy || !!pd.blockedReason || !pd.currentValid}
            placeholder="选择档位" options={pd.availableLevels.map(value => ({ value: String(value), label: levelText(value, pd.zeroBased) }))}
            onChange={value => setDraft(Number(value))} /></label>
        {!pd.currentValid && <p className="ccc-control-caption">当前等级不在候选中，暂不能应用新设置；已有初始快照仍可恢复。</p>}
        {draft !== null && !pd.availableLevels.includes(draft) && <p className="ccc-control-caption">此适配器不支持当前草稿，请重新选择。</p>}
          <ConfigurationAction label="PD 档位" revision={JSON.stringify(request)} disabled={busy || !valid || draft === pd.currentLevel || !!pd.blockedReason}
            onConfirm={() => request ? run(() => applyPdLevel(request.revision, request.level), "PD 档位已写入并回读") : undefined}>
            <p>{levelText(pd.currentLevel, pd.zeroBased)} → {draft !== null && levelText(draft, pd.zeroBased)}</p>
          </ConfigurationAction>
          <ConfigurationAction label="PD 档位" revision={pd.revision} restore disabled={busy || !pd.initial || !!pd.restoreBlockedReason || !!pd.blockedReason}
            onConfirm={() => run(() => restorePdLevel(pd.revision), "PD 初始档位已恢复")}>
            {pd.initial && <p>恢复至{levelText(pd.initial.value, pd.initial.zeroBased)}。</p>}
          </ConfigurationAction>
        </div>
      </div>}
      {profile.deepS5 !== null && <Surface as="div" variant="inset" className="ccc-keyboard-setting">
        <strong>Deep S5</strong>
        <span>{profile.deepS5 ? "已启用" : "未启用"}</span>
      </Surface>}
    </>}
  </Surface>;
}
