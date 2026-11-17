import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { Select } from "../Select";
import { Button } from "../Button";
import { useCallback, useEffect, useRef, useState } from "react";
import { applyDisplayRefresh, applyDisplayRotation, cancelDisplayTrial, keepDisplayMode, readDisplayModes, restoreDisplayModes, type DisplayProfile } from "../../api/client";
import { ConfirmButton } from "../ConfirmButton";
import { DisplayModeVisual } from "./DisplayModeVisual";

const direction = (orientation: number) => ["横向", "纵向", "横向（翻转）", "纵向（翻转）"][orientation] ?? "未知方向";

function useDisplaySettings(active: boolean, revision: number, rotationRequest: number | null) {
  const [profile, setProfile] = useState<DisplayProfile | null>(null);
  const [device, setDevice] = useState(""), [frequency, setFrequency] = useState<number | null>(null);
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null);
  const [readError, setReadError] = useState<string | null>(null);
  const running = useRef(false), generation = useRef(0);
  const handledRotation = useRef<number | null>(null);
  const reload = useCallback(async () => {
    const id = ++generation.current;
    let next: DisplayProfile;
    try { next = await readDisplayModes(); } catch (e) { if (id === generation.current) { setProfile(null); setReadError(String(e)); throw e; } else return; }
    if (id !== generation.current) return;
    setReadError(null); setProfile(next); setDevice(previous => next.pending?.targets[0]?.device ?? (next.displays.some(d => d.current.device === previous) ? previous : next.displays[0]?.current.device ?? ""));
  }, []);
  useEffect(() => {
    if (!active) return;
    let cancelled = false, timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      if (!running.current) { try { await reload(); } catch { /* reload owns the read error and invalid snapshot. */ } }
      if (!cancelled) timer = setTimeout(poll, 500);
    };
    void poll(); return () => { cancelled = true; clearTimeout(timer); ++generation.current; };
  }, [active, revision, reload]);
  useEffect(() => {
    if (rotationRequest === null || handledRotation.current === rotationRequest || !profile || profile.pending) return;
    const primary = profile.displays.filter(display => display.primary);
    if (primary.length !== 1) return;
    setDevice(primary[0].current.device); setFrequency(null); handledRotation.current = rotationRequest;
  }, [profile, rotationRequest]);
  const selected = profile?.displays.find(d => d.current.device === device);
  const draft = frequency ?? selected?.current.frequency ?? 0;
  const run = async (action: () => Promise<void>) => {
    if (running.current) return;
    running.current = true; ++generation.current; setBusy(true); setError(null);
    try { await action(); await reload(); setFrequency(null); }
    catch (e) { setError(String(e)); await reload().catch(readError => setError(previous => `${previous}；回读失败：${readError}`)); }
    finally { running.current = false; setBusy(false); }
  };
  return { profile, device, selected, draft, busy, error: error ?? readError, run, reload,
    select: (value: string) => { setDevice(value); setFrequency(null); }, setFrequency };
}
export function DisplaySettings({ active, revision, rotationRequest }: { active: boolean; revision: number; rotationRequest: number | null }) {
  const state = useDisplaySettings(active, revision, rotationRequest);
  const { profile, selected, draft, busy, error, run } = state;
  const pending = profile?.pending;
  const uniquePrimary = profile?.displays.filter(display => display.primary).length === 1;
  const rotation = profile?.rotation && profile.rotation.before.device === selected?.current.device ? profile.rotation : null;
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="屏幕显示">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="display" />屏幕显示</h2><span className="ccc-control-caption">{selected ? `${selected.current.width} × ${selected.current.height}` : "未读取"}</span></div>
    {(error || profile?.error) && <p role="alert" className="ccc-control-error">{error || profile?.error}</p>}
    {profile && profile.displays.length === 0 && <p className="ccc-control-caption">没有可用的活动显示器。</p>}
    {selected && <>
      <label className="ccc-lighting-mode ccc-display-device-label">显示器<Select label="显示器" value={state.device} disabled={busy || !!pending}
        options={profile?.displays.map(d => ({ value: d.current.device, label: `${d.current.name} · ${d.current.device}` })) ?? []}
        onChange={state.select} /></label>
      <div className="ccc-device-preview"><span>当前 <strong>{selected.current.frequency} Hz</strong></span><span>→</span><label>目标<Select label="目标刷新率" value={String(draft)} disabled={busy || !!pending}
        options={selected.frequencies.map(f => ({ value: String(f), label: `${f} Hz` }))}
        onChange={value => state.setFrequency(Number(value))} /></label></div>
      <div className="ccc-display-orientation" aria-label="显示旋转预览">
        <Surface variant="inset" className="ccc-display-state"><DisplayModeVisual label="当前方向" mode={selected.current} /></Surface>
        <Surface variant="inset" className="ccc-display-state"><DisplayModeVisual label="目标方向" mode={rotation?.target ?? null} /></Surface>
      </div>
      {(!selected.primary || !uniquePrimary) && <p className="ccc-control-caption">{uniquePrimary ? "原厂旋转只作用于主显示器。" : "主显示器状态不明确，无法旋转。"}</p>}
      {pending && <p className="ccc-control-caption" role="status">{pending.rotation ? `试用 ${direction(pending.targets[0].orientation)}` : pending.targets.length === 1 ? `试用 ${pending.targets[0].frequency} Hz` : pending.targets.map(target => `${target.name} ${target.frequency} Hz`).join("、")} · {pending.remainingMs == null ? "已保留，等待档案结果" : `${Math.max(0, Math.ceil(pending.remainingMs / 1000))} 秒后自动恢复`}{pending.profileSession != null && "；请在档案显示确认中保留或恢复"}{pending.automatic && "；请在自动刷新率试用中确认"}</p>}
      <div className="ccc-actions">
        {pending && !pending.automatic ? <Button disabled={busy || pending.trialId == null || pending.error != null || pending.remainingMs == null || pending.remainingMs <= 0 || pending.profileSession != null || pending.targets.length !== 1} onClick={() => pending.trialId != null && run(() => keepDisplayMode(pending.trialId!, selected.current))}>{pending.rotation ? "保留屏幕方向" : "保留刷新率"}</Button>
          : !pending && <ConfirmButton key={JSON.stringify([selected.current, draft])} label="应用刷新率" confirmLabel={`试用 ${draft} Hz，15 秒内保留？`} busy={busy} disabled={draft === selected.current.frequency || !selected.frequencies.includes(draft)} onConfirm={() => run(() => applyDisplayRefresh(selected.current, draft))} />}
        {!pending && <ConfirmButton key={`rotation-${JSON.stringify(rotation)}`} label="旋转主屏" confirmLabel={`试用${rotation ? direction(rotation.target.orientation) : ""}，15 秒内保留？`} busy={busy} disabled={!selected.primary || !uniquePrimary || !rotation} onConfirm={() => run(() => rotation ? applyDisplayRotation(rotation.before) : Promise.reject("旋转预览不可用"))} />}
        {pending && !pending.automatic && pending.profileSession == null && <ConfirmButton icon="close" key={`cancel-${pending.trialId}`} variant="secondary" label="取消本次试用" confirmLabel="恢复本次试用前的显示模式？" busy={busy} disabled={pending.trialId == null} onConfirm={() => run(() => cancelDisplayTrial(pending.trialId!))} />}
        <ConfirmButton icon="undo" variant="secondary" label="恢复屏幕初始" confirmLabel="恢复首次切换前的显示模式？" busy={busy} disabled={!profile?.canRestore || pending?.profileSession != null || pending?.automatic} onConfirm={() => run(restoreDisplayModes)} />
      </div>
    </>}
    {!selected && <Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={() => { void state.reload().catch(() => {}); }}>重新读取屏幕</Button>}
  </Surface>;
}
