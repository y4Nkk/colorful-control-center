import type { ComponentProps } from "react";

export function Slider({ className, ...props }: Omit<ComponentProps<"input">, "type">) {
  return <input {...props} type="range" className={["ccc-range", className].filter(Boolean).join(" ")} />;
}
