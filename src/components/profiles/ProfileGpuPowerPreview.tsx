import { Icon } from "../Icon";
import type { ProfileSettings, ProfileGpuPowerReview } from "../../api/client";
import { Surface } from "../Surface";
import { FactList } from "../FactList";
import { DetailsDialog } from "../DetailsDialog";

const state = (enabled: boolean) => enabled ? "开启" : "关闭";
export function ProfileGpuPowerPreview({ target, review }: { target: ProfileSettings["gpuPower"]; review: ProfileGpuPowerReview | null }) {
  if (!target) return null;
  const { values } = target;
  const value = (key: "enabled" | "automaticPowerOff") => review
    ? `${state(review.before[key])} → ${state(values[key])}` : `目标${state(values[key])}`;
  return <Surface as="section" variant="group" className="ccc-profile-device" aria-label="档案独显配置">
    <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="gpu" />独显电源</h3><strong>{target.name}</strong>
      <span className="ccc-rule-heading"><Icon name={values.enabled ? "gpu" : "shutdown"} />{values.enabled ? "开启独显 → 核对调校能力" : "退出软件风扇 → 关断独显"}</span>
      {!values.enabled && <span className="ccc-chip is-warn">GPU监控与调校暂停</span>}
      {review?.softwareFanRunning && !values.enabled && <p className="ccc-control-caption">当前软件风扇曲线将退出；后续失败会恢复原曲线。</p>}
      <DetailsDialog title="独显设备绑定"><p className="ccc-control-caption">{target.id}</p><p>复核失败恢复本次独显状态；后续失败会恢复原曲线。</p></DetailsDialog>
    </div>
    <FactList compact facts={[{ label: "独显节点", value: value("enabled") }, { label: "自动关断策略", value: value("automaticPowerOff") }]} />
  </Surface>;
}
