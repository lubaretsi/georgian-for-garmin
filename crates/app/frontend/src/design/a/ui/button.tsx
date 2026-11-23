import { cva, type VariantProps } from "class-variance-authority";
import { Slot } from "radix-ui";
import type { ComponentProps } from "react";

import { cn } from "./cn";

export const buttonVariants = cva(
  [
    "wa:relative wa:inline-flex wa:items-center wa:justify-center wa:gap-2 wa:font-semibold wa:whitespace-normal wa:text-center",
    "wa:transition-[background-color,box-shadow,transform,color] wa:duration-150 wa:ease-out",
    "wa:active:translate-y-px wa:disabled:pointer-events-none",
    "wa:[&_svg]:size-[18px] wa:[&_svg]:shrink-0",
  ],
  {
    variants: {
      variant: {
        primary: [
          "wa:bg-accent wa:text-on-accent wa:shadow-button",
          "wa:hover:bg-accent-hover wa:active:bg-accent-press",
          "wa:disabled:bg-sunken wa:disabled:text-ink-2 wa:disabled:shadow-none",
        ],
        secondary: [
          "wa:bg-surface wa:text-ink wa:shadow-card wa:border wa:border-control",
          "wa:hover:bg-sunken wa:active:bg-line",
          "wa:disabled:text-ink-2",
        ],
        quiet: ["wa:text-ink-2 wa:hover:bg-sunken wa:hover:text-ink wa:active:bg-line"],
        pill: [
          "wa:bg-surface/70 wa:text-ink wa:border wa:border-line wa:hover:bg-surface wa:hover:border-control wa:active:bg-sunken",
        ],
      },
      size: {
        lg: "wa:min-h-12 wa:rounded-[14px] wa:px-5 wa:py-2.5 wa:text-[16px] wa:leading-snug",
        md: "wa:min-h-10 wa:rounded-xl wa:px-4 wa:py-2 wa:text-[15px] wa:leading-snug",
        sm: "wa:min-h-8 wa:rounded-full wa:px-3 wa:py-1 wa:text-[14px] wa:leading-tight wa:[&_svg]:size-4",
        icon: "wa:size-9 wa:rounded-full",
      },
      block: { true: "wa:w-full" },
    },
    defaultVariants: { variant: "primary", size: "lg" },
  },
);

export type ButtonProps = ComponentProps<"button"> & VariantProps<typeof buttonVariants> & { asChild?: boolean };

export function Button({ className, variant, size, block, asChild = false, type, ...props }: ButtonProps) {
  const Component = asChild ? Slot.Root : "button";
  return (
    <Component
      data-slot="button"
      type={asChild ? undefined : (type ?? "button")}
      className={cn(buttonVariants({ variant, size, block }), className)}
      {...props}
    />
  );
}
