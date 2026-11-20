import { useEffect } from "react";
import type { KeymapStep } from "../../api/client";
import { Button } from "../Button";
import { Icon } from "../Icon";
import { useKeymapRecording } from "./useKeymapRecording";

export function MacroRecorder({ active, disabled, onAdopt, onRecordingChange }: {
  active: boolean; disabled: boolean; onAdopt: (steps: KeymapStep[]) => void; onRecordingChange: (active: boolean) => void;
}) {
  const model = useKeymapRecording(active), recording = model.capture?.active ?? false;
  useEffect(() => { onRecordingChange(recording); }, [recording, onRecordingChange]);
  useEffect(() => () => onRecordingChange(false), [onRecordingChange]);
  return <div className="ccc-keymap-recorder">
    <div className="ccc-actions">
      <Button variant="secondary" size="compact" disabled={disabled || model.busy || recording} onClick={() => void model.start()}>录制按键</Button>
      <Button icon="pause" variant="secondary" size="compact" disabled={!recording || model.busy} onClick={() => void model.stop()}>停止录制</Button>
      <Button variant="secondary" size="compact" disabled={disabled || recording || !model.capture?.steps.length}
        onClick={() => { if (model.capture) onAdopt(structuredClone(model.capture.steps)); }}>采用录制</Button>
    </div>
    <p className="ccc-control-caption" role="status">{recording ? `正在录制 · ${model.capture?.steps.length ?? 0} 步 · ${Math.floor((model.capture?.elapsedMs ?? 0) / 1000)} 秒，按 Esc 停止` : model.capture?.reason ?? "最多256步 · 2分钟"}</p>
    {model.error && <p className="ccc-control-error" role="alert"><Icon name="error" size={16} />{model.error}</p>}
    {recording && <span className="ccc-control-caption is-warn"><Icon name="warning" size={16} />按键照常输入 · 失焦停止</span>}
  </div>;
}
