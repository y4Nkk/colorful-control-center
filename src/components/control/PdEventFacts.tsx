import type { PdEventProfile, PowerDeliveryProfile } from "../../api/client";
import { Surface } from "../Surface";
import { pdLevelText } from "./platform-values";

export function PdEventFacts({ profile }: { profile: PdEventProfile }) {
  const pd = profile.current;
  return <section className="ccc-pd-event-facts" aria-label="原厂 PD 事件事实与预览">
    <strong>原厂 PD 档位事件</strong>
    <div className="ccc-pd-event-columns">
      <Surface variant="inset" className="ccc-pd-event-part">
        <strong>当前设备</strong>
        {pd ? <dl className="ccc-pd-event-details">
          <div><dt>当前档位</dt><dd>{pd.currentValid ? pdLevelText(pd.currentLevel, pd.zeroBased) : "未识别"}</dd></div>
          <div><dt>适配器报告功率</dt><dd>{pd.adapterWatts === null ? "未报告" : `${pd.adapterWatts} W`}</dd></div>
          <div><dt>可选档位</dt><dd>{pd.availableLevels.map(level => pdLevelText(level, pd.zeroBased)).join("、")}</dd></div>
        </dl> : <p className="ccc-control-caption">PD 状态当前不可读取。</p>}
        {profile.error && <p className="ccc-control-error">{profile.error}</p>}
      </Surface>
      <Surface variant="inset" className="ccc-pd-event-part">
        <strong>事件目标与恢复</strong>
        {pd ? <>
          <p>触发后设置：{pdLevelText(1, pd.zeroBased)}</p>
          <p className="ccc-control-caption">原厂事件使用固定的档位编号；执行前会再核对一次适配器和可选档位。</p>
          <PdEventRestorePreview profile={pd} />
        </> : <p className="ccc-control-caption">目标不可用，不能确认接管。</p>}
      </Surface>
    </div>
  </section>;
}

export function PdEventRestorePreview({ profile: pd }: { profile: PowerDeliveryProfile }) {
  if (!pd.initial) return null;
  return <>
    <p className="ccc-control-caption">恢复接管前的档位：{pd.currentValid ? pdLevelText(pd.currentLevel, pd.zeroBased) : "未知"} → {pdLevelText(pd.initial.value, pd.initial.zeroBased)}</p>
    {pd.restoreBlockedReason && <p className="ccc-control-error">{pd.restoreBlockedReason}</p>}
  </>;
}
