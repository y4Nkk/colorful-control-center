import { Button } from "../Button";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { setShortcutRecording } from "../../api/client";
import { captureShortcut, shortcutLabel } from "../../lib/global-shortcut";
import { SettingRow } from "./PreferenceCards";

function useRecorder(active: boolean, onChange: (chord: string) => Promise<boolean>) {
  const [recording, setRecording] = useState(false), [preparing, setPreparing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const button = useRef<HTMLButtonElement>(null), session = useRef(0), mounted = useRef(false);
  const report = (cause: unknown) => { if (mounted.current) setError(String(cause)); else console.error("结束快捷键录入失败", cause); };
  const finish = () => {
    ++session.current; setRecording(false); setPreparing(false);
    void setShortcutRecording(false).catch(report);
  };
  useEffect(() => {
    mounted.current = true;
    window.addEventListener("blur", finish);
    return () => { mounted.current = false; ++session.current; window.removeEventListener("blur", finish);
      void setShortcutRecording(false).catch(report); };
  }, []);
  useEffect(() => { if (!active) finish(); }, [active]);
  const start = async () => {
    const id = ++session.current;
    setPreparing(true); setError(null);
    try {
      await setShortcutRecording(true);
      if (!mounted.current || id !== session.current) { await setShortcutRecording(false); return; }
      setRecording(true); button.current?.focus();
    } catch (cause) { report(cause); }
    finally { if (mounted.current) setPreparing(false); }
  };
  const capture = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (!recording) return;
    if (event.key === "Tab") { finish(); return; }
    event.preventDefault(); event.stopPropagation();
    if (event.key === "Escape") { finish(); return; }
    if (event.repeat || /^(Control|Alt|Shift|Meta)(Left|Right)$/.test(event.code)) return;
    const chord = captureShortcut(event);
    if (!chord) { setError("请按 Ctrl、Alt 或 Win 与字母、数字、空格或 F1–F12 的组合"); return; }
    finish(); void onChange(chord);
  };
  return { button, recording, preparing, error, start, finish, capture };
}

export function ShortcutSetting({ value, busy, active, onChange }: {
  value: string; busy: boolean; active: boolean; onChange: (value: string) => Promise<boolean>;
}) {
  const recorder = useRecorder(active, onChange);
  return <SettingRow label="全局唤起快捷键">
    <div className="ccc-shortcut-setting"><div className="ccc-setting-control">
      <Button variant="secondary" type="button" ref={recorder.button} disabled={busy}
        aria-pressed={recorder.recording} aria-busy={recorder.preparing}
        aria-label={recorder.recording ? "按下快捷键，Esc 取消" : `录入全局快捷键，当前${shortcutLabel(value)}`}
        onClick={() => { if (!recorder.recording && !recorder.preparing) void recorder.start(); }}
        onKeyDown={recorder.capture} onBlur={() => { if (recorder.recording || recorder.preparing) recorder.finish(); }}>
        {recorder.preparing ? "准备录入…" : recorder.recording ? "按下快捷键…" : shortcutLabel(value)}
      </Button>
      {recorder.recording ? <Button icon="close" variant="secondary" type="button" onClick={recorder.finish}>取消</Button>
        : value && <Button icon="delete" variant="secondary" type="button" disabled={busy || recorder.preparing} onClick={() => void onChange("")}>清除</Button>}
    </div>{recorder.error && <p className="ccc-settings-error" role="alert">{recorder.error}</p>}</div>
  </SettingRow>;
}
