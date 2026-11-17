import { Icon } from "../Icon";
import { useCallback, useEffect, useRef, useState } from "react";
import { applyGraphicsMode, readGraphicsMode, rebootForGraphicsMode, restoreGraphicsMode, type GraphicsModeProfile, type GraphicsModeVerification } from "../../api/client";
import { Button } from "../Button";
import { ConfirmButton } from "../ConfirmButton";
import { Select } from "../Select";
import { Surface } from "../Surface";
import { toast } from "../use-toast";

const MODES = [{ value: 1, label: "仅集显" }, { value: 2, label: "仅独显" }, { value: 3, label: "混合显卡" }, { value: 4, label: "动态切换" }];
const name = (mode: number) => MODES.find(item => item.value === mode)?.label ?? "未知";
const STATUS: Record<GraphicsModeVerification, string> = { uncertain: "上次操作未完成，请重新发送或恢复", pendingReboot: "已发送选择，重启后确认", confirmed: "重启后已确认", notApplied: "重启后未匹配目标，可重新发送或恢复" };

function useGraphicsMode(active: boolean, revision: number) {
  const [profile, setProfile] = useState<GraphicsModeProfile | null>(null), [target, setTarget] = useState<number | null>(null);
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null);
  const running = useRef(false), generation = useRef(0);
  const reload = useCallback(async (reset: boolean) => {
    const id = ++generation.current;
    let next: GraphicsModeProfile;
    try { next = await readGraphicsMode(); } catch (e) { if (id === generation.current) throw e; else return; }
    if (id !== generation.current) return;
    setProfile(next); if (reset) setTarget(null);
  }, []);
  useEffect(() => { if (active) void reload(false).catch(e => setError(String(e))); return () => { ++generation.current; }; }, [active, revision, reload]);
  const run = async (action: () => Promise<void>, message: string) => {
    if (running.current) return;
    running.current = true; ++generation.current; setBusy(true); setError(null);
    try { await action(); await reload(true); toast({ kind: "success", title: message }); }
    catch (e) { setError(String(e)); await reload(false).catch(readError => setError(previous => `${previous}；回读失败：${readError}`)); }
    finally { running.current = false; setBusy(false); }
  };
  return { profile, draft: target ?? profile?.lastRequest?.target ?? profile?.current?.mode ?? 0, setTarget, busy, error, run,
    reread: () => reload(false).catch(e => setError(String(e))) };
}
export function GraphicsModeSettings({ active, revision }: { active: boolean; revision: number }) {
  const { profile, draft, setTarget, busy, error, run, reread } = useGraphicsMode(active, revision);
  const current = profile?.current, last = profile?.lastRequest;
  const choices = MODES.filter(item => current && (current.availableMask & (1 << (item.value - 1))) !== 0);
  const unchanged = !last && current?.mode === draft || last?.verification === "confirmed" && last.target === draft;
  return <Surface as="section" variant="group" className="ccc-panel ccc-keyboard-editor" aria-label="显卡模式">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="gpu" />显卡模式</h2><Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={reread}>重新读取显卡模式</Button></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {!profile || !current ? <p className="ccc-control-caption">{profile ? "本机未提供已确认的模式切换接口。" : "正在读取显卡模式。"}</p> : <>
      <div className="ccc-device-preview"><span>固件报告 <strong>{name(current.mode)}</strong></span><span>→</span>
        <Select label="目标显卡模式" value={String(draft)} disabled={busy} options={choices.map(item => ({ value: String(item.value), label: item.label }))} onChange={value => setTarget(Number(value))} /></div>
      <span className="ccc-chip is-warn">需重启生效</span>
      {last && <Surface as="div" variant="inset" className="ccc-keyboard-setting" role="status"><span>上次选择 <strong>{name(last.target)}</strong></span><span>{STATUS[last.verification]}</span></Surface>}
      {last && (current.availableMask & (1 << (last.initialMode - 1))) === 0 && <p className="ccc-control-caption">当前固件未提供原模式，无法发送恢复选择。</p>}
      <div className="ccc-actions">
        <ConfirmButton key={JSON.stringify([profile.revision, draft])} label="发送显卡选择" confirmLabel={`确认发送${name(draft)}选择？`} busy={busy}
          disabled={unchanged || !choices.some(item => item.value === draft)} onConfirm={() => run(() => applyGraphicsMode(profile.revision, draft), "显卡选择已发送，重启后确认")} />
        <ConfirmButton icon="undo" key={`restore-${profile.revision}`} variant="secondary" label="恢复模式初始" confirmLabel={`恢复${last ? name(last.initialMode) : "初始"}选择，重启后确认？`} busy={busy} disabled={!profile.canRestore}
          onConfirm={() => run(() => restoreGraphicsMode(profile.revision), "原模式选择已发送，重启后确认")} />
        {last?.verification === "pendingReboot" && <ConfirmButton key={`reboot-${profile.revision}`} variant="secondary" label="重启 Windows" confirmLabel="已保存工作，确认重启 Windows？" busy={busy}
          onConfirm={() => run(() => rebootForGraphicsMode(profile.revision), "Windows 重启请求已发送")} />}
      </div>
    </>}
  </Surface>;
}
