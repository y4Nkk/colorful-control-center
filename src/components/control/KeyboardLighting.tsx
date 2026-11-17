import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { Button } from "../Button";
import { applyKeyboardLighting, restoreKeyboardLighting } from "../../api/client";
import { ConfirmButton } from "../ConfirmButton";
import { Collapsible } from "../Collapsible";
import { ScaleSlider } from "./ScaleSlider";
import { KeyboardEffectEditor } from "./KeyboardEffectEditor";
import { EFFECTS, useKeyboardLighting } from "./useKeyboardLighting";
import { PerkeyLighting } from "./PerkeyLighting";
import { KeyboardBacklightVisual } from "./KeyboardBacklightVisual";

export function KeyboardLighting({ active, revision = 0 }: { active: boolean; revision?: number }) {
  const state = useKeyboardLighting(active, revision);
  const { profile, family, brightness, effect, busy, error, request, run } = state;
  const unsupported = profile?.detectedFamily === "unknown" || profile?.detectedFamily === "none";
  const disabled = busy || !profile?.available || unsupported;
  const nameOf = (value: typeof effect) => EFFECTS.find((item) => item.id === value)?.label ?? "未知灯效";
  if (profile?.detectedFamily === "perkey") return <PerkeyLighting active={active} revision={revision} />;
  if (profile?.detectedFamily === "multiZone") return null;
  return <Surface as="section" variant="group" className="ccc-panel ccc-keyboard-panel" aria-label="键盘灯光">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="lighting" />键盘灯光</h2></div>
    {error && <p role="alert" className="ccc-control-error">{error}</p>}
    {!profile?.available ? <>
      {profile?.unavailableReason && <p role="status" className="ccc-control-caption">{profile.unavailableReason}</p>}
      <Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={state.reload}>重试读取</Button>
    </> : <>
      {unsupported ? <><span className="ccc-chip is-warn">{profile.detectedFamily === "unknown" ? "未能识别键盘灯类型" : "未检测到键盘背光"}</span>
        <Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={state.reload}>重新读取键盘灯</Button></> : <>
        {family === "white" && <div className="ccc-lighting-state-pair">
          <Surface variant="inset"><KeyboardBacklightVisual label="当前保存亮度" maximum={5} level={profile.current?.brightnessLevel ?? null} enabled={profile.current?.enabled ?? null} /></Surface>
          <Surface variant="inset"><KeyboardBacklightVisual label="待应用亮度" maximum={5} level={brightness} enabled={brightness > 0} /></Surface>
        </div>}
        {family === "zoneRgb" && <KeyboardEffectEditor effect={effect} setEffect={state.setEffect} allColor={state.allColor} setAllColor={state.setAllColor}
          zones={state.zones} setZones={state.setZones} brightness={brightness} disabled={disabled} />}
        <ScaleSlider label="亮度" unit="级" min={0} max={family === "white" ? 5 : 4} current={profile.current?.enabled ? profile.current.brightnessLevel : 0}
          draft={brightness} disabled={disabled || !family} onChange={state.setBrightness} />
        {family === "zoneRgb" && <p className="ccc-control-caption">当前 {nameOf(profile.current?.effect ?? null)} → 草稿 {nameOf(effect)}{brightness === 0 ? " · 关闭" : ""}</p>}
        <div className="ccc-actions">
          <ConfirmButton key={JSON.stringify(request)} label="应用" confirmLabel={`确认${family === "zoneRgb" ? nameOf(effect) : "白光"} / ${brightness} 级？`} disabled={disabled || !request}
            busy={busy} onConfirm={() => request ? run(() => applyKeyboardLighting(request)) : undefined} />
          <ConfirmButton icon="undo" variant="secondary" label="恢复初始" confirmLabel="恢复键盘灯初始设置？" disabled={!profile.baseline} busy={busy}
            onConfirm={() => run(restoreKeyboardLighting)} />
        </div>
      </>}
      <Collapsible summary="诊断">
        {profile.warnings.map((warning) => <p key={warning}>{warning}</p>)}
        <p>类型 {profile.detectedType ?? "未知"} · {profile.detectionSource}</p>
      </Collapsible>
    </>}
  </Surface>;
}
