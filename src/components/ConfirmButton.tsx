/**
 * 危险动作两步确认按钮（DESIGN.md 组件所有权）：第一次点击进入确认态，
 * 3 秒未确认自动复位；确认态使用错误红。
 */
import { useEffect, useState } from "react";

export function ConfirmButton({
  label,
  confirmLabel,
  onConfirm,
}: {
  label: string;
  confirmLabel: string;
  onConfirm: () => void;
}) {
  const [arming, setArming] = useState(false);
  useEffect(() => {
    if (!arming) return;
    const timer = setTimeout(() => setArming(false), 3000);
    return () => clearTimeout(timer);
  }, [arming]);
  return (
    <button
      type="button"
      className="ccc-button"
      style={arming ? { background: "var(--ccc-danger)" } : undefined}
      onClick={() => {
        if (arming) {
          setArming(false);
          onConfirm();
        } else {
          setArming(true);
        }
      }}
    >
      {arming ? confirmLabel : label}
    </button>
  );
}
