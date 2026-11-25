import {
  restoreBatteryAlertAudio, restoreBatteryThresholds, restoreGpuTurbo, restoreHotkeys,
  restoreLowBatteryBrightness, restorePdEvent, restoreScreenBrightness, type HotkeyProfile,
} from "../../api/client";
import { RestoreSettings, type RestoreOption } from "../RestoreSettings";
import { toast } from "../use-toast";
import { BatteryAudioRestorePreview } from "./BatteryAlertFacts";
import { LowBatteryBrightnessRestorePreview } from "./BatteryLowBrightnessFacts";
import { BatteryThresholdRestorePreview } from "./BatteryThresholdFacts";
import { GpuTurboRestorePreview } from "./GpuTurboFacts";
import { PdEventRestorePreview } from "./PdEventFacts";
import { ScreenBrightnessRestorePreview } from "./ScreenBrightnessFacts";
import { summary } from "./hotkey-actions";
import type { useHotkeySettings } from "./useHotkeySettings";

const TITLE = "原厂热键与屏显";
type Option = RestoreOption & { commit: (revision: string) => Promise<void>; policy?: true };
type Model = ReturnType<typeof useHotkeySettings>;

function recoveryOptions(profile: HotkeyProfile) {
  const { facts, initial } = profile;
  const { gpuTurbo, batteryThresholds, screenBrightness } = facts;
  const codec = facts.batteryAlert?.codec, pd = facts.pdEvent?.current;
  const low = facts.batteryLowBrightness?.recovery ? facts.batteryLowBrightness : facts.batteryAlert?.brightness;
  const overlaps = low?.recovery && screenBrightness?.initial.some(plan => plan.plan === low.recovery?.record.baseline.plan);
  const exclusiveGroup = overlaps ? "计划亮度" : undefined;
  const options: Option[] = [];
  // Stopping other event handlers can already clean up the audio baseline.
  if (codec?.initial) options.push({ label: "初始电池音频", revision: codec.restoreRevision,
    preview: <BatteryAudioRestorePreview codec={codec} />, commit: restoreBatteryAlertAudio });
  if (screenBrightness?.initial.length) options.push({ label: "初始屏幕计划亮度", revision: screenBrightness.restoreRevision, exclusiveGroup,
    preview: <ScreenBrightnessRestorePreview profile={screenBrightness} />, commit: restoreScreenBrightness });
  if (low?.recovery) options.push({ label: "低电量前亮度", revision: low.restoreRevision, exclusiveGroup,
    preview: <LowBatteryBrightnessRestorePreview profile={low} />, commit: restoreLowBatteryBrightness });
  if (gpuTurbo?.initial) options.push({ label: "初始 GPU 状态", revision: gpuTurbo.restoreRevision,
    preview: <GpuTurboRestorePreview profile={gpuTurbo} />, commit: restoreGpuTurbo });
  if (batteryThresholds?.recovery) options.push({ label: "初始电池阈值", revision: batteryThresholds.restoreRevision,
    preview: <BatteryThresholdRestorePreview profile={batteryThresholds} />, commit: restoreBatteryThresholds });
  if (pd?.initial) options.push({ label: "初始 PD 档位", revision: pd.blockedReason || pd.restoreBlockedReason ? null : pd.revision,
    preview: <PdEventRestorePreview profile={pd} />, commit: restorePdEvent });
  if (initial && profile.restoreRevision) options.push({ label: "初始热键策略", revision: profile.restoreRevision, policy: true,
    preview: <p>恢复初始策略 · {initial.enabled ? "启用" : "关闭"} · {summary(initial)}</p>, commit: restoreHotkeys });
  return { options, overlaps };
}

async function restoreSelected(options: Option[]) {
  const failures: string[] = [];
  let restored = 0;
  for (const { commit, revision, label, policy } of options) {
    if (revision === null) continue;
    if (policy && failures.length) {
      failures.push(`${label}：请先处理设备设置恢复失败，再恢复监听策略`);
      continue;
    }
    try { await commit(revision); restored += 1; }
    catch (error) { failures.push(`${label}：${String(error)}`); }
  }
  if (failures.length) {
    const message = `${restored ? `已恢复 ${restored} 项。` : ""}${failures.join("；")}`;
    toast({ kind: "error", title: "热键恢复未完成", description: message });
    throw new Error(message);
  }
}

export function HotkeyRecovery({ profile, busy, run }: { profile: HotkeyProfile; busy: boolean; run: Model["run"] }) {
  const { options, overlaps } = recoveryOptions(profile);
  return <RestoreSettings label={TITLE} options={options} blocked={busy}
    note={<>
      {options.some(option => !option.policy) && <p>恢复设备设置会停止本次热键监听。</p>}
      {profile.facts.batteryAlert?.codec.initial && <p>停止监听会结束静音循环，并还原本次接管的 PC 提示音。</p>}
      {overlaps && <p>两个亮度目标涉及同一个电源计划，请选择其中一项。</p>}
    </>}
    onRestore={selected => run(() => restoreSelected(selected), `已恢复 ${selected.length} 项热键设置`, selected.some(option => option.policy))} />;
}
