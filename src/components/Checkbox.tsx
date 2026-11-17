import type { ComponentProps, ReactNode } from "react";
import { Icon } from "./Icon";

interface CheckboxProps extends Omit<ComponentProps<"input">, "type" | "size" | "children"> {
  label: ReactNode;
}

export function Checkbox({ label, className, ...props }: CheckboxProps) {
  return <label className={["ccc-checkbox", className].filter(Boolean).join(" ")}>
    <input {...props} type="checkbox" />
    <span className="ccc-checkbox-box" aria-hidden="true"><Icon name="check" /></span>
    <span>{label}</span>
  </label>;
}
