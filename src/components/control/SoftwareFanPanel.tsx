import { SegmentedControl } from "../SegmentedControl";
import { Button } from "../Button";
import { useEffect, useRef, useState } from "react";
import { FAN_LABELS, applySoftwareFanCurves, setFanMode,
  type FanProfile, type SoftwareCurve, type SoftwareCurves } from "../../api/client";
import { ConfigurationAction } from "../ConfigurationAction";
import { toast } from "../use-toast";
import { PointNumber } from "./CurvePointInputs";
import { DiffPreview } from "./DiffPreview";
import { SoftwareCurveEditor } from "./SoftwareCurveEditor";
import { defaultSoftwareCurve, sameSoftware, softwareChanges } from "./softwareCurve";
import { ParameterHelp } from "./ParameterHelp";
import { SOFTWARE_FAN_HELP } from "./parameterDescriptions";

export function SoftwareFanPanel({ visible, readValid, profile, refresh, onBusy }: {
  visible: boolean; readValid: boolean; profile: FanProfile; refresh: () => Promise<void>; onBusy: (busy: boolean) => void;
}) {
  const [draft, setDraft] = useState<SoftwareCurves>(() => profile.software ?? { cpu: defaultSoftwareCurve(), gpu1: defaultSoftwareCurve() });
  const [tab, setTab] = useState<"cpu" | "gpu1">("cpu");
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null);
  const edited = useRef(false), running = useRef(false);
  const appliedKey = JSON.stringify(profile.software);
  useEffect(() => {
    if (!edited.current && profile.software) setDraft(profile.software);
  }, [appliedKey, profile.software]);
  const curve = draft[tab];
  const change = (next?: SoftwareCurve) => {
    edited.current = true;
    setDraft((before) => { const after = { ...before }; if (next) after[tab] = next; else delete after[tab]; return after; });
  };
  const run = async (action: () => Promise<{ ok: boolean; message: string }>) => {
    if (running.current) return;
    running.current = true; setBusy(true); onBusy(true); setError(null);
    try {
      const result = await action();
      if (!result.ok) throw new Error(result.message);
      edited.current = false; await refresh();
      toast({ kind: "success", title: "风扇控制已更新" });
    } catch (cause) {
      let message = String(cause);
      try { await refresh(); } catch (read) { message += `；回读失败：${String(read)}`; }
      setError(message);
    } finally { running.current = false; setBusy(false); onBusy(false); }
  };
  const dirty = !sameSoftware(profile.software, draft) && Object.keys(draft).length > 0;
  const changes = softwareChanges(profile.software, draft);
  return <div className="ccc-fan-content ccc-software-fan">
    <SegmentedControl className="ccc-software-fan-selector" label="软件曲线风扇" value={tab} disabled={busy}
      options={(["cpu", "gpu1"] as const).map(id => ({ value: id, label: FAN_LABELS[id] }))}
      onChange={value => setTab(value as typeof tab)} />
    <span className="ccc-plot-preview">按 CPU / GPU 较高温度调节 · 应用运行期间生效</span>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {profile.softwareBlockedReason && <p className="ccc-control-caption">{profile.softwareBlockedReason}</p>}
    {curve ? <>
      <SoftwareCurveEditor key={tab} current={profile.software?.[tab]?.points ?? null} points={curve.points} disabled={busy}
        onChange={(points) => change({ ...curve, points })} />
      <div className="ccc-curve-point ccc-software-response">
        <PointNumber label="回差 °C" name="软件曲线回差" value={curve.hysteresisC} min={0} max={10} disabled={busy} onChange={(hysteresisC) => change({ ...curve, hysteresisC })} />
        <PointNumber label="斜坡 %/秒" name="软件曲线斜坡" value={curve.rampPctPerS} min={1} max={100} disabled={busy} onChange={(rampPctPerS) => change({ ...curve, rampPctPerS })} />
        <Button variant="ghost" size="compact" type="button" disabled={busy || Object.keys(draft).length <= 1} onClick={() => change()}>交还固件</Button>
      </div>
    </> : <div className="ccc-actions"><span className="ccc-control-caption">由固件控制</span><Button icon="add" variant="secondary" type="button" disabled={busy} onClick={() => change(defaultSoftwareCurve())}>添加曲线</Button></div>}
    <ConfigurationAction label="软件曲线" revision={JSON.stringify([profile, draft])}
      disabled={!visible || busy || !readValid || !dirty || !!profile.softwareBlockedReason} onConfirm={() => run(() => applySoftwareFanCurves(draft))}>
      <DiffPreview items={changes} />
      <p>确认后软件接管所选风扇，应用运行期间生效。</p>
    </ConfigurationAction>
    <ConfigurationAction label="自动风扇控制" revision={JSON.stringify(profile)} restore
      disabled={busy || !profile.software || !!profile.blockedReason} onConfirm={() => run(() => setFanMode(0))}>
      <p>结束软件曲线，交还固件自动控制。</p>
    </ConfigurationAction>
    <ParameterHelp title="软件曲线参数说明" items={SOFTWARE_FAN_HELP} />
  </div>;
}
