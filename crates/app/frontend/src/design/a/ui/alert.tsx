import { cva, type VariantProps } from "class-variance-authority";
import type { ComponentProps, ReactNode } from "react";

import { cn } from "./cn";

export const alertVariants = cva(
  "wa:grid wa:grid-cols-[auto_1fr] wa:gap-x-3 wa:gap-y-1 wa:rounded-2xl wa:p-4 wa:text-[15px] wa:leading-normal",
  {
    variants: {
      tone: {
        sand: "wa:bg-sand wa:text-ink wa:[--alert-icon:var(--a-sand-ink)]",
        accent: "wa:bg-accent-soft wa:text-ink wa:[--alert-icon:var(--a-accent-soft-ink)]",
        success: "wa:bg-success-soft wa:text-ink wa:[--alert-icon:var(--a-success-ink)]",
        plain: "wa:bg-surface wa:text-ink wa:shadow-card wa:[--alert-icon:var(--a-accent-ink)]",
      },
    },
    defaultVariants: { tone: "sand" },
  },
);

/** A tinted note with an icon, a title and a body (shadcn's Alert, warmer). */
export function Alert({
  className,
  tone,
  icon,
  title,
  children,
  ...props
}: Omit<ComponentProps<"div">, "title"> & VariantProps<typeof alertVariants> & { icon?: ReactNode; title?: ReactNode }) {
  return (
    <div data-slot="alert" className={cn(alertVariants({ tone }), className)} {...props}>
      <span
        aria-hidden
        className="wa:row-span-2 wa:mt-0.5 wa:grid wa:size-8 wa:place-items-center wa:rounded-full wa:bg-surface/70 wa:text-(--alert-icon) wa:[&_svg]:size-[18px]"
      >
        {icon}
      </span>
      {title ? <p className="wa:font-semibold wa:text-ink wa:self-center">{title}</p> : null}
      <div className={cn("wa:text-ink", !title && "wa:self-center")}>{children}</div>
    </div>
  );
}
