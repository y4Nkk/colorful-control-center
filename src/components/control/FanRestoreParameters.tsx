import type { StoredFanCheckpoint } from "../../api/fans";
import { DetailsDialog } from "../DetailsDialog";

export function FanRestoreParameters({ initial, current }: { initial: StoredFanCheckpoint; current?: StoredFanCheckpoint }) {
  return <DetailsDialog title="完整风扇恢复参数" label="查看完整风扇恢复参数" icon={null} wide>
    <div className="ccc-mode-restore-columns">
      {current && <div><h3>当前参数</h3><pre className="ccc-mode-restore-raw ccc-mono">{JSON.stringify(current, null, 2)}</pre></div>}
      <div><h3>初始参数</h3><pre className="ccc-mode-restore-raw ccc-mono">{JSON.stringify(initial, null, 2)}</pre></div>
    </div>
  </DetailsDialog>;
}
