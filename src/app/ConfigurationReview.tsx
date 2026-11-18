import { useRef, useState, useSyncExternalStore } from "react";
import { Button } from "../components/Button";
import { Dialog } from "../components/Dialog";
import { Surface } from "../components/Surface";
import { toast } from "../components/use-toast";
import type { ActionRegistry, RegisteredAction } from "../components/ConfigurationAction";

type Review = { restore: boolean; groups: RegisteredAction[][]; selected: number };

function reviewGroups(actions: RegisteredAction[], restore: boolean) {
  const eligible = actions.filter(action => Boolean(action.restore) === restore);
  const together = eligible.filter(action => action.together);
  const groups = eligible.filter(action => !action.together).map(action => [action]);
  return together.length ? [together, ...groups] : groups;
}

function groupLabel(group: RegisteredAction[]) {
  return group.length === 1 ? group[0].label : group.map(action => action.label).join("、");
}

export function ConfigurationReview({ registry, active }: { registry: ActionRegistry; active: boolean }) {
  const actions = useSyncExternalStore(registry.subscribe, registry.snapshot);
  const [review, setReview] = useState<Review | null>(null);
  const [busy, setBusy] = useState(false);
  const running = useRef(false);
  const applies = actions.filter(action => !action.restore);
  const restores = actions.filter(action => action.restore);
  const group = review?.groups[review.selected];
  const expired = !!group?.some(action => !registry.current(action));
  const close = () => { if (!running.current) setReview(null); };
  const open = (restore: boolean) => {
    const groups = reviewGroups(registry.snapshot(), restore);
    if (groups.length) setReview({ restore, groups, selected: 0 });
  };
  const submit = async () => {
    if (!group || expired || running.current || !active) return;
    running.current = true;
    setBusy(true);
    if (group.length === 1) setReview(null);
    try { await submitGroup(group, registry); }
    finally { running.current = false; setBusy(false); setReview(null); }
  };
  return <>
    {(applies.length > 0 || restores.length > 0 || busy) && <Surface className="ccc-page-actions" variant="group" role="region" aria-label="页面操作">
      <span role="status" className="ccc-page-action-status">{busy ? "正在提交…" : applies.length > 0 ? `${applies.length} 项待应用` : ""}</span>
      {restores.length > 0 && <Button icon="undo" variant="secondary" size="compact" aria-haspopup="dialog"
        aria-description={groupLabel(restores)}
        disabled={busy} onClick={() => open(true)}>恢复设置</Button>}
      {applies.length > 0 && <Button icon="check" aria-haspopup="dialog" aria-description={groupLabel(applies)}
        disabled={busy} onClick={() => open(false)}>预览更改</Button>}
    </Surface>}
    <Dialog title={review?.restore ? "恢复设置" : "预览更改"} open={review !== null} busy={busy} onClose={close}
      actions={<><Button variant="secondary" disabled={busy} onClick={close}>取消</Button>
        <Button disabled={busy || expired || !group} icon={busy ? "loading" : review?.restore ? "undo" : "check"}
          onClick={() => void submit()}>{busy ? "正在提交…" : group?.length === 1 && group[0].confirmLabel
            ? group[0].confirmLabel : review?.restore ? "确认恢复" : "确认应用"}</Button></>}>
      {review && review.groups.length > 1 && <div className="ccc-review-choices" aria-label="选择确认范围">
        {review.groups.map((option, index) => <Button key={option[0].id} variant="secondary" size="compact"
          aria-pressed={review.selected === index} disabled={busy} onClick={() => setReview({ ...review, selected: index })}>{groupLabel(option)}</Button>)}
      </div>}
      {group?.map(action => <section className="ccc-review-section" key={action.id}>
        <h3>{action.label}</h3>{action.children}
      </section>)}
      {expired && !busy && <p className="ccc-control-error" role="alert">设置已变化。关闭后重新预览。</p>}
    </Dialog>
  </>;
}

async function submitGroup(group: RegisteredAction[], registry: ActionRegistry) {
  let completed = 0;
  for (const action of group) {
    const current = registry.current(action);
    const remaining = group.length - completed - 1;
    const progress = completed ? `已处理 ${completed} 项。其余草稿已保留。` : "草稿已保留。";
    if (!current) {
      toast({ kind: "warning", title: `${action.label}已变化，请重新预览`, description: progress });
      return;
    }
    try {
      if (await current.onConfirm() === false) {
        if (remaining > 0) toast({ kind: "warning", title: `${action.label}未完成，已停止提交`, description: progress });
        return;
      }
      completed++;
    } catch (error) {
      toast({ kind: "error", title: `${action.label}失败`, description: `${String(error)} ${progress}` });
      return;
    }
  }
}
