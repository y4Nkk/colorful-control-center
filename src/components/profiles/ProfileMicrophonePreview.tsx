import { Icon } from "../Icon";
import type { MicrophoneTarget } from "../../api/client";
import { Surface } from "../Surface";

export function ProfileMicrophonePreview({ target }: { target: MicrophoneTarget | null }) {
  if (!target) return null;
  return <Surface as="section" variant="group" className="ccc-profile-device" aria-label="档案麦克风">
    <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="microphone" />麦克风</h3><p className="ccc-control-caption">{target.endpoints.length} 个活动录音设备</p>
      <span className="ccc-control-caption">保存偏好 · {target.savedMuted ? "静音" : "未静音"}</span></div>
    <div className="ccc-profile-values">{target.endpoints.map((endpoint, index) => <div className="ccc-microphone-endpoint" key={endpoint.id}>
      <span>{endpoint.name || `麦克风 ${index + 1}`}</span><span className="ccc-control-caption">{endpoint.muted ? "已静音" : "未静音"}</span>
    </div>)}</div>
  </Surface>;
}
