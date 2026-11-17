import type { ComponentProps } from "react";

export function Textarea({ className, rows = 4, ...props }: ComponentProps<"textarea">) {
  return <textarea {...props} rows={rows} data-multiline="true"
    className={["ccc-field", "ccc-input", className].filter(Boolean).join(" ")} />;
}
