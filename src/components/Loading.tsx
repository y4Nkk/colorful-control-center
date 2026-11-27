import { Icon } from "./Icon";

/** 等待状态唯一原语。嵌入已有的常驻 status 区域时传 embedded，避免嵌套读屏区域；className 只用于布局。 */
export function Loading({ label, embedded = false, className }: { label: string; embedded?: boolean; className?: string }) {
  return <span className={["ccc-loading", className].filter(Boolean).join(" ")} role={embedded ? undefined : "status"}>
    <Icon name="loading" size={16} />
    <span>{label}</span>
  </span>;
}
