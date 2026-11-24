import { Icon } from "../Icon";
import type { ProfileSettings, ProfileDisplayReview } from "../../api/client";
import { Surface } from "../Surface";
import { DisplayModeVisual } from "../control/DisplayModeVisual";

export function ProfileDisplayPreview({ target, review }: { target: ProfileSettings["display"]; review: ProfileDisplayReview | null }) {
  if (!target) return null;
  return <section className="ccc-profile-values" aria-label="档案显示配置"><h3 className="ccc-icon-title"><Icon name="display" />屏幕刷新率</h3>
    <span className="ccc-control-caption is-warn">整组试用15秒 · 需保留确认 · 开机同样确认</span>
    {target.displays.map(mode => {
      const current = review?.displays.find(change => change.target.device === mode.device)?.before;
      return <Surface variant="group" className="ccc-profile-device" key={mode.device}>
        <div><strong>{mode.name}</strong><p className="ccc-control-caption">{mode.width} × {mode.height} · {mode.bits} 位</p></div>
        <div className="ccc-profile-values"><span className="ccc-control-caption">{current ? `切换前 ${current.frequency} Hz → 目标` : "当前未读取 · 档案目标"}</span><DisplayModeVisual mode={mode} label="档案目标" /></div>
      </Surface>;
    })}
  </section>;
}
