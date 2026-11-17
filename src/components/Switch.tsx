interface SwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  describedBy?: string;
  disabled?: boolean;
}

/** 原生输入拥有键盘与辅助语义，轨道和滑块由应用绘制。 */
export function Switch({ checked, onChange, label, describedBy, disabled = false }: SwitchProps) {
  return (
    <label className="ccc-switch">
      <input type="checkbox" role="switch" aria-label={label} aria-describedby={describedBy}
        checked={checked} aria-checked={checked} aria-disabled={disabled || undefined}
        onClick={(event) => { if (disabled) event.preventDefault(); }}
        onChange={(event) => { if (!disabled) onChange(event.target.checked); }} />
      <span className="ccc-switch-track" aria-hidden="true"><span className="ccc-switch-thumb" /></span>
    </label>
  );
}
