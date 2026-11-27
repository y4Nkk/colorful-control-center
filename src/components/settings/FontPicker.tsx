import { useEffect, useMemo, useState } from "react";
import { listSystemFonts } from "../../api/client";
import { interfaceFontStack } from "../../lib/appearance";
import { Select } from "../Select";

export function FontPicker({ value, busy, onChange }: {
  value: string; busy: boolean; onChange: (font: string) => Promise<boolean>;
}) {
  const [fonts, setFonts] = useState<string[]>([]), [error, setError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0), [loading, setLoading] = useState(true);
  useEffect(() => {
    let cancelled = false;
    setLoading(true); setError(null);
    listSystemFonts().then(list => {
      if (!cancelled) { setFonts(list); if (!list.length) setError("未读取到已安装的系统字体"); }
    }).catch(cause => { if (!cancelled) setError("系统字体读取失败：" + cause); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [attempt]);
  const options = useMemo(() => ["system", ...new Set([...fonts, ...(value === "system" ? [] : [value])])]
    .map(font => ({ value: font, label: font === "system" ? "默认（文泉驿正黑）" : font, description: "概览 Aa 0123", style: { fontFamily: interfaceFontStack(font) } })), [fonts, value]);
  return <Select label="选择界面字体" searchLabel="搜索字体" value={value} options={options}
    disabled={busy} loading={loading} error={error} onRetry={() => setAttempt(current => current + 1)}
    onChange={font => { void onChange(font); }} />;
}
