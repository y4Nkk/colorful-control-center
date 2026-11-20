import { useId, useState } from "react";
import type { StoredFanCheckpoint } from "../../api/fans";
import { Button } from "../Button";

export function FanRestoreParameters({ initial, current }: { initial: StoredFanCheckpoint; current?: StoredFanCheckpoint }) {
  const [expanded, setExpanded] = useState(false);
  const id = useId();
  return <div>
    <Button variant="ghost" size="compact" icon={expanded ? "chevron-down" : "chevron-right"}
      aria-expanded={expanded} aria-controls={id} onClick={() => setExpanded(value => !value)}>查看完整风扇恢复参数</Button>
    <div id={id} hidden={!expanded}>
      {expanded && <div className="ccc-mode-restore-columns">
        {current && <div><h4>当前参数</h4><pre className="ccc-mode-restore-raw ccc-mono">{JSON.stringify(current, null, 2)}</pre></div>}
        <div><h4>初始参数</h4><pre className="ccc-mode-restore-raw ccc-mono">{JSON.stringify(initial, null, 2)}</pre></div>
      </div>}
    </div>
  </div>;
}
