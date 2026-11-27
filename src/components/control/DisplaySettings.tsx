import { useEffect, useRef, useState } from "react";
import { applyDisplayRefresh, applyDisplayRotation, cancelDisplayTrial, keepDisplayMode, readDisplayModes, type DisplayMode, type DisplayPending } from "../../api/client";
import { Button } from "../Button";
import { DetailsDialog } from "../DetailsDialog";
import { Icon } from "../Icon";
import { Loading } from "../Loading";
import { OemConflictAction } from "../OemConflictAction";
import { Select } from "../Select";
import { Surface } from "../Surface";
import { Switch } from "../Switch";
import { DisplayModeVisual, displayDirection } from "./DisplayModeVisual";
import { useDeviceSettings } from "./useDeviceSettings";

export function DisplaySettings({ active, revision, rotationRequest }: { active: boolean; revision: number; rotationRequest: number | null }) {
  const { profile, loaded, busy, error, run, reread } = useDeviceSettings(readDisplayModes, active, revision, 500);
  const [device, setDevice] = useState("");
  const handledRotation = useRef<number | null>(null);
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
        <p className="ccc-control-caption">{selected.current.name} · {selected.current.device}</p>
        <p className="ccc-control-caption">支持刷新率 · {selected.frequencies.join(" / ")} Hz</p>
        {!canRotate && <p className="ccc-control-caption">{rotation ? "翻转只适用于主显示器。" : "当前主显示器无法翻转。"}</p>}
        <p>切换刷新率或翻转主屏后有 15 秒试用时间，期间可以保留，否则自动恢复。</p>
      </DetailsDialog></div>}
    </div>
    {(error || profile?.error) && <p role="alert" className="ccc-control-error">{error || profile?.error}</p>}
    {blocked && blocked !== error && <p className="ccc-control-caption">{blocked}</p>}
    <OemConflictAction issues={[blocked, error, profile?.error, pending?.error]} disabled={busy} onClosed={reread} />
    {profile && !displays.length && <p className="ccc-control-caption">没有可用的活动显示器。</p>}
    {selected && <div className="ccc-display-board ccc-display-settings">
      <DisplayModeVisual label={selected.primary ? "当前主屏" : "当前显示器"} mode={selected.current} showLabel={false} />
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
      </div>
    </div>}
    {pending && <DisplayTrial pending={pending} current={selected?.current} busy={busy} run={run} />}
    {selected ? <div className="ccc-display-rotation">
      <div className="ccc-display-rotation-label"><strong>翻转主屏</strong>
        <DetailsDialog title="翻转主屏说明">
          {canRotate ? <p>主屏当前是 {displayDirection(rotation.before.orientation)} · {rotation.before.width} × {rotation.before.height}。
            {selected.current.orientation === 2 ? "关闭开关后" : "打开开关后"}切换为 {displayDirection(rotation.target.orientation)}
            {rotation.before.width !== rotation.target.width && ` · ${rotation.target.width} × ${rotation.target.height}`}。</p>
            : <p>{rotation ? "翻转只适用于主屏。在上方「显示器」里选中主屏后，才能使用这个开关。" : "当前没有可翻转的主显示器。"}</p>}
          <p>翻转会把主屏画面旋转 180°，让屏幕上下颠倒显示。刷新率和屏幕位置保持不变。</p>
          <p>切换后屏幕有 15 秒的试用时间：想保留新方向，点「保留屏幕方向」；不操作或点「取消本次试用」，屏幕会自动恢复到切换前的方向。</p>
          <p className="ccc-control-caption">只对主屏生效，外接显示器不受影响。</p>
          <p className="ccc-control-caption">翻转只改变当前屏幕状态，不会保存到 Windows 的显示设置，也不会写入配置档案。</p>
          <p className="ccc-control-caption">在「原厂热键与屏显」里勾选了「原厂屏幕旋转」时，按下快捷键只会跳到这个开关，不会自行翻转。</p>
        </DetailsDialog>
      </div>
      <Switch label="翻转主屏" checked={selected.current.orientation === 2 && !!selected.primary}
        disabled={disabled || !canRotate || !!blocked} onChange={() => { if (canRotate) void run(() => applyDisplayRotation(rotation.before)); }} />
    </div> : loaded ? <Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={reread}>重新读取屏幕</Button>
      : <Loading label="正在读取屏幕状态…" />}
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
