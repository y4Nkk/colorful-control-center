/** 全局通知唯一渲染 Host：app 层挂载一次，渲染 use-toast 单例队列。 */
import { Surface } from "./Surface";
import { Button } from "./Button";
import { useToast, type ToastKind } from "./use-toast";
import { Icon, type IconName } from "./Icon";
import type { BatteryColdWarning } from "../api/client";

const STATUS_ICONS: Record<ToastKind, IconName> = {
  info: "info", success: "check-circle", warning: "warning", error: "error",
};

export function Toaster({ coldWarning, dismissColdWarning }: { coldWarning: BatteryColdWarning | null; dismissColdWarning: () => void }) {
  const { toasts, dismiss } = useToast();

  return (
    <div className="ccc-toaster" role="region" aria-label="通知">
      {coldWarning && <Surface as="div" variant="card" className="ccc-toast" data-kind="warning" role="alert">
        <Icon name="warning" className="ccc-toast-status" />
        <div className="ccc-toast-body"><div className="ccc-toast-title">电池温度较低</div>
          <div className="ccc-toast-description">电池温度 {coldWarning.temperatureC}°C，电量 {coldWarning.batteryPercent}%。请接入电源。</div></div>
        <Button variant="secondary" size="compact" onClick={dismissColdWarning}>知道了</Button>
      </Surface>}
      {toasts.map((item) => (
        <Surface as="div" variant="card" key={item.id} className="ccc-toast" data-kind={item.kind}>
          <Icon name={STATUS_ICONS[item.kind]} className="ccc-toast-status" />
          <div className="ccc-toast-body">
            <div className="ccc-toast-title">{item.title}</div>
            {item.description && <div className="ccc-toast-description">{item.description}</div>}
          </div>
          <Button variant="ghost" size="icon-compact" type="button" className="ccc-toast-close" aria-label="关闭通知" onClick={() => dismiss(item.id)}>
            <Icon name="close" size={16} />
          </Button>
        </Surface>
      ))}
    </div>
  );
}
