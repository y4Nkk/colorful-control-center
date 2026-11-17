import { Icon } from "../Icon";
import { useCallback, useEffect, useRef, useState } from "react";
import { applyKeyboardSettings, readKeyboardSettings, restoreKeyboardSettings, type KeyboardSettingsProfile, type KeyboardValues } from "../../api/client";
import { Button } from "../Button";
import { Checkbox } from "../Checkbox";
import { ConfirmButton } from "../ConfirmButton";
import { Surface } from "../Surface";
import { Collapsible } from "../Collapsible";
import { toast } from "../use-toast";
import { FIELDS, stateText } from "./keyboard-values";

function useKeyboardSettings(active: boolean, revision: number) {
  const [profile, setProfile] = useState<KeyboardSettingsProfile | null>(null);
  const [draft, setDraft] = useState<KeyboardValues | null>(null);
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null);
  const running = useRef(false), generation = useRef(0), accepted = useRef<KeyboardValues | null>(null);
  const reload = useCallback(async (reset: boolean) => {
    const id = ++generation.current;
    let next: KeyboardSettingsProfile;
    try { next = await readKeyboardSettings(); } catch (e) { if (id === generation.current) { setProfile(null); throw e; } else return; }
    if (id !== generation.current) return;
    setProfile(next);
    const before = accepted.current; accepted.current = next.current;
    setDraft(previous => reset || previous === null || JSON.stringify(previous) === JSON.stringify(before) ? next.current : previous);
  }, []);
  useEffect(() => {
    if (active && !running.current) void reload(false).catch(e => setError(String(e)));
    return () => { ++generation.current; };
  }, [active, revision, reload]);
  const run = async (action: () => Promise<void>, message: string) => {
    if (running.current) return;
    running.current = true; ++generation.current; setBusy(true); setError(null);
    try { await action(); await reload(true); toast({ kind: "success", title: message }); }
    catch (e) { setError(String(e)); await reload(false).catch(readError => setError(previous => `${previous}；回读失败：${readError}`)); }
    finally { running.current = false; setBusy(false); }
  };
  const reread = async () => {
    if (running.current) return;
    running.current = true; setBusy(true); setError(null);
    try { await reload(true); } catch (e) { setError(String(e)); }
    finally { running.current = false; setBusy(false); }
  };
  return { profile, draft, setDraft, busy, error, run, reread };
}
export function KeyboardSettings({ active, revision = 0 }: { active: boolean; revision?: number }) {
  const { profile, draft, setDraft, busy, error, run, reread } = useKeyboardSettings(active, revision);
  const dirty = !!profile && !!draft && JSON.stringify(profile.current) !== JSON.stringify(draft);
  const request = profile && draft ? { expectedRevision: profile.revision, values: draft } : null;
  const change = (key: keyof KeyboardValues, value: boolean) => setDraft(previous => previous ? { ...previous, [key]: value } : previous);
  return <Surface as="section" variant="group" className="ccc-panel ccc-keyboard-editor" aria-label="键盘功能">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="keyboard" />键盘功能</h2><Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={reread}>重新读取键盘</Button></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {!profile || !draft ? <p className="ccc-control-caption">{error ? "暂时无法读取，请重试。" : "正在读取键盘功能。"}</p> : <>
      <div className="ccc-setting-tiles">{FIELDS.map(({ key, title }) => <Surface as="div" variant="inset" className="ccc-keyboard-setting" key={key}>
        <div><strong>{title}</strong>
          <p className="ccc-control-caption">{profile.current[key] === null ? "此设备未提供" : `当前${stateText(profile.current[key])}${draft[key] !== profile.current[key] ? ` → ${stateText(draft[key])}` : ""}`}</p></div>
        <Checkbox label={stateText(draft[key])} aria-label={title} checked={draft[key] ?? false}
          disabled={busy || profile.current[key] === null} onChange={event => change(key, event.target.checked)} />
      </Surface>)}</div>
      <div className="ccc-actions">
        <ConfirmButton key={JSON.stringify(request)} label="应用键盘设置" confirmLabel="确认应用键盘设置？" busy={busy} disabled={!dirty || !request}
          onConfirm={() => request ? run(() => applyKeyboardSettings(request), "键盘设置已应用") : undefined} />
        <ConfirmButton icon="undo" variant="secondary" label="恢复键盘初始" confirmLabel="恢复本会话修改前的键盘设置？" busy={busy} disabled={!profile.canRestore}
          onConfirm={() => run(restoreKeyboardSettings, "键盘初始设置已恢复")} />
      </div>
      <Collapsible summary="键盘功能说明">{FIELDS.map(field => <p key={field.key}><strong>{field.title}</strong> · {field.description}</p>)}</Collapsible>
    </>}
  </Surface>;
}
