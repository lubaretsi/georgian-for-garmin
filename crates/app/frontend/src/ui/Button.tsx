import type { ButtonHTMLAttributes, ReactNode } from "react";

export type ButtonKind = "primary" | "secondary" | "quiet";

export function Button(
  props: { kind?: ButtonKind; children: ReactNode } & Omit<ButtonHTMLAttributes<HTMLButtonElement>, "type">,
) {
  const { kind = "secondary", className, children, ...rest } = props;
  return (
    <button type="button" className={`button button-${kind}${className ? ` ${className}` : ""}`} {...rest}>
      {children}
    </button>
  );
}
