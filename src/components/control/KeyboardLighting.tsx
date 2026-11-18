import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { Button } from "../Button";
import { applyKeyboardLighting, restoreKeyboardLighting } from "../../api/client";
import { ConfigurationAction } from "../ConfigurationAction";
import { FactList } from "../FactList";
import { DetailsDialog } from "../DetailsDialog";
import { ScaleSlider } from "./ScaleSlider";
import { KeyboardEffectEditor } from "./KeyboardEffectEditor";
import { EFFECTS, hexOf, useKeyboardLighting } from "./useKeyboardLighting";
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
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="lighting" />键盘灯光</h2>
      <div className="ccc-module-tools"><DetailsDialog title="键盘灯诊断" label="详情">
        {profile?.warnings.map(warning => <p key={warning}>{warning}</p>)}
        <p>类型 {profile?.detectedType ?? "未知"}{profile?.detectionSource && ` · ${profile.detectionSource}`}</p>
        {profile?.unavailableReason && <p>{profile.unavailableReason}</p>}
      </DetailsDialog>
      <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取键盘灯" disabled={busy} onClick={state.reload} />
      </div>
    </div>
    {error && <p role="alert" className="ccc-control-error">{error}</p>}
    {!profile?.available ? <>
      {profile?.unavailableReason && <p role="status" className="ccc-control-caption">{profile.unavailableReason}</p>}
    </> : <>
      {unsupported ? <span className="ccc-chip is-warn">{profile.detectedFamily === "unknown" ? "未能识别键盘灯类型" : "未检测到键盘背光"}</span> : <>
        {family === "zoneRgb" && <KeyboardEffectEditor effect={effect} setEffect={state.setEffect} allColor={state.allColor} setAllColor={state.setAllColor}
          zones={state.zones} setZones={state.setZones} brightness={brightness} disabled={disabled} />}
        <div className={family === "white" ? "ccc-display-board" : undefined}>
          {family === "white" && <KeyboardBacklightVisual label={state.dirty ? "目标亮度" : "当前保存亮度"} maximum={5}
            level={state.dirty ? brightness : profile.current?.brightnessLevel ?? null}
            enabled={state.dirty ? brightness > 0 : profile.current?.enabled ?? null} />}
          <ScaleSlider label="亮度" unit="级" min={0} max={family === "white" ? 5 : 4} current={profile.current?.enabled ? profile.current.brightnessLevel : 0}
            draft={brightness} disabled={disabled || !family} onChange={state.setBrightness} />
        </div>
        <ConfigurationAction label="键盘灯光" revision={JSON.stringify([profile.current, request])}
          disabled={disabled || !request || !state.dirty} onConfirm={() => request ? run(() => applyKeyboardLighting(request)) : undefined}>
          <FactList compact facts={[
            { label: "亮度", value: `${profile.current?.enabled ? `${profile.current.brightnessLevel} 级` : "关闭"} → ${brightness > 0 ? `${brightness} 级` : "关闭"}` },
            ...(family === "zoneRgb" ? [{ label: "灯效", value: `${nameOf(profile.current?.effect ?? null)} → ${nameOf(effect)}` }] : []),
          ]} />
          {family === "zoneRgb" && <KeyboardColorFacts effect={effect} allColor={state.allColor} zones={state.zones} />}
        </ConfigurationAction>
        <ConfigurationAction label="键盘灯光" revision={JSON.stringify([profile.current, profile.baseline])} restore
          disabled={busy || !profile.baseline} onConfirm={() => run(restoreKeyboardLighting)}>
          <p>恢复首次修改前的键盘灯效、亮度和颜色。</p>
          {profile.baseline && <FactList compact facts={[{ label: "亮度", value: profile.baseline.enabled ? `${profile.baseline.brightnessLevel} 级` : "关闭" }]} />}
          {profile.baseline && family === "zoneRgb" && <KeyboardColorFacts effect={profile.baseline.effect} allColor={profile.baseline.allColor} zones={profile.baseline.zones} />}
        </ConfigurationAction>
      </>}
    </>}
  </Surface>;
}

function KeyboardColorFacts({ effect, allColor, zones }: Pick<ReturnType<typeof useKeyboardLighting>, "effect" | "allColor" | "zones">) {
  const color = (c: typeof allColor) => hexOf(c);
  return <FactList compact facts={effect === "zones"
    ? zones.map((zone, index) => ({ label: ["左区", "中区", "右区"][index], value: `${zone.enabled ? "" : "关闭 · "}${color(zone.color)}` }))
    : [{ label: "颜色", value: color(allColor) }]} />;
}
