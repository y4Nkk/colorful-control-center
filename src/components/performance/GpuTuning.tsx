import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { Button } from "../Button";
import { useEffect, useRef, useState } from "react";
import { applyGpuClockOffsets, readGpuProfile, restoreGpuBaseline, type GpuApplyOutcome, type GpuProfile } from "../../api/client";
import { ConfigurationAction } from "../ConfigurationAction";
import { DetailsDialog } from "../DetailsDialog";
import { ScaleSlider } from "../control/ScaleSlider";
import { DiffPreview } from "../control/DiffPreview";
import { toast } from "../use-toast";
import { ParameterDescriptions } from "../control/ParameterHelp";
import { GPU_TUNING_HELP } from "../control/parameterDescriptions";

export function GpuTuning({ active, revision, onSettled }: { active: boolean; revision: number; onSettled: () => void }) {
  const [profile, setProfile] = useState<GpuProfile | null>(null);
  const [core, setCore] = useState(0);
  const [mem, setMem] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const lastProfile = useRef<GpuProfile | null>(null);
  const running = useRef(false);
  const accept = (current: GpuProfile, reset: boolean) => {
    const previous = lastProfile.current;
    setCore((draft) => reset || !previous || draft === previous.coreOffsetMhz ? current.coreOffsetMhz ?? 0 : draft);
    setMem((draft) => reset || !previous || draft === previous.memOffsetMhz ? current.memOffsetMhz ?? 0 : draft);
    lastProfile.current = current; setProfile(current); setError(null);
  };
  const load = async (reset: boolean) => accept(await readGpuProfile(), reset);
  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    readGpuProfile().then((current) => { if (!cancelled) accept(current, false); })
      .catch((error) => { if (!cancelled) { setProfile(null); setError(String(error)); } });
    return () => { cancelled = true; };
  }, [active, revision]);
  const run = async (action: () => Promise<GpuApplyOutcome>) => {
    if (running.current) return;
    running.current = true;
    setBusy(true); setError(null);
    try {
      const result = await action();
      if (!result.ok) throw new Error([result.message, result.rollbackError].filter(Boolean).join("；"));
      await load(true); toast({ kind: "success", title: "GPU 偏移已确认" });
    } catch (error) { setError(String(error)); }
    finally { running.current = false; setBusy(false); onSettled(); }
  };
  if (!profile || !profile.available) return (
    <Surface as="section" variant="group" className="ccc-panel" aria-label="GPU 频率">
      <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="gpu" />GPU 频率</h2><span className="ccc-control-caption">{error || profile ? "不可用" : "读取中…"}</span></div>
      <DetailsDialog title="GPU 参数与诊断" label="详情">{error
        ? <p className="ccc-control-error" role="alert"><Icon name="error" size={16} />{error}</p>
        : <p className="ccc-control-caption">{profile?.unavailableReason ?? "等待驱动读数"}</p>}
        <ParameterDescriptions items={GPU_TUNING_HELP} /></DetailsDialog>
      <Button icon="refresh" variant="ghost" size="compact" onClick={() => { void load(false).catch((e) => setError(String(e))); }}>重试</Button>
    </Surface>
  );
  return <GpuControls profile={profile} core={core} mem={mem} busy={busy} error={error}
    setCore={setCore} setMem={setMem} run={run} />;
}

function GpuControls({ profile: p, core, mem, busy, error, setCore, setMem, run }: {
  profile: GpuProfile; core: number; mem: number; busy: boolean; error: string | null;
  setCore: (v: number) => void; setMem: (v: number) => void;
  run: (action: () => Promise<GpuApplyOutcome>) => Promise<void>;
}) {
  const coreDirty = p.coreEditable && core !== p.coreOffsetMhz;
  const memDirty = p.memEditable && mem !== p.memOffsetMhz;
  const changes = [
    ...(coreDirty ? [{ label: "核心频率偏移", from: `${p.coreOffsetMhz} MHz`, to: `${core} MHz` }] : []),
    ...(memDirty ? [{ label: "显存频率偏移", from: `${p.memOffsetMhz} MHz`, to: `${mem} MHz` }] : []),
  ];
  return (
    <Surface as="section" variant="group" className="ccc-panel" aria-label="GPU 频率">
      <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="gpu" />GPU 频率</h2>
        <div className="ccc-module-tools"><DetailsDialog title="GPU 参数与诊断" label="详情">
          <p className="ccc-control-caption">{p.gpuName}</p><ParameterDescriptions items={GPU_TUNING_HELP} />
          {p.warnings.map(w => <p key={w} className="ccc-control-caption is-warn"><Icon name="warning" size={16} />{w}</p>)}
        </DetailsDialog></div>
      </div>
      <ScaleSlider label="核心频率偏移" unit="MHz" min={p.coreMinMhz} max={Math.max(p.coreMinMhz, p.coreMaxMhz)}
        current={p.coreOffsetMhz ?? 0} draft={core} disabled={busy || !p.coreEditable}
        unavailable={p.coreOffsetMhz == null} onChange={setCore} />
      <ScaleSlider label="显存频率偏移" unit="MHz" min={p.memMinMhz} max={Math.max(p.memMinMhz, p.memMaxMhz)}
        current={p.memOffsetMhz ?? 0} draft={mem} disabled={busy || !p.memEditable}
        unavailable={p.memOffsetMhz == null} onChange={setMem} />
      {error && <p className="ccc-control-error" role="alert">{error}</p>}
      <ConfigurationAction label="GPU 频率" revision={JSON.stringify([p, changes])} disabled={busy || changes.length === 0}
        onConfirm={() => run(() => applyGpuClockOffsets(coreDirty ? core : null, memDirty ? mem : null))}>
        <DiffPreview items={changes} />
      </ConfigurationAction>
      <ConfigurationAction label="GPU 频率" revision={JSON.stringify(p)} restore disabled={busy || !p.baseline}
        onConfirm={() => run(restoreGpuBaseline)}>
        <p>恢复 GPU 初始频率偏移{p.baseline ? ` · 核心 ${p.baseline.coreOffsetMhz ?? "—"} MHz，显存 ${p.baseline.memOffsetMhz ?? "—"} MHz` : ""}。</p>
      </ConfigurationAction>
    </Surface>
  );
}
