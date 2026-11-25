import type { BatteryAlertProfile } from "../../api/client";
import { Icon } from "../Icon";
import { Surface } from "../Surface";

export function BatteryAlertFacts({ profile }: { profile: BatteryAlertProfile }) {
  const { codec, binding } = profile;
  return <section aria-label="原厂电池音频事实与预览" className="ccc-battery-low-facts">
    <strong className="ccc-icon-title"><Icon name="battery" />原厂电池音频事件</strong>
    <div className="ccc-battery-low-columns">
      <Surface variant="inset" className="ccc-battery-low-part">
        <strong className="ccc-icon-title"><Icon name={profile.playing ? "play" : "pause"} />静音循环</strong>
        <output>{profile.playing ? "运行中" : "已停止"}</output>
        <p className="ccc-control-caption">屏幕亮度需要单独恢复</p>
        {profile.error && <p className="ccc-control-error">{profile.error}</p>}
      </Surface>
      <Surface variant="inset" className="ccc-battery-low-part">
        <strong className="ccc-icon-title"><Icon name={codec.current?.muted ? "volume-off" : "volume"} />PC 提示音（Beep）</strong>
        <output>{codec.current ? codec.current.muted ? "静音" : "未静音" : binding?.pcBeep === false ? "电脑不支持" : "状态不可用"}</output>
        {codec.error && <p className="ccc-control-error">{codec.error}</p>}
        <BatteryAudioRestorePreview codec={codec} />
      </Surface>
    </div>
  </section>;
}

export function BatteryAudioRestorePreview({ codec }: { codec: BatteryAlertProfile["codec"] }) {
  return <>
    {codec.initial && <p className="ccc-control-caption">接管前：{codec.initial.muted ? "静音" : "未静音"}</p>}
    {codec.restoreError && <p className="ccc-control-error">{codec.restoreError}</p>}
  </>;
}
