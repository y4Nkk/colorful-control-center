import type { PdEventProfile, PowerDeliveryProfile } from "../../api/client";
import { FactList, FactStatus } from "../FactList";
import { Surface } from "../Surface";
import { pdLevelText } from "./platform-values";

export function PdEventFacts({ profile }: { profile: PdEventProfile }) {
  const pd = profile.current;
  return <section className="ccc-pd-event-facts" aria-label="原厂 PD 事件事实与预览">
    <strong>原厂 PD 档位事件</strong>
    <div className="ccc-pd-event-columns">
      <Surface variant="inset" className="ccc-pd-event-part">
        <strong>当前设备</strong>
        {pd ? <FactList compact facts={[
          { label: "当前档位", icon: "plug", tone: pd.currentValid ? undefined : "warning",
            value: pd.currentValid ? pdLevelText(pd.currentLevel, pd.zeroBased) : <FactStatus tone="warning">未识别</FactStatus> },
          { label: "适配器报告功率", icon: "power", format: pd.adapterWatts === null ? undefined : "number",
            value: pd.adapterWatts === null ? "未报告" : `${pd.adapterWatts} W` },
          { label: "可选档位", icon: "settings", value: pd.availableLevels.map(level => pdLevelText(level, pd.zeroBased)).join("、") },
        ]} /> : <p className="ccc-control-caption">PD 状态当前不可读取。</p>}
        {profile.error && <p className="ccc-control-error">{profile.error}</p>}
      </Surface>
      <Surface variant="inset" className="ccc-pd-event-part">
        <strong>事件目标与恢复</strong>
        {pd ? <>
          <FactList label="PD 事件目标" compact facts={[{ label: "触发后设置", icon: "plug", value: pdLevelText(1, pd.zeroBased) }]} />
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
    <FactList label="PD 档位恢复" compact facts={[{ label: "恢复接管前的档位", icon: "undo",
      value: `${pd.currentValid ? pdLevelText(pd.currentLevel, pd.zeroBased) : "未知"} → ${pdLevelText(pd.initial.value, pd.initial.zeroBased)}` }]} />
    {pd.restoreBlockedReason && <p className="ccc-control-error">{pd.restoreBlockedReason}</p>}
  </>;
}
