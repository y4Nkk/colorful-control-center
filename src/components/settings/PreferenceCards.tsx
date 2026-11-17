import { Radio } from "../Radio";
import { Surface } from "../Surface";
import { useId, type ReactNode } from "react";
import { Icon } from "../Icon";

export function SettingRow({ label, detail, children }: { label: string; detail?: string; children: ReactNode }) {
  return <div className="ccc-setting-row">
    <div className="ccc-setting-copy"><span className="ccc-settings-label">{label}</span>{detail && <p>{detail}</p>}</div>
    {children}
  </div>;
}

export function PreferenceCards<T extends string | number>({ label, value, options, busy, onChange, preview }: {
  label: string; value: T; options: readonly { value: T; label: string }[];
  busy: boolean; onChange: (value: T) => void; preview: (value: T) => ReactNode;
}) {
  const id = useId();
  return <fieldset className="ccc-preference-fieldset">
    <legend className="ccc-settings-label">{label}</legend>
    <div className={`ccc-preference-options${options.length === 4 ? " is-four" : ""}`}>
      {options.map(option => <Surface as="label" variant="card" key={option.value} className="ccc-preference-card">
        <Radio appearance="overlay" name={id} checked={value === option.value} disabled={busy}
          aria-label={option.label} onChange={() => onChange(option.value)} />
        <span className="ccc-preference-card-body">
          <span className="ccc-preference-art" aria-hidden="true">{preview(option.value)}</span>
          <span className="ccc-preference-caption"><span><strong>{option.label}</strong></span>
            <Icon name="check-circle" className="ccc-preference-check" /></span>
        </span>
      </Surface>)}
    </div>
  </fieldset>;
}
