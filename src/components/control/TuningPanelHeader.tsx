import type { ReactNode } from "react";
import { DetailsDialog } from "../DetailsDialog";
import { Icon, type IconName } from "../Icon";

export function TuningPanelHeader({ title, icon, hardwareName, detailsTitle, children }: {
  title: string; icon: IconName; hardwareName: string | null; detailsTitle: string; children: ReactNode;
}) {
  return <header className="ccc-tuning-head">
    <div className="ccc-panel-head">
      <h2 className="ccc-panel-title"><Icon name={icon} />{title}</h2>
      <div className="ccc-module-tools"><DetailsDialog title={detailsTitle}>{children}</DetailsDialog></div>
    </div>
    <div className="ccc-tuning-meta">
      <p className="ccc-control-caption">{hardwareName?.trim() || "型号未读取"}</p>
    </div>
  </header>;
}
