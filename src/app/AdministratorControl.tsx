import { Button } from "../components/Button";
import { Icon, type IconName } from "../components/Icon";
import { Tooltip } from "../components/Tooltip";
import { useAdministrator, type AccessState } from "../components/AdministratorHost";

const PRESENTATION = {
  loading: { label: "正在读取管理员权限", icon: "loading" },
  unavailable: { label: "无法确认管理员权限", icon: "warning" },
  canElevate: { label: "以管理员运行", icon: "shield-off" },
  standard: { label: "当前 Windows 账户无法提升为管理员", icon: "shield-off" },
  elevated: { label: "已以管理员身份运行", icon: "shield-check" },
} satisfies Record<AccessState, { label: string; icon: IconName }>;

export function AdministratorControl({ pending }: { pending: boolean }) {
  const { access, request } = useAdministrator();
  const { label, icon } = PRESENTATION[access];
  const canRestart = access === "canElevate" && !pending;
  return <Tooltip content={label} side="bottom">
    <Button variant="ghost" size="icon" className="ccc-window-button" data-administrator-access={access}
      aria-label={label} aria-disabled={!canRestart} aria-haspopup={access === "canElevate" ? "dialog" : undefined}
      onClick={() => { if (canRestart) request(); }}>
      <Icon name={icon} size={16} />
    </Button>
  </Tooltip>;
}
