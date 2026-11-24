import { restoreLowBatteryBrightness, type LowBatteryBrightnessProfile } from "../../api/client";
import { ConfirmButton } from "../ConfirmButton";
import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { MetricReadout } from "./MetricReadout";

type Props = { absolute?: boolean; lowControl?: boolean; profile: LowBatteryBrightnessProfile; powerSummary: string | null; busy: boolean; run: (action: () => Promise<void>, message: string, reset: boolean) => Promise<void> };
const labels = ["插电计划亮度", "电池计划亮度"];

export function BatteryLowBrightnessFacts({ profile, powerSummary, busy, run, absolute = false, lowControl = true }: Props) {
  const { current, recovery } = profile;
  return <section className="ccc-battery-low-facts" aria-label="原厂低电量亮度事实与预览">
    <strong>{absolute ? "电池事件亮度" : "原厂低电量亮度"}</strong>
    <div className="ccc-battery-low-columns">
      <Surface variant="inset" className="ccc-battery-low-part">
        <strong>当前使用的电源计划</strong>
        {current ? <>
          <div className="ccc-battery-low-meters">{current.levels.map((value, index) =>
            <MetricReadout key={labels[index]} label={labels[index]} value={value} maximum={100} unit="%" colorVar="var(--ccc-action)" />)}</div>
          {powerSummary && <p className="ccc-control-caption">{powerSummary}</p>}
        </> : <p className="ccc-control-caption">当前计划亮度不可读取。</p>}
        {profile.error && <p className="ccc-control-error"><Icon name="error" size={16} />{profile.error}</p>}
      </Surface>
      <Surface variant="inset" className="ccc-battery-low-part">
        {absolute ? <>
          <strong>事件指定亮度</strong>
          <p className="ccc-control-caption">0–100% · 同步插电 / 电池</p>
          {lowControl && <p className="ccc-control-caption">同时满足低电量条件时，还会再调暗到电脑记录的亮度。</p>}
        </> : <>
        <strong>事件目标</strong>
        <div className="ccc-battery-low-meters">{labels.map((label, index) =>
          <MetricReadout key={label} label={label} value={profile.settingTarget?.[index] ?? null} maximum={100} unit="%" colorVar="var(--ccc-action)" />)}</div>
        <p className="ccc-control-caption">事件自然结束时恢复为 · 插电 {profile.endingTarget?.[0] ?? "—"}% · 电池 {profile.endingTarget?.[1] ?? "—"}%</p>
        </>}
      </Surface>
    </div>
    {(recovery || profile.restoreError) && <Surface variant="inset" className="ccc-battery-low-part">
      <strong>首次接管前的设置</strong>
      {recovery && <>
        <p>{labels.map((label, index) => recovery.record.levelsOwned[index]
          ? `${label} ${recovery.levels[index]}% → ${recovery.record.baseline.levels[index]}%`
          : `${label}保留`).join(" · ")}</p>
        <span className="ccc-control-caption">只恢复本软件接管时改过的亮度和电脑记录的值</span>
      </>}
      {profile.restoreError && <p className="ccc-control-error"><Icon name="error" size={16} />{profile.restoreError}</p>}
      {recovery && <ConfirmButton icon="undo" key={profile.restoreRevision ?? "low-brightness-unavailable"} variant="secondary"
        label="恢复低电量前亮度" confirmLabel="确认停止监听并恢复上述亮度？" busy={busy} disabled={!profile.restoreRevision}
        onConfirm={() => run(() => restoreLowBatteryBrightness(profile.restoreRevision!), "低电量前亮度已恢复", false)} />}
    </Surface>}
  </section>;
}
