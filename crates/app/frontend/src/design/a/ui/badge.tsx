import { cva, type VariantProps } from "class-variance-authority";
import type { ComponentProps } from "react";

import { cn } from "./cn";

export const badgeVariants = cva(
  "wa:inline-flex wa:items-center wa:gap-1.5 wa:rounded-full wa:font-semibold wa:whitespace-nowrap wa:[&_svg]:size-3.5 wa:[&_svg]:shrink-0",
  {
    variants: {
      tone: {
        neutral: "wa:bg-sunken wa:text-ink-2",
        accent: "wa:bg-accent-soft wa:text-accent-soft-ink",
        success: "wa:bg-success-soft wa:text-success-ink",
        sand: "wa:bg-sand wa:text-sand-ink",
        outline: "wa:border wa:border-line wa:text-ink-2",
      },
      size: {
        md: "wa:px-2.5 wa:py-1 wa:text-[13px] wa:leading-none",
        lg: "wa:px-3 wa:py-1.5 wa:text-[14px] wa:leading-none",
      },
    },
    defaultVariants: { tone: "neutral", size: "md" },
  },
);

export function Badge({ className, tone, size, ...props }: ComponentProps<"span"> & VariantProps<typeof badgeVariants>) {
  return <span data-slot="badge" className={cn(badgeVariants({ tone, size }), className)} {...props} />;
}
