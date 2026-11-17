import { useEffect, useRef, useState } from "react";
import { previewKeymapImport, type KeymapConfiguration } from "../../api/client";
import { Button } from "../Button";
import { Input } from "../Input";

export function KeymapFiles({ draft, disabled, onImport }: { draft: KeymapConfiguration; disabled: boolean; onImport: (draft: KeymapConfiguration) => void }) {
  const [name, setName] = useState("我的映射"), [error, setError] = useState<string | null>(null), [busy, setBusy] = useState(false);
  const input = useRef<HTMLInputElement>(null), generation = useRef(0), running = useRef(false);
  useEffect(() => { ++generation.current; }, [disabled]);
  useEffect(() => () => { ++generation.current; }, []);
  const run = async (action: (id: number) => Promise<void>) => {
    if (running.current || disabled) return;
    running.current = true; setBusy(true); setError(null); const id = ++generation.current;
    try { await action(id); } catch (error) { if (id === generation.current) setError(String(error)); }
    finally { running.current = false; setBusy(false); }
  };
  const stage = (file: File) => run(async id => {
    if (file.size > 4 * 1024 * 1024) throw new Error("映射配置不能超过4 MB");
    const candidate = await previewKeymapImport(await file.text());
    if (id !== generation.current) return;
    setName(candidate.name); onImport(candidate.configuration);
  });
  const download = () => run(async id => {
    const file = await previewKeymapImport(JSON.stringify({ version: 1, name, configuration: draft }));
    if (id !== generation.current) return;
    const url = URL.createObjectURL(new Blob([JSON.stringify(file, null, 2)], { type: "application/json" }));
    const anchor = document.createElement("a"); anchor.href = url; anchor.download = `${file.name.replace(/[<>:"/\\|?*]/g, "_")}.keymap.json`;
    anchor.click(); URL.revokeObjectURL(url);
  });
  return <div className="ccc-keymap-files">
    <label className="ccc-keymap-field">配置名称 <Input aria-label="映射配置名称" maxLength={80} value={name} disabled={disabled || busy} onChange={event => setName(event.target.value)} /></label>
    <div className="ccc-actions">
      <Button icon="upload" variant="secondary" size="compact" disabled={disabled || busy} onClick={() => input.current?.click()}>导入映射草稿</Button>
      <Button icon="download" variant="secondary" size="compact" disabled={disabled || busy || !name.trim()} onClick={() => void download()}>导出映射草稿</Button>
      <Input type="file" ref={input} hidden accept=".json,application/json" aria-label="导入映射配置" disabled={disabled || busy}
        onChange={event => { const file = event.target.files?.[0]; event.target.value = ""; if (file) void stage(file); }} />
    </div>
    <p className="ccc-control-caption">导入只替换草稿，预览并确认应用后才改变映射。</p>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
  </div>;
}
