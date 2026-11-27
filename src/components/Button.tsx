import type { ComponentProps } from "react";
import { Icon, type IconName } from "./Icon";

type ButtonVariant = "primary" | "secondary" | "neutral" | "ghost" | "field" | "selection" | "navigation" | "handle" | "swatch" | "keycap";
interface ButtonProps extends ComponentProps<"button"> {
  variant?: ButtonVariant;
  size?: "default" | "compact" | "icon" | "icon-compact" | "custom";
  tone?: "normal" | "danger";
  icon?: IconName;
  busy?: boolean;
}

export function Button({ variant = "primary", size, tone = "normal", type = "button", className,
  icon, busy = false, children, ...props }: ButtonProps) {
  const custom = ["selection", "navigation", "handle", "swatch", "keycap"].includes(variant);
  const glyph = busy ? "loading" : icon;
  // 手柄、下拉触发器等自己传 aria-busy，在暂时锁定时保留焦点；这里不能盖掉调用方的值。
  return <button {...props} type={type} className={["ccc-button", className].filter(Boolean).join(" ")}
    disabled={props.disabled || busy} data-variant={variant} data-size={size ?? (custom ? "custom" : "default")} data-tone={tone}
    aria-busy={busy || props["aria-busy"] || undefined}>
    {glyph && <Icon name={glyph} size={size === "icon" ? 20 : 16} />}{children}
  </button>;
}
