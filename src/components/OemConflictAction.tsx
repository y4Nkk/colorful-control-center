import { Button } from "./Button";
import { useOemConflict, fnKeyConflict } from "./OemConflictHost";

export function OemConflictAction({ issues, disabled, onClosed }: {
  issues: Parameters<typeof fnKeyConflict>[0]; disabled?: boolean; onClosed?: () => void;
}) {
  const { review } = useOemConflict();
  const conflict = fnKeyConflict(issues);
  if (!conflict) return null;
  return <div className="ccc-actions">
    <Button variant="secondary" size="compact" aria-haspopup="dialog" disabled={disabled}
      onClick={() => review(issues, onClosed)}>关闭 FnKey</Button>
  </div>;
}
