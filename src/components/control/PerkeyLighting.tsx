import { Icon } from "../Icon";
import { useEffect, useRef } from "react";
import { Surface } from "../Surface";
import { Button } from "../Button";
import { Select } from "../Select";
import { Switch } from "../Switch";
import { ConfirmButton } from "../ConfirmButton";
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
      {draft && <Switch label="逐键灯光开关" checked={draft.enabled} disabled={busy} onChange={enabled => model.setDraft({ ...draft, enabled })} />}</div>
    {model.error && <p className="ccc-control-error" role="alert">{model.error}</p>}
    {profile?.reactiveError && <p className="ccc-control-error" role="alert">{profile.reactiveError}</p>}
    {!profile || !draft ? <Button icon="refresh" disabled={busy} onClick={model.reload}>重新读取逐键灯</Button> : <>
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
      <div className="ccc-device-preview ccc-control-caption"><span>当前设置：{perkeySummary(profile.current)}</span>
        {model.dirty && <span>待应用：{perkeySummary(draft)}</span>}</div>
      <div className="ccc-actions">
        <ConfirmButton key={JSON.stringify(model.request)} label="应用逐键" confirmLabel="确认应用逐键灯效？" busy={busy} disabled={!model.dirty || !model.request} onConfirm={model.apply} />
        <ConfirmButton icon="undo" variant="secondary" label="恢复逐键初始" confirmLabel="恢复原始灯效与逐键颜色？" busy={busy} disabled={!profile.canRestore} onConfirm={model.restore} />
        <Button icon="refresh" variant="ghost" disabled={busy} onClick={model.reload}>重新读取</Button>
      </div>
    </>}
  </Surface>;
}
