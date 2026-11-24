interface SwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  describedBy?: string;
  disabled?: boolean;
  busy?: boolean;
}

/** 原生输入拥有键盘与辅助语义，轨道和滑块由应用绘制。 */
export function Switch({ checked, onChange, label, describedBy, disabled = false, busy = false }: SwitchProps) {
  const locked = disabled || busy;
  return (
    <label className="ccc-switch">
      <input type="checkbox" role="switch" aria-label={label} aria-describedby={describedBy}
        checked={checked} aria-checked={checked} aria-disabled={locked || undefined} aria-busy={busy && !disabled || undefined}
        onClick={(event) => { if (locked) event.preventDefault(); }}
        onChange={(event) => { if (!locked) onChange(event.target.checked); }} />
      <span className="ccc-switch-track" aria-hidden="true"><span className="ccc-switch-thumb" /></span>
    </label>
  );
}
