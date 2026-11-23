import type { ReactNode } from "react";

export function FanCurveSection({ status, plot, fields }: {
  status?: string;
  plot?: ReactNode;
  fields?: ReactNode;
}) {
  return <section className="ccc-fan-curve">
    <div className="ccc-fan-curve-head">
      <h3>风扇曲线</h3>
      {status && <p className="ccc-control-caption">{status}</p>}
    </div>
    {plot && <div className="ccc-fan-curve-body">
      {plot}
      {fields && <div className="ccc-fan-fields">{fields}</div>}
    </div>}
  </section>;
}
