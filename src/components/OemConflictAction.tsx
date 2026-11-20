import { Button } from "./Button";
import { findOemConflict } from "../api/client";
import { useOemConflict } from "./OemConflictHost";

export function OemConflictAction({ issues, disabled, onClosed }: {
  issues: Parameters<typeof findOemConflict>[0]; disabled?: boolean; onClosed?: () => void;
}) {
  const { review } = useOemConflict();
  const conflict = findOemConflict(issues);
  if (!conflict) return null;
  return <div className="ccc-actions">
    <Button variant="secondary" size="compact" aria-haspopup="dialog" disabled={disabled}
      onClick={() => review([conflict.message], onClosed)}>{conflict.executable === "FnKey.exe" ? "关闭原厂软件" : "处理原厂软件冲突"}</Button>
  </div>;
}
