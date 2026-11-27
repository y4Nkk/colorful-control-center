import { useId, type ReactNode } from "react";
import { Radio } from "./Radio";
import { Tooltip } from "./Tooltip";

interface SegmentedControlProps {
  label: string;
  value: string;
  options: readonly { value: string; label: ReactNode; disabled?: boolean; tooltip?: string }[];
  disabled?: boolean;
  busy?: boolean;
  pendingValue?: string;
  balancedWrap?: boolean;
  className?: string;
  onChange: (value: string) => void;
  onReselect?: (value: string) => void;
}

export function SegmentedControl({ label, value, options, disabled, busy, pendingValue, balancedWrap, className, onChange, onReselect }: SegmentedControlProps) {
  const group = useId();
  return <div className={["ccc-segmented", className].filter(Boolean).join(" ")} role="radiogroup" aria-label={label} aria-busy={busy}
    data-balanced-wrap={balancedWrap || undefined}>
    {options.map(option => {
      const checked = value === option.value;
      return <label className="ccc-segmented-option" key={option.value} data-pending={pendingValue === option.value || undefined}>
        <Tooltip content={option.tooltip}>
          <Radio appearance="overlay" name={group} checked={checked} aria-description={option.tooltip}
            disabled={disabled || option.disabled} aria-disabled={busy || undefined}
            onChange={() => { if (!busy) onChange(option.value); }}
            onClick={event => {
              if (busy) event.preventDefault();
              else if (checked) onReselect?.(option.value);
            }} />
        </Tooltip>
        <span>{option.label}</span>
      </label>;
    })}
  </div>;
}
