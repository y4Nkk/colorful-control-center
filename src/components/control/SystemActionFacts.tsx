import type { LidActionProfile, SystemActionKind } from "../../api/client";
import { Icon, type IconName } from "../Icon";
import { Surface } from "../Surface";

const labels: Record<SystemActionKind, string> = { sleep: "睡眠", hibernate: "休眠", screenOff: "关闭屏幕", shutdown: "关机" };
const icons: Record<SystemActionKind, IconName> = { sleep: "sleep", hibernate: "sleep", screenOff: "display", shutdown: "shutdown" };

export function SystemActionFacts({ profile }: { profile: LidActionProfile }) {
  const current = profile.current;
  return <section className="ccc-battery-action-facts" aria-label="原厂合盖动作预览">
    <strong>合盖后</strong>
    <div className="ccc-battery-action-columns">{(["插电", "电池"] as const).map((title, index) => {
      const action = current?.actions[index];
      return <Surface key={title} variant="inset" className="ccc-battery-action-part">
        <span><Icon name={index === 0 ? "plug" : "battery"} /> {title}</span>
        <strong><Icon name={action ? icons[action] : "pause"} /> {current ? action ? labels[action] : "不操作" : "未知"}</strong>
      </Surface>;
    })}</div>
    {profile.error && <p className="ccc-control-error" role="alert">{profile.error}</p>}
    {profile.powerError && <p className="ccc-control-error" role="alert">{profile.powerError}</p>}
  </section>;
}
