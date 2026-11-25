import { useContext, useState, type ReactNode } from "react";
import { Checkbox } from "./Checkbox";
import { ConfirmButton } from "./ConfirmButton";
import { WorkspaceActivity } from "./Dialog";

export type RestoreOption = {
  label: string;
  revision: string | null;
  preview: ReactNode;
  exclusiveGroup?: string;
};

type Props<T extends RestoreOption> = {
  label: string;
  triggerLabel?: string;
  options: readonly T[];
  onRestore: (options: T[]) => void | boolean | Promise<void | boolean>;
  blocked?: boolean;
  note?: ReactNode;
};

export function RestoreSettings<T extends RestoreOption>({ label, triggerLabel = `恢复${label}`, options, onRestore,
  blocked, note }: Props<T>) {
  const active = useContext(WorkspaceActivity);
  const [checked, setChecked] = useState<string[]>([]);
  if (!active || options.length === 0) return null;
  const revision = JSON.stringify(options.map(({ label, revision, exclusiveGroup }) => [label, revision, exclusiveGroup]));
  const grouped = options.length > 1;
  const available = options.filter(option => option.revision !== null);
  const selected = grouped ? available.filter(option => checked.includes(option.label)) : available;
  const toggle = (label: string, checked: boolean) => setChecked(previous =>
    checked ? [...previous, label] : previous.filter(value => value !== label));
  return <div className="ccc-actions" data-module-restore={label}>
    <ConfirmButton label={triggerLabel} icon="undo" size="compact" tone="danger" confirmLabel="确认恢复" revision={revision}
      disabled={blocked || available.length === 0} confirmDisabled={selected.length === 0}
      onOpen={() => setChecked([])} onConfirm={() => onRestore(selected)}>
      {note}
      {grouped ? <div className="ccc-restore-options" role="group" aria-label="恢复项目">
        {options.map(option => {
          const isChecked = checked.includes(option.label);
          const disabled = option.revision === null || !!option.exclusiveGroup && selected.some(other =>
            other !== option && other.exclusiveGroup === option.exclusiveGroup);
          return <div className="ccc-restore-option" key={option.label}>
            <Checkbox className="ccc-restore-check" label={option.label} checked={isChecked} disabled={disabled}
              onChange={event => toggle(option.label, event.target.checked)} />
            {(isChecked || option.revision === null) && option.preview &&
              <div className="ccc-restore-preview">{option.preview}</div>}
          </div>;
        })}
      </div> : options[0].preview}
    </ConfirmButton>
  </div>;
}
