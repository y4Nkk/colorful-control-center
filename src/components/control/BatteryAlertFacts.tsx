import { restoreBatteryAlertAudio, type BatteryAlertProfile } from "../../api/client";
import { ConfirmButton } from "../ConfirmButton";
import { Icon } from "../Icon";
import { Surface } from "../Surface";

type Props = { profile: BatteryAlertProfile; busy: boolean; run: (action: () => Promise<void>, message: string, reset: boolean) => Promise<void> };
export function BatteryAlertFacts({ profile, busy, run }: Props) {
  const { codec, binding } = profile;
  return <section aria-label="原厂电池音频事实与预览" className="ccc-battery-low-facts">
    <strong className="ccc-icon-title"><Icon name="battery" />原厂电池音频事件</strong>
    <div className="ccc-battery-low-columns">
      <Surface variant="inset" className="ccc-battery-low-part">
        <strong className="ccc-icon-title"><Icon name={profile.playing ? "play" : "pause"} />静音循环</strong>
        <output>{profile.playing ? "运行中" : "已停止"}</output>
        <p className="ccc-control-caption">亮度独立恢复</p>
        {profile.error && <p className="ccc-control-error">{profile.error}</p>}
      </Surface>
      <Surface variant="inset" className="ccc-battery-low-part">
        <strong className="ccc-icon-title"><Icon name={codec.current?.muted ? "volume-off" : "volume"} />PC Beep控件</strong>
        <output>{codec.current ? codec.current.muted ? "静音" : "未静音" : binding?.pcBeep === false ? "固件未声明" : "状态不可用"}</output>
        {codec.initial && <p className="ccc-control-caption">初始：{codec.initial.muted ? "静音" : "未静音"}</p>}
        {codec.error && <p className="ccc-control-error">{codec.error}</p>}
        {codec.restoreError && <p className="ccc-control-error">{codec.restoreError}</p>}
        {codec.initial && <ConfirmButton icon="undo" key={codec.restoreRevision ?? "audio-unavailable"} variant="secondary"
          label="恢复初始电池音频" confirmLabel="停止监听并恢复初始电池音频？" busy={busy} disabled={!codec.restoreRevision}
          onConfirm={() => run(() => restoreBatteryAlertAudio(codec.restoreRevision!), "初始电池音频已恢复", false)} />}
      </Surface>
    </div>
  </section>;
}
