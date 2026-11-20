import { useEffect, useId, useRef, useState } from "react";
import { applyDisplayRefresh, applyDisplayRotation, cancelDisplayTrial, keepDisplayMode, readDisplayModes, restoreDisplayModes, type DisplayMode, type DisplayPending } from "../../api/client";
import { Button } from "../Button";
import { ConfirmButton } from "../ConfirmButton";
import { DetailsDialog } from "../DetailsDialog";
import { Icon } from "../Icon";
import { OemConflictAction } from "../OemConflictAction";
import { Select } from "../Select";
import { Surface } from "../Surface";
import { Switch } from "../Switch";
import { DisplayModeVisual, displayDirection } from "./DisplayModeVisual";
import { useDeviceSettings } from "./useDeviceSettings";

export function DisplaySettings({ active, revision, rotationRequest }: { active: boolean; revision: number; rotationRequest: number | null }) {
  const { profile, busy, error, run, reread } = useDeviceSettings(readDisplayModes, active, revision, 500);
  const [device, setDevice] = useState("");
  const handledRotation = useRef<number | null>(null);
  const rotationHint = useId();
  const pending = profile?.pending;
  const displays = profile?.displays ?? [];
  const selected = displays.find(display => display.current.device === (pending?.targets[0]?.device ?? device))
    ?? displays.find(display => display.primary) ?? displays[0];
  const rotation = profile?.rotation;
  const blocked = profile?.blockedReason;
  const canRotate = !!rotation && selected?.current.device === rotation.before.device;
  const disabled = busy || !!pending;

  useEffect(() => {
    if (!active || rotationRequest === null || handledRotation.current === rotationRequest || !rotation || pending) return;
    handledRotation.current = rotationRequest;
    setDevice(rotation.before.device);
  }, [active, rotationRequest, rotation, pending]);

  return <Surface as="section" variant="group" className="ccc-panel" aria-label="屏幕显示" aria-busy={busy || undefined}>
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="display" />屏幕显示</h2>
      {selected && <div className="ccc-module-tools"><DetailsDialog title="显示器详情" label="详情">
        <p>{selected.current.name} · {selected.current.device}</p>
        <p>支持刷新率 · {selected.frequencies.join(" / ")} Hz</p>
        {!canRotate && <p>{rotation ? "翻转只适用于主显示器。" : "当前主显示器无法翻转。"}</p>}
        <p>刷新率和主屏方向切换后可在 15 秒内保留。</p>
      </DetailsDialog></div>}
    </div>
    {(error || profile?.error) && <p role="alert" className="ccc-control-error">{error || profile?.error}</p>}
    {blocked && blocked !== error && <p className="ccc-control-caption">{blocked}</p>}
    <OemConflictAction issues={[blocked, error, profile?.error, pending?.error]} disabled={busy} onClosed={reread} />
    {profile && !displays.length && <p className="ccc-control-caption">没有可用的活动显示器。</p>}
    {selected && <div className="ccc-display-board">
      <DisplayModeVisual label={selected.primary ? "当前主屏" : "当前显示器"} mode={selected.current} />
      <div className="ccc-display-controls">
        <label className="ccc-config-field">显示器<Select label="显示器" value={selected.current.device} disabled={disabled}
          options={displays.map((display, index) => ({ value: display.current.device,
            label: `${display.primary ? "主屏" : `屏幕 ${index + 1}`} · ${display.current.name}` }))} onChange={setDevice} /></label>
        <label className="ccc-config-field">刷新率<Select label="目标刷新率" value={String(selected.current.frequency)} disabled={disabled || !!blocked}
          options={selected.frequencies.map(frequency => ({ value: String(frequency), label: `${frequency} Hz` }))}
          onChange={value => {
            const frequency = Number(value);
            if (frequency !== selected.current.frequency && selected.frequencies.includes(frequency)) {
              void run(() => applyDisplayRefresh(selected.current, frequency));
            }
          }} /></label>
        <div className="ccc-keyboard-setting">
          <div><strong>翻转主屏</strong>
            {canRotate && <p id={rotationHint} className="ccc-control-caption">
              切换至{displayDirection(rotation.target.orientation)}
              {rotation.before.width !== rotation.target.width && ` · ${rotation.target.width} × ${rotation.target.height}`}
            </p>}
          </div>
          <Switch label="翻转主屏" describedBy={canRotate ? rotationHint : undefined} checked={selected.current.orientation === 2 && !!selected.primary}
            disabled={disabled || !canRotate || !!blocked} onChange={() => { if (canRotate) void run(() => applyDisplayRotation(rotation.before)); }} />
        </div>
      </div>
    </div>}
    {pending && <DisplayTrial pending={pending} current={selected?.current} busy={busy} run={run} />}
    {!selected && <Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={reread}>重新读取屏幕</Button>}
    {profile?.canRestore && !pending && <div className="ccc-actions">
      <ConfirmButton key={JSON.stringify(profile.displays)} size="compact" icon="undo" label="恢复初始设置" confirmLabel="确认恢复"
        busy={busy} disabled={!!blocked} onConfirm={() => run(restoreDisplayModes)}><p>恢复首次切换前的显示模式。</p></ConfirmButton>
    </div>}
  </Surface>;
}

function DisplayTrial({ pending, current, busy, run }: {
  pending: DisplayPending; current?: DisplayMode; busy: boolean; run: (action: () => Promise<void>) => Promise<void>;
}) {
  if (pending.automatic || pending.profileSession != null) {
    const host = pending.automatic ? "自动刷新率试用" : "档案显示确认";
    return <p className="ccc-control-caption" role="status">请在「{host}」中保留或恢复本次试用</p>;
  }
  const { trialId, remainingMs, targets, rotation, error } = pending;
  const target = targets[0];
  const destination = rotation && target ? displayDirection(target.orientation)
    : targets.length === 1 ? `${target.frequency} Hz`
    : targets.map(mode => `${mode.name} ${mode.frequency} Hz`).join("、");
  const recovery = remainingMs == null ? "恢复待处理" : `${Math.max(0, Math.ceil(remainingMs / 1000))} 秒后自动恢复`;
  const mayKeep = current && targets.length === 1 && target.device === current.device && !error && remainingMs != null && remainingMs > 0;
  return <div className="ccc-display-trial">
    <p className="ccc-control-caption" role="status">试用 {destination} · {recovery}</p>
    <div className="ccc-actions">
      <Button size="compact" disabled={busy || trialId == null || !mayKeep}
        onClick={() => { if (trialId != null && mayKeep) void run(() => keepDisplayMode(trialId, current)); }}>
        {rotation ? "保留屏幕方向" : "保留刷新率"}
      </Button>
      <Button icon="undo" variant="secondary" size="compact" disabled={busy || trialId == null}
        onClick={() => { if (trialId != null) void run(() => cancelDisplayTrial(trialId)); }}>取消本次试用</Button>
    </div>
  </div>;
}
