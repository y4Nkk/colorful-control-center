/** 全局通知唯一渲染 Host：app 层挂载一次，渲染 use-toast 单例队列。 */
import { useToast } from "./use-toast";

export function Toaster() {
  const { toasts, dismiss } = useToast();

  return (
    <div className="ccc-toaster" role="region" aria-label="通知">
      {toasts.map((item) => (
        <div key={item.id} className="ccc-toast" data-kind={item.kind}>
          <span className="ccc-toast-dot" aria-hidden="true" />
          <div className="ccc-toast-body">
            <div className="ccc-toast-title">{item.title}</div>
            {item.description && <div className="ccc-toast-description">{item.description}</div>}
          </div>
          <button type="button" className="ccc-toast-close" aria-label="关闭通知" onClick={() => dismiss(item.id)}>
            ×
          </button>
        </div>
      ))}
    </div>
  );
}
