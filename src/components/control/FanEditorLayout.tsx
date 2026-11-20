import type { ReactNode } from "react";

export interface FanEditorSections {
  settings: ReactNode;
  toolbar: ReactNode;
  recovery: ReactNode;
}

export function FanEditorLayout({ settings, toolbar, recovery, plot, fields }: FanEditorSections & {
  plot?: ReactNode;
  fields?: ReactNode;
}) {
  return <div className="ccc-fan-layout">
    <div className="ccc-fan-settings">{settings}</div>
    <div className="ccc-fan-curve">
      <div className="ccc-fan-curve-toolbar">{toolbar}</div>
      {plot}
    </div>
    {fields && <div className="ccc-fan-fields">{fields}</div>}
    {recovery && <div className="ccc-fan-recovery">{recovery}</div>}
  </div>;
}
