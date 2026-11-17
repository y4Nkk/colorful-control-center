import { useId } from "react";
import { Radio } from "./Radio";

interface SegmentedControlProps {
  label: string;
  value: string;
  options: readonly { value: string; label: string; disabled?: boolean; title?: string }[];
  disabled?: boolean;
  className?: string;
  onChange: (value: string) => void;
}

export function SegmentedControl({ label, value, options, disabled, className, onChange }: SegmentedControlProps) {
  const group = useId();
  return <div className={["ccc-segmented", className].filter(Boolean).join(" ")} role="radiogroup" aria-label={label}>
    {options.map(option => <label className="ccc-segmented-option" key={option.value}>
      <Radio appearance="overlay" name={group} checked={value === option.value} title={option.title}
        disabled={disabled || option.disabled} onChange={() => onChange(option.value)} />
      <span>{option.label}</span>
    </label>)}
  </div>;
}
