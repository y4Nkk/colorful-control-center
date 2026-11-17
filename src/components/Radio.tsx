import type { ComponentProps } from "react";

interface RadioProps extends Omit<ComponentProps<"input">, "type" | "size"> {
  appearance?: "indicator" | "overlay";
}

export function Radio({ appearance = "indicator", className, ...props }: RadioProps) {
  return <input {...props} type="radio" data-appearance={appearance}
    className={["ccc-radio", className].filter(Boolean).join(" ")} />;
}
