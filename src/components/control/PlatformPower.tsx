import { Icon } from "../Icon";
import { applyPdLevel, restorePdLevel } from "../../api/client";
import { Button } from "../Button";
import { ConfirmButton } from "../ConfirmButton";
import { FactList } from "../FactList";
import { Select } from "../Select";
import { Surface } from "../Surface";
import { usePlatformPower } from "./usePlatformPower";
import { pdLevelText as levelText } from "./platform-values";
import { PowerSourceVisual } from "./PowerSourceVisual";
import { Collapsible } from "../Collapsible";

export function PlatformPower({ active, revision }: { active: boolean; revision: number }) {
  const { profile, draft, setDraft, loaded, busy, error, run, reread } = usePlatformPower(active, revision);
  const pd = profile?.powerDelivery;
  if (loaded && profile && !pd && profile.deepS5 === null && !profile.warnings.length && !error) return null;
  const valid = !!pd && pd.currentValid && draft !== null && pd.availableLevels.includes(draft);
  const request = pd && draft !== null ? { revision: pd.revision, level: draft } : null;
  return <Surface as="section" variant="group" className={`ccc-panel${pd ? " ccc-platform-panel" : ""}`} aria-label="平台电源">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="plug" />平台电源</h2>
      <Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={reread}>重新读取平台电源</Button></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {!profile ? <p className="ccc-control-caption">{error ? "无法读取，请重试。" : "正在读取平台电源。"}</p> : <>
      {profile.warnings.map(warning => <p className="ccc-control-error" role="alert" key={warning}>{warning}</p>)}
      {pd && <div className="ccc-platform-pd">
        <div className="ccc-platform-current">
        <PowerSourceVisual source="adapter" watts={pd.adapterWatts} label="PD 适配器报告功率" />
        <FactList compact facts={[
          { label: "当前 PD 档位", value: pd.currentValid ? levelText(pd.currentLevel, pd.zeroBased) : "当前等级未识别" },
        ]} />
        {pd.initial && <p className="ccc-control-caption">恢复目标：{levelText(pd.initial.value, pd.initial.zeroBased)}</p>}
        {pd.restoreBlockedReason && <p className="ccc-control-caption">{pd.restoreBlockedReason}</p>}
        {pd.blockedReason && <p className="ccc-control-caption">{pd.blockedReason}</p>}
        </div>
        <Surface as="div" variant="inset" className="ccc-platform-target">
        <label className="ccc-platform-level">PD 档位
          <Select label="PD 档位" value={draft === null ? "" : String(draft)} disabled={busy || !!pd.blockedReason || !pd.currentValid}
            placeholder="选择档位" options={pd.availableLevels.map(value => ({ value: String(value), label: levelText(value, pd.zeroBased) }))}
            onChange={value => setDraft(Number(value))} /></label>
        {request && draft !== pd.currentLevel && <p className="ccc-control-caption">{pd.currentValid ? levelText(pd.currentLevel, pd.zeroBased) : "当前等级未识别"} → {levelText(draft!, pd.zeroBased)}</p>}
        {!pd.currentValid && <p className="ccc-control-caption">当前等级不在候选中，暂不能应用新设置；已有初始快照仍可恢复。</p>}
        {draft !== null && !pd.availableLevels.includes(draft) && <p className="ccc-control-caption">此适配器不支持当前草稿，请重新选择。</p>}
        <div className="ccc-actions">
          <ConfirmButton key={JSON.stringify({ request, blocked: pd.blockedReason })} label="应用 PD 档位" confirmLabel="确认应用 PD 档位？" busy={busy}
            disabled={!valid || draft === pd.currentLevel || !!pd.blockedReason}
            onConfirm={() => request ? run(() => applyPdLevel(request.revision, request.level), "PD 档位已写入并回读") : undefined} />
          <ConfirmButton icon="undo" key={`restore-${pd.revision}-${pd.blockedReason}-${pd.restoreBlockedReason}`} variant="secondary" label="恢复 PD 初始" confirmLabel="恢复本会话修改前的 PD 档位？" busy={busy}
            disabled={!pd.initial || !!pd.restoreBlockedReason || !!pd.blockedReason}
            onConfirm={() => run(() => restorePdLevel(pd.revision), "PD 初始档位已恢复")} />
        </div>
        </Surface>
      </div>}
      {profile.deepS5 !== null && <Surface as="div" variant="inset" className="ccc-keyboard-setting">
        <strong>Deep S5</strong>
        <span>{profile.deepS5 ? "已启用" : "未启用"}</span>
      </Surface>}
      <Collapsible summary="平台电源说明"><p>档位来自固件，与CPU功耗上限分别设置。适配器功率是固件报告，不能由档位换算。Deep S5仅显示固件报告的关机节能状态。</p></Collapsible>
    </>}
  </Surface>;
}
