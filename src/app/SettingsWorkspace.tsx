/**
 * 设置（Phase 4 接入偏好/托盘/导入导出）。当前为占位 + 全局通知冒烟：
 * 演示「模块位置 → 全局服务」标准接入方式（toast() 命令式调用）。
 */
import { toast } from "../components/use-toast";
import { PagePlaceholder } from "../components/PagePlaceholder";

export function SettingsWorkspace(_props: { active: boolean }) {
  return (
    <PagePlaceholder
      title="设置"
      phase="Phase 4"
      description="应用偏好、开机自启、托盘常驻、档案导入导出与 OEM 杂项（键盘灯等）。"
    >
      <button
        type="button"
        className="ccc-button"
        onClick={() =>
          toast({
            kind: "success",
            title: "通知链路正常",
            description: "模块通过全局 toast() 单例接入，无需自建浮层。",
          })
        }
      >
        发送测试通知
      </button>
    </PagePlaceholder>
  );
}
