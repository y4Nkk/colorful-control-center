import type { LidActionProfile, SystemActionKind } from "../../api/client";
import { Icon, type IconName } from "../Icon";
import { FactList } from "../FactList";

const labels: Record<SystemActionKind, string> = { sleep: "睡眠", hibernate: "休眠", screenOff: "关闭屏幕", shutdown: "关机" };
const icons: Record<SystemActionKind, IconName> = { sleep: "sleep", hibernate: "sleep", screenOff: "display", shutdown: "shutdown" };

export function SystemActionFacts({ profile }: { profile: LidActionProfile }) {
  const current = profile.current;
  return <section className="ccc-battery-action-facts" aria-label="原厂合盖动作预览">
    <strong>合盖后</strong>
    <FactList label="合盖动作" layout="tiles" compact facts={(["插电", "电池"] as const).map((label, index) => {
      const action = current?.actions[index];
      return { label, icon: index === 0 ? "plug" : "battery", value: <span className="ccc-rule-heading">
        <Icon name={action ? icons[action] : "pause"} size={16} />{current ? action ? labels[action] : "不操作" : "未知"}
      </span> };
    })} />
    {profile.error && <p className="ccc-control-error" role="alert"><Icon name="error" size={16} />{profile.error}</p>}
    {profile.powerError && <p className="ccc-control-error" role="alert"><Icon name="error" size={16} />{profile.powerError}</p>}
  </section>;
}
