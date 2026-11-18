import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { Select } from "../Select";
import { Button } from "../Button";
import { useCallback, useEffect, useRef, useState } from "react";
import { applyDisplayRefresh, applyDisplayRotation, cancelDisplayTrial, keepDisplayMode, readDisplayModes, restoreDisplayModes, type DisplayProfile } from "../../api/client";
import { ConfirmButton } from "../ConfirmButton";
import { ConfigurationAction } from "../ConfigurationAction";
import { DetailsDialog } from "../DetailsDialog";
import { Switch } from "../Switch";
import { DisplayModeVisual } from "./DisplayModeVisual";

const direction = (orientation: number) => ["横向", "纵向", "横向（翻转）", "纵向（翻转）"][orientation] ?? "未知方向";

function useDisplaySettings(active: boolean, revision: number, rotationRequest: number | null) {
  const [profile, setProfile] = useState<DisplayProfile | null>(null);
  const [device, setDevice] = useState(""), [frequency, setFrequency] = useState<number | null>(null);
  const [rotate, setRotate] = useState(false);
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
    setDevice(primary[0].current.device); setFrequency(null); setRotate(true); handledRotation.current = rotationRequest;
  }, [profile, rotationRequest]);
  const selected = profile?.displays.find(d => d.current.device === device);
  const draft = frequency ?? selected?.current.frequency ?? 0;
  const run = async (action: () => Promise<void>, reset?: "frequency" | "rotation" | "all") => {
    if (running.current) return;
    running.current = true; ++generation.current; setBusy(true); setError(null);
    try {
      await action(); await reload();
      if (reset === "frequency" || reset === "all") setFrequency(null);
      if (reset === "rotation" || reset === "all") setRotate(false);
    }
    catch (e) { setError(String(e)); await reload().catch(readError => setError(previous => `${previous}；回读失败：${readError}`)); }
    finally { running.current = false; setBusy(false); }
  };
  return { profile, device, selected, draft, rotate, setRotate, busy, error: error ?? readError, run, reload,
    select: (value: string) => { setDevice(value); setFrequency(null); setRotate(false); }, setFrequency };
}
export function DisplaySettings({ active, revision, rotationRequest }: { active: boolean; revision: number; rotationRequest: number | null }) {
  const state = useDisplaySettings(active, revision, rotationRequest);
  const { profile, selected, draft, busy, error, run } = state;
  const pending = profile?.pending;
  const uniquePrimary = profile?.displays.filter(display => display.primary).length === 1;
  const rotation = profile?.rotation && profile.rotation.before.device === selected?.current.device ? profile.rotation : null;
  const canRotate = !!rotation && !!selected?.primary && uniquePrimary;
  const canChangeRate = !!selected && draft !== selected.current.frequency && selected.frequencies.includes(draft);
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="屏幕显示">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="display" />屏幕显示</h2>
      {selected && <div className="ccc-module-tools"><DetailsDialog title="显示器详情" label="详情">
        <p>{selected.current.name} · {selected.current.device}</p>
        <p>支持刷新率 · {selected.frequencies.join(" / ")} Hz</p>
        <p>{uniquePrimary ? "翻转仅适用于主显示器。" : "主显示器状态不明确，无法翻转。"}</p>
      </DetailsDialog></div>}
    </div>
    {(error || profile?.error) && <p role="alert" className="ccc-control-error">{error || profile?.error}</p>}
    {profile && profile.displays.length === 0 && <p className="ccc-control-caption">没有可用的活动显示器。</p>}
    {selected && <>
      <div className="ccc-display-board">
        <DisplayModeVisual label={selected.primary ? "当前主屏" : "当前显示器"} mode={selected.current} />
        <div className="ccc-display-controls">
          <label className="ccc-config-field">显示器<Select label="显示器" value={state.device} disabled={busy || !!pending}
            options={profile?.displays.map((display, index) => ({ value: display.current.device,
              label: `${display.primary ? "主屏" : `屏幕 ${index + 1}`} · ${display.current.name}` })) ?? []} onChange={state.select} /></label>
          <label className="ccc-config-field">刷新率<Select label="目标刷新率" value={String(draft)} disabled={busy || !!pending}
            options={selected.frequencies.map(f => ({ value: String(f), label: `${f} Hz` }))}
            onChange={value => state.setFrequency(Number(value))} /></label>
          <div className="ccc-keyboard-setting"><strong>翻转主屏</strong>
            <Switch label="翻转主屏" checked={state.rotate && canRotate} disabled={busy || !!pending || !canRotate} onChange={state.setRotate} /></div>
        </div>
      </div>
      <DisplayChanges state={state} pending={pending} canChangeRate={canChangeRate} rotation={canRotate ? rotation : null} />
      {pending && <p className="ccc-control-caption" role="status">{pending.rotation ? `试用 ${direction(pending.targets[0].orientation)}` : pending.targets.length === 1 ? `试用 ${pending.targets[0].frequency} Hz` : pending.targets.map(target => `${target.name} ${target.frequency} Hz`).join("、")} · {pending.remainingMs == null ? "已保留，等待档案结果" : `${Math.max(0, Math.ceil(pending.remainingMs / 1000))} 秒后自动恢复`}{pending.profileSession != null && "；请在档案显示确认中保留或恢复"}{pending.automatic && "；请在自动刷新率试用中确认"}</p>}
      {pending && !pending.automatic && pending.profileSession == null && <div className="ccc-actions">
        <Button disabled={busy || pending.trialId == null || pending.error != null || pending.remainingMs == null || pending.remainingMs <= 0 || pending.targets.length !== 1}
          onClick={() => pending.trialId != null && run(() => keepDisplayMode(pending.trialId!, selected.current))}>{pending.rotation ? "保留屏幕方向" : "保留刷新率"}</Button>
        <ConfirmButton icon="close" key={`cancel-${pending.trialId}`} label="取消本次试用" confirmLabel="恢复本次试用前的显示模式"
          busy={busy} disabled={pending.trialId == null} onConfirm={() => run(() => cancelDisplayTrial(pending.trialId!))} />
      </div>}
    </>}
    {!selected && <Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={() => { void state.reload().catch(() => {}); }}>重新读取屏幕</Button>}
  </Surface>;
}

function DisplayChanges({ state, pending, canChangeRate, rotation }: {
  state: ReturnType<typeof useDisplaySettings>; pending: DisplayProfile["pending"] | undefined;
  canChangeRate: boolean; rotation: DisplayProfile["rotation"];
}) {
  const { selected, draft, rotate, busy, run, profile } = state;
  if (!selected) return null;
  return <>
    <ConfigurationAction label="屏幕刷新率" revision={JSON.stringify([selected.current, draft])}
      disabled={busy || !!pending || !canChangeRate} confirmLabel={`试用 ${draft} Hz`}
      onConfirm={() => run(() => applyDisplayRefresh(selected.current, draft), "frequency")}>
      <p>{selected.current.name} · {selected.current.frequency} → {draft} Hz</p>
      <p>屏幕正常显示时请在 15 秒内保留，超时自动恢复。</p>
    </ConfigurationAction>
    <ConfigurationAction label="屏幕方向" revision={JSON.stringify(rotation)} disabled={busy || !!pending || !rotate || !rotation}
      confirmLabel="试用屏幕方向" onConfirm={() => rotation ? run(() => applyDisplayRotation(rotation.before), "rotation") : undefined}>
      <div className="ccc-display-orientation" aria-label="显示旋转预览">
        <DisplayModeVisual label="当前方向" mode={rotation?.before ?? null} />
        <DisplayModeVisual label="目标方向" mode={rotation?.target ?? null} />
      </div>
      <p>屏幕正常显示时请在 15 秒内保留，超时自动恢复。</p>
    </ConfigurationAction>
    <ConfigurationAction label="屏幕显示" revision={JSON.stringify([profile?.displays, pending?.trialId])} restore
      disabled={busy || !profile?.canRestore || pending?.profileSession != null || pending?.automatic}
      onConfirm={() => run(restoreDisplayModes, "all")}>
      <p>恢复首次切换前的显示模式。</p>
    </ConfigurationAction>
  </>;
}
