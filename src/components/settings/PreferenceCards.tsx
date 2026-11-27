import { Radio } from "../Radio";
import { Surface } from "../Surface";
import { useId, type ReactNode } from "react";
import { Icon } from "../Icon";

export function PreferenceCards<T extends string | number>({ label, value, options, busy, onChange, preview }: {
  label: string; value: T; options: readonly { value: T; label: string }[];
  busy: boolean; onChange: (value: T) => void; preview: (value: T) => ReactNode;
}) {
  const id = useId();
  return <div className="ccc-settings-item">
    <fieldset className="ccc-preference-fieldset">
      <legend className="ccc-settings-label">{label}</legend>
      <div className="ccc-preference-options" data-options={options.length}>
        {options.map(option => <Surface as="label" variant="card" key={option.value} className="ccc-preference-card">
          <Radio appearance="overlay" name={id} checked={value === option.value} disabled={busy}
            aria-label={option.label} onChange={() => onChange(option.value)} />
          <span className="ccc-preference-card-body">
            <span className="ccc-preference-art" aria-hidden="true">{preview(option.value)}</span>
            <span className="ccc-preference-caption"><strong>{option.label}</strong>
              <Icon name="check-circle" className="ccc-preference-check" /></span>
          </span>
        </Surface>)}
      </div>
    </fieldset>
  </div>;
}
