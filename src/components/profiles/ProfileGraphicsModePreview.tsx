import { Icon } from "../Icon";
import type { ProfileSettings, ProfileGraphicsModeReview } from "../../api/client";
import { Surface } from "../Surface";
import { FactList } from "../FactList";

const mode = (value: number) => ({ 1: "集显", 2: "独显", 3: "混合", 4: "动态" })[value] ?? "未知";
export function ProfileGraphicsModePreview({ target, review }: { target: ProfileSettings["graphicsMode"]; review: ProfileGraphicsModeReview | null }) {
  if (!target) return null;
  const previous = review?.before.lastRequest;
  const pending = previous?.verification === "pendingReboot";
  const known = pending ? previous.target : review?.before.current?.mode;
  const changes = !!review && known !== target.mode;
  return <Surface as="section" variant="group" className="ccc-profile-device" aria-label="档案显卡模式">
    <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="gpu" />显卡模式 · MUX</h3><strong>{mode(target.mode)}</strong>
      {(changes || pending) && <span className="ccc-control-caption is-warn">需重启生效 · 单独确认重启</span>}
      {changes && review.softwareFanRunning && <p className="ccc-control-caption">发送时短暂释放软件风扇，随后恢复本次曲线。</p>}
      {previous?.verification === "uncertain" && <p className="ccc-control-error">此前选择未确认，请先恢复。</p>}
    </div>
    <FactList compact facts={[
      { label: "启动报告", value: review?.before.current ? mode(review.before.current.mode) : "未读取" },
      { label: "待重启选择", value: !review ? "未读取" : previous?.verification === "pendingReboot" ? mode(previous.target) : "无" },
      { label: "档案选择", value: mode(target.mode) },
    ]} />
  </Surface>;
}
