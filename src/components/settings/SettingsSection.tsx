import { useId, type ReactNode } from "react";
import { Icon, type IconName } from "../Icon";
import { Surface } from "../Surface";

export function SettingsSection({ title, icon, children }: { title: string; icon?: IconName; children: ReactNode }) {
  const titleId = useId();
  return <section className="ccc-settings-section" aria-labelledby={titleId}>
    <h3 className="ccc-panel-title" id={titleId}>
      {icon && <Icon name={icon} />}<span>{title}</span>
    </h3>
    <Surface variant="group" className="ccc-settings-group">{children}</Surface>
  </section>;
}

export function SettingRow({ label, detail, children }: { label: string; detail?: ReactNode; children: ReactNode }) {
  const labelId = useId();
  return <div className="ccc-settings-item ccc-setting-row" role="group" aria-labelledby={labelId}>
    <div className="ccc-setting-copy">
      <span className="ccc-settings-label" id={labelId}>{label}</span>{detail && <p>{detail}</p>}
    </div>
    <div className="ccc-setting-control">{children}</div>
  </div>;
}
