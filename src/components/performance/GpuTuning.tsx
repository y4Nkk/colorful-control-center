import { Icon } from "../Icon";
import { Loading } from "../Loading";
import { Surface } from "../Surface";
import { Button } from "../Button";
import { useCallback, useEffect, useRef, useState } from "react";
import { applyGpuClockOffsets, readGpuProfile, restoreGpuBaseline, type GpuApplyOutcome, type GpuProfile } from "../../api/client";
import { useAdministrator } from "../AdministratorHost";
import { ConfigurationAction } from "../ConfigurationAction";
import { ScaleSlider } from "../control/ScaleSlider";
import { DiffPreview } from "../control/DiffPreview";
import { toast } from "../use-toast";
import { ParameterDescriptions } from "../control/ParameterHelp";
import { GPU_TUNING_HELP } from "../control/parameterDescriptions";
import { TuningPanelHeader } from "../control/TuningPanelHeader";

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
  const load = useCallback(async (resetDraft = false) => {
    const version = ++readVersion.current;
    try {
      const current = await readGpuProfile();
      if (version !== readVersion.current) return false;
      if (!current.available) {
        setProfile(previous => previous?.available ? previous : current);
        setReadError(current.unavailableReason ?? "GPU 偏移不可用");
        return false;
      }
      setProfile(current);
      setReadError(null);
      if (resetDraft) { setCoreDraft(null); setMemDraft(null); }
      return true;
    } catch (error) {
      if (version === readVersion.current) {
        setReadError(String(error));
      }
      return false;
    }
  }, []);
  useEffect(() => {
    if (active && !running.current) void load();
  }, [active, revision, load]);
  const run = async (action: () => Promise<GpuApplyOutcome>) => {
    if (!active || running.current || readError !== null || !profile?.available) return;
    running.current = true;
    ++readVersion.current;
    setBusy(true);
    try {
      const result = await action();
      if (!result.ok) {
        setActionError([result.message, result.rollbackError].filter(Boolean).join("；"));
        if (result.rollbackError) await load();
        return;
      }
      const refreshed = await load(true);
      setActionError(null);
      if (refreshed) toast({ kind: "success", title: "GPU 偏移已确认" });
    } catch (error) { setActionError(String(error)); }
    finally { running.current = false; setBusy(false); onSettled(); }
  };
  const unavailableReason = readError ?? profile?.unavailableReason;
  return <Surface as="section" variant="group" className="ccc-panel" aria-label={TITLE} aria-busy={busy || undefined}>
    <TuningPanelHeader title={TITLE} icon="gpu" hardwareName={profile?.gpuName ?? null} detailsTitle="GPU 参数与诊断">
      {actionError && <p className="ccc-control-error" role="alert"><Icon name="error" size={16} />{actionError}</p>}
      {(!profile?.available || readError !== null) && <p className="ccc-control-caption">{unavailableReason ?? "等待读取偏移"}</p>}
      {profile?.writeBlockedReason && <p className="ccc-control-caption">{profile.writeBlockedReason}</p>}
      <ParameterDescriptions items={GPU_TUNING_HELP} />
      {profile?.warnings.map(w => <p key={w} className="ccc-control-caption is-warn"><Icon name="warning" size={16} />{w}</p>)}
    </TuningPanelHeader>
    {profile?.available ? <GpuControls profile={profile} core={coreDraft ?? profile.coreOffsetMhz ?? 0}
      mem={memDraft ?? profile.memOffsetMhz ?? 0} busy={busy} readValid={readError === null} error={actionError}
      setCore={(value) => setCoreDraft(value === profile.coreOffsetMhz ? null : value)}
      setMem={(value) => setMemDraft(value === profile.memOffsetMhz ? null : value)} run={run} /> : <>
      {actionError && <p className="ccc-control-error" role="alert">{actionError}</p>}
      {unavailableReason ? <p className="ccc-control-caption">无法读取当前偏移 · {unavailableReason}</p> : <Loading label="正在读取…" />}
    </>}
    {(!profile?.available || readError !== null) && <div className="ccc-tuning-scope">
      {profile?.available && <p className="ccc-control-error" role="alert">无法读取当前偏移 · {readError}。显示上次读取的设置。</p>}
      <Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={() => void load()}>重试</Button>
    </div>}
  </Surface>;
}

function GpuControls({ profile: p, core, mem, busy, readValid, error, setCore, setMem, run }: {
  profile: GpuProfile; core: number; mem: number; busy: boolean; readValid: boolean; error: string | null;
  setCore: (v: number) => void; setMem: (v: number) => void;
  run: (action: () => Promise<GpuApplyOutcome>) => Promise<void>;
}) {
  const [restartError, setRestartError] = useState<string | null>(null);
  const administrator = useAdministrator();
  const writeBlocked = p.writeBlockedReason !== null;
  const controlsDisabled = !readValid || writeBlocked;
  const blocked = busy || !readValid;
  const coreDirty = p.coreEditable && core !== p.coreOffsetMhz;
  const memDirty = p.memEditable && mem !== p.memOffsetMhz;
  const changes = [
    ...(coreDirty ? [{ label: "核心频率偏移", from: `${p.coreOffsetMhz} MHz`, to: `${core} MHz` }] : []),
    ...(memDirty ? [{ label: "显存频率偏移", from: `${p.memOffsetMhz} MHz`, to: `${mem} MHz` }] : []),
  ];
  return <>
    <div className="ccc-tuning-group ccc-tuning-columns">
      <ScaleSlider label="核心频率偏移" unit="MHz" min={p.coreMinMhz} max={Math.max(p.coreMinMhz, p.coreMaxMhz)} magnetInterval={50}
        current={p.coreOffsetMhz ?? 0} draft={core} disabled={controlsDisabled || !p.coreEditable} busy={busy}
        unavailable={p.coreOffsetMhz == null} onChange={setCore} />
      <ScaleSlider label="显存频率偏移" unit="MHz" min={p.memMinMhz} max={Math.max(p.memMinMhz, p.memMaxMhz)} magnetInterval={100}
        current={p.memOffsetMhz ?? 0} draft={mem} disabled={controlsDisabled || !p.memEditable} busy={busy}
        unavailable={p.memOffsetMhz == null} onChange={setMem} />
    </div>
      {writeBlocked && <div className="ccc-tuning-scope">
        <p className="ccc-control-caption">{p.writeBlockedReason}</p>
        {administrator.access === "canElevate" && <Button variant="secondary" icon="confirm" size="compact"
          aria-haspopup="dialog" disabled={blocked || administrator.pending} onClick={() => {
            setRestartError(null);
            administrator.request(setRestartError);
          }}>以管理员身份重启</Button>}
      </div>}
      {writeBlocked && restartError && <p className="ccc-control-error" role="alert">{restartError}</p>}
      {error && <p className="ccc-control-error" role="alert">{error}</p>}
      <ConfigurationAction label="GPU 频率" revision={JSON.stringify([p, changes])} disabled={writeBlocked || changes.length === 0} blocked={blocked}
        onConfirm={() => run(() => applyGpuClockOffsets(coreDirty ? core : null, memDirty ? mem : null))}>
        <DiffPreview items={changes} />
      </ConfigurationAction>
      <ConfigurationAction label="GPU 频率" revision={JSON.stringify(p)} restore disabled={writeBlocked || !p.baseline} blocked={blocked}
        onConfirm={() => run(restoreGpuBaseline)}>
        <p>恢复 GPU 初始频率偏移{p.baseline ? ` · 核心 ${p.baseline.coreOffsetMhz ?? "—"} MHz，显存 ${p.baseline.memOffsetMhz ?? "—"} MHz` : ""}。</p>
      </ConfigurationAction>
  </>;
}
