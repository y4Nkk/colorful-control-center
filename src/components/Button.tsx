import type { ComponentProps } from "react";
import { Icon, type IconName } from "./Icon";

type ButtonVariant = "primary" | "secondary" | "ghost" | "field" | "selection" | "navigation" | "handle" | "swatch" | "keycap";
interface ButtonProps extends ComponentProps<"button"> {
  variant?: ButtonVariant;
  size?: "default" | "compact" | "icon" | "icon-compact" | "custom";
  tone?: "normal" | "danger";
  icon?: IconName;
}

export function Button({ variant = "primary", size, tone = "normal", type = "button", className,
  icon, children, ...props }: ButtonProps) {
  const custom = ["selection", "navigation", "handle", "swatch", "keycap"].includes(variant);
  return <button {...props} type={type} className={["ccc-button", className].filter(Boolean).join(" ")}
    data-variant={variant} data-size={size ?? (custom ? "custom" : "default")} data-tone={tone}>
    {icon && <Icon name={icon} size={size === "icon" ? 20 : 16} />}{children}
  </button>;
}
