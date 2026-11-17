import type { ComponentProps } from "react";

interface InputProps extends Omit<ComponentProps<"input">, "type" | "size"> {
  type?: "text" | "search" | "number" | "color" | "file";
  size?: "default" | "compact";
}

export function Input({ type = "text", size = "default", className, ...props }: InputProps) {
  return <input {...props} type={type} data-size={size}
    className={["ccc-field", "ccc-input", className].filter(Boolean).join(" ")} />;
}
