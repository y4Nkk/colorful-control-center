import { Icon } from "../Icon";
import type { TouchpadTarget } from "../../api/client";
import { Surface } from "../Surface";

export function ProfileTouchpadPreview({ target }: { target: TouchpadTarget | null }) {
  if (!target) return null;
  return <Surface as="section" variant="group" className="ccc-profile-device" aria-label="档案触摸板">
    <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="touchpad" />触摸板</h3>
      <span className="ccc-control-caption">{target.devices.length} 个识别节点</span></div>
    <div className="ccc-profile-values"><strong>系统目标 · {target.enabled ? "启用" : "关闭"}</strong>
      <span className="ccc-control-caption">保存偏好 · {target.savedEnabled ? "启用" : "关闭"}</span>
    </div>
  </Surface>;
}
