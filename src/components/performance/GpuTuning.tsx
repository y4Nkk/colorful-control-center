import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { Button } from "../Button";
import { useCallback, useEffect, useRef, useState } from "react";
import { applyGpuClockOffsets, readGpuProfile, restoreGpuBaseline, type GpuApplyOutcome, type GpuProfile } from "../../api/client";
import { ConfigurationAction } from "../ConfigurationAction";
import { DetailsDialog } from "../DetailsDialog";
import { ScaleSlider } from "../control/ScaleSlider";
import { DiffPreview } from "../control/DiffPreview";
import { toast } from "../use-toast";
import { ParameterDescriptions } from "../control/ParameterHelp";
import { GPU_TUNING_HELP } from "../control/parameterDescriptions";

const TITLE = "GPU 频率";

export function GpuTuning({ active, revision, onSettled }: { active: boolean; revision: number; onSettled: () => void }) {
  const [profile, setProfile] = useState<GpuProfile | null>(null);
  const [coreDraft, setCoreDraft] = useState<number | null>(null);
  const [memDraft, setMemDraft] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [readError, setReadError] = useState<string | null>(null);
  const readVersion = useRef(0);
  const running = useRef(false);
  const load = useCallback(async () => {
    const version = ++readVersion.current;
    try {
      const current = await readGpuProfile();
      if (version !== readVersion.current) return false;
      setProfile(current);
      setReadError(null);
      return current.available;
    } catch (error) {
      if (version === readVersion.current) {
        setProfile(null);
        setReadError(String(error));
      }
      return false;
    }
  }, []);
  useEffect(() => {
    if (active && !running.current) void load();
  }, [active, revision, load]);
  const run = async (action: () => Promise<GpuApplyOutcome>) => {
    if (running.current) return;
    running.current = true;
    ++readVersion.current;
    setBusy(true);
    setActionError(null);
    try {
      const result = await action();
      if (!result.ok) {
        setActionError([result.message, result.rollbackError].filter(Boolean).join("；"));
        if (result.rollbackError) {
          setProfile(null);
          await load();
        }
        return;
      }
      setCoreDraft(null);
      setMemDraft(null);
      if (await load()) toast({ kind: "success", title: "GPU 偏移已确认" });
    } catch (error) { setActionError(String(error)); }
    finally { running.current = false; setBusy(false); onSettled(); }
  };
  const unavailableReason = readError ?? profile?.unavailableReason;
  return <Surface as="section" variant="group" className="ccc-panel" aria-label={TITLE}>
    <div className="ccc-tuning-head">
      <h2 className="ccc-panel-title"><Icon name="gpu" />{TITLE}</h2>
      <div className="ccc-tuning-meta">
        {profile?.gpuName && <p className="ccc-control-caption">{profile.gpuName}</p>}
        <DetailsDialog title="GPU 参数与诊断" label="说明与诊断" icon={null}>
          {actionError && <p className="ccc-control-error" role="alert"><Icon name="error" size={16} />{actionError}</p>}
          {!profile?.available && <p className="ccc-control-caption">{unavailableReason ?? "等待读取偏移"}</p>}
          {profile?.writeBlockedReason && <p className="ccc-control-caption">{profile.writeBlockedReason}</p>}
          <ParameterDescriptions items={GPU_TUNING_HELP} />
          {profile?.warnings.map(w => <p key={w} className="ccc-control-caption is-warn"><Icon name="warning" size={16} />{w}</p>)}
        </DetailsDialog>
      </div>
    </div>
    {profile?.available ? <GpuControls profile={profile} core={coreDraft ?? profile.coreOffsetMhz ?? 0}
      mem={memDraft ?? profile.memOffsetMhz ?? 0} busy={busy} error={actionError}
      setCore={(value) => setCoreDraft(value === profile.coreOffsetMhz ? null : value)}
      setMem={(value) => setMemDraft(value === profile.memOffsetMhz ? null : value)} run={run} /> : <>
      {actionError && <p className="ccc-control-error" role="alert">{actionError}</p>}
      <p className="ccc-control-caption">{unavailableReason ? `无法读取当前偏移 · ${unavailableReason}` : "读取中…"}</p>
      {!busy && <div className="ccc-tuning-scope"><Button icon="refresh" variant="ghost" size="compact"
        onClick={() => void load()}>重试</Button></div>}
    </>}
  </Surface>;
}

function GpuControls({ profile: p, core, mem, busy, error, setCore, setMem, run }: {
  profile: GpuProfile; core: number; mem: number; busy: boolean; error: string | null;
  setCore: (v: number) => void; setMem: (v: number) => void;
  run: (action: () => Promise<GpuApplyOutcome>) => Promise<void>;
}) {
  const controlsDisabled = busy || p.writeBlockedReason !== null;
  const coreDirty = p.coreEditable && core !== p.coreOffsetMhz;
  const memDirty = p.memEditable && mem !== p.memOffsetMhz;
  const changes = [
    ...(coreDirty ? [{ label: "核心频率偏移", from: `${p.coreOffsetMhz} MHz`, to: `${core} MHz` }] : []),
    ...(memDirty ? [{ label: "显存频率偏移", from: `${p.memOffsetMhz} MHz`, to: `${mem} MHz` }] : []),
  ];
  return <>
    <div className="ccc-tuning-group ccc-tuning-columns">
      <ScaleSlider label="核心频率偏移" unit="MHz" min={p.coreMinMhz} max={Math.max(p.coreMinMhz, p.coreMaxMhz)} magnetInterval={50}
        current={p.coreOffsetMhz ?? 0} draft={core} disabled={controlsDisabled || !p.coreEditable}
        unavailable={p.coreOffsetMhz == null} onChange={setCore} />
      <ScaleSlider label="显存频率偏移" unit="MHz" min={p.memMinMhz} max={Math.max(p.memMinMhz, p.memMaxMhz)} magnetInterval={100}
        current={p.memOffsetMhz ?? 0} draft={mem} disabled={controlsDisabled || !p.memEditable}
        unavailable={p.memOffsetMhz == null} onChange={setMem} />
    </div>
      {p.writeBlockedReason && <p className="ccc-control-caption">{p.writeBlockedReason}</p>}
      {error && <p className="ccc-control-error" role="alert">{error}</p>}
      <ConfigurationAction label="GPU 频率" revision={JSON.stringify([p, changes])} disabled={controlsDisabled || changes.length === 0}
        onConfirm={() => run(() => applyGpuClockOffsets(coreDirty ? core : null, memDirty ? mem : null))}>
        <DiffPreview items={changes} />
      </ConfigurationAction>
      <ConfigurationAction label="GPU 频率" revision={JSON.stringify(p)} restore disabled={controlsDisabled || !p.baseline}
        onConfirm={() => run(restoreGpuBaseline)}>
        <p>恢复 GPU 初始频率偏移{p.baseline ? ` · 核心 ${p.baseline.coreOffsetMhz ?? "—"} MHz，显存 ${p.baseline.memOffsetMhz ?? "—"} MHz` : ""}。</p>
      </ConfigurationAction>
  </>;
}
