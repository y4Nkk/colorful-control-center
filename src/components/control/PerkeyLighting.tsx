import { Icon } from "../Icon";
import { useEffect, useRef } from "react";
import { Surface } from "../Surface";
import { Button } from "../Button";
import { Select } from "../Select";
import { Switch } from "../Switch";
import { ConfigurationAction } from "../ConfigurationAction";
import { hexOf } from "./useKeyboardLighting";
import { ScaleSlider } from "./ScaleSlider";
import { PerkeyKeycaps } from "./PerkeyKeycaps";
import { PerkeyPatternEditor } from "./PerkeyPatternEditor";
import { PERKEY_NAMES, perkeySummary } from "../../lib/perkey-pattern";
import { usePerkeyLighting } from "./usePerkeyLighting";
import type { PerkeyPattern } from "../../api/client";

export function PerkeyLighting({ active, revision = 0 }: { active: boolean; revision?: number }) {
  const model = usePerkeyLighting(active, revision);
  const { profile, draft, busy } = model;
  const patterns = useRef(new Map<string, PerkeyPattern>());
  useEffect(() => { patterns.current.clear(); }, [profile?.revision]);
  const changePattern = (kind: string) => {
    if (!draft || !profile) return;
    patterns.current.set(draft.pattern.kind, draft.pattern);
    const pattern = patterns.current.get(kind) ?? profile.presets.find(p => p.kind === kind);
    if (pattern) model.setDraft({ ...draft, pattern: structuredClone(pattern) });
  };
  const eventPattern = !!draft && ["ripple", "fn", "fade"].includes(draft.pattern.kind);
  return <Surface as="section" variant="group" className="ccc-panel ccc-keyboard-panel" aria-label="逐键 RGB">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="keyboard" />逐键 RGB</h2>
      <div className="ccc-module-tools">{draft && <Switch label="逐键灯光开关" checked={draft.enabled} disabled={busy} onChange={enabled => model.setDraft({ ...draft, enabled })} />}
        <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取逐键灯" disabled={busy} onClick={model.reload} /></div>
    </div>
    {model.error && <p className="ccc-control-error" role="alert">{model.error}</p>}
    {profile?.reactiveError && <p className="ccc-control-error" role="alert">{profile.reactiveError}</p>}
    {!profile || !draft ? <p className="ccc-control-caption">{model.error ? "读取失败" : "读取中…"}</p> : <>
      <div className="ccc-perkey-options"><Select label="逐键灯效" value={draft.pattern.kind} disabled={busy}
        options={profile.presets.map(p => ({ value: p.kind, label: PERKEY_NAMES[p.kind] }))} onChange={changePattern} />
        {eventPattern && <span className="ccc-control-caption">{profile.reactiveRunning ? "按键响应运行中" : "确认应用后启动按键响应"}</span>}</div>
      {draft.pattern.kind === "static" ? <PerkeyKeycaps colors={draft.pattern.colors} device={profile.device} disabled={busy}
        onChange={colors => model.setDraft({ ...draft, pattern: { kind: "static", colors } })} /> :
        <PerkeyPatternEditor pattern={draft.pattern} advanced={profile.device.newEffects} disabled={busy}
          onChange={pattern => model.setDraft({ ...draft, pattern })} />}
      <div className="ccc-perkey-sliders">
        <ScaleSlider label="逐键亮度" unit="级" min={0} max={4} current={profile.current.brightness} draft={draft.brightness} disabled={busy}
          onChange={brightness => model.setDraft({ ...draft, brightness })} />
        {draft.pattern.kind !== "static" && draft.pattern.kind !== "fn" && <ScaleSlider label="灯效速度" unit="档" min={1} max={3}
          current={profile.current.speed + 1} draft={draft.speed + 1} disabled={busy} onChange={speed => model.setDraft({ ...draft, speed: speed - 1 })} />}
      </div>
      <p className="ccc-control-caption">已保存 · {perkeySummary(profile.current)}</p>
      <ConfigurationAction label="逐键 RGB" revision={JSON.stringify(model.request)}
        disabled={busy || !model.dirty || !model.request} onConfirm={model.apply}>
        <p>{perkeySummary(profile.current)} → {perkeySummary(draft)}</p>
        <PerkeyColorPreview current={profile.current.pattern} target={draft.pattern} />
        {eventPattern && <p className="ccc-control-caption">按键响应在应用运行时生效。</p>}
      </ConfigurationAction>
      <ConfigurationAction label="逐键 RGB" revision={profile.revision} restore disabled={busy || !profile.canRestore}
        onConfirm={model.restore}><p>恢复首次修改前的灯效、亮度、速度与逐键颜色。</p></ConfigurationAction>
    </>}
  </Surface>;
}

function PerkeyColorPreview({ current, target }: { current: PerkeyPattern; target: PerkeyPattern }) {
  if (target.kind !== "static") return target.kind === "scan" && target.colors
    ? <p>扫描色 · {target.colors.map(hexOf).join(" / ")}</p> : null;
  const colors = target.colors.map(hexOf);
  const changed = colors.map((color, index) => current.kind !== "static" || !current.colors[index] || color !== hexOf(current.colors[index]));
  return <div className="ccc-perkey-color-preview">
    <p>{changed.filter(Boolean).length} 个灯位的颜色将更新</p>
    <svg viewBox={`0 0 200 ${Math.ceil(colors.length / 20) * 10}`} role="img" aria-label="逐键灯位目标颜色">
      {colors.map((color, index) => <rect key={index} x={index % 20 * 10} y={Math.floor(index / 20) * 10}
        width="8" height="8" rx="2" fill={color} opacity={changed[index] ? 1 : .28}><title>{color}</title></rect>)}
    </svg>
  </div>;
}
