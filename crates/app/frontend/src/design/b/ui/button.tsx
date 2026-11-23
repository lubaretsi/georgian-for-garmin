import { cva, type VariantProps } from "class-variance-authority";
import type { ComponentProps } from "react";

import { cn } from "../lib/cn";

/**
 * Native-sized push buttons: 32 px tall, 14 px text, the platform's corner radius.
 * - primary: the accent fill, the screen's one main action;
 * - secondary: a bordered neutral push button (3:1 border), the quiet companion;
 * - toolbar: borderless, a fill on hover, for the title bar.
 */
export const buttonVariants = cva(
  [
    "nb:inline-flex nb:items-center nb:justify-center nb:gap-1.5 nb:whitespace-nowrap nb:select-none",
    "nb:rounded-control nb:font-medium nb:transition-[background-color,box-shadow,transform,opacity]",
    "nb:duration-150 nb:ease-native nb:active:scale-[0.98]",
    "nb:disabled:pointer-events-none nb:disabled:opacity-45",
  ],
  {
    variants: {
      variant: {
        primary: [
          "nb:bg-accent nb:text-on-accent nb:shadow-control",
          "nb:bg-[linear-gradient(180deg,rgb(255_255_255/0.1),rgb(255_255_255/0))]",
          "nb:hover:bg-accent-hover nb:active:bg-accent-active",
        ],
        secondary: [
          "nb:bg-control-bg nb:text-fg nb:border nb:border-control nb:shadow-control",
          "nb:hover:bg-group-hover nb:active:bg-group-active",
        ],
        toolbar: ["nb:text-fg nb:hover:bg-fill nb:active:bg-fill-strong"],
      },
      size: {
        default: "nb:h-8 nb:min-w-20 nb:px-3.5 nb:text-row",
        sm: "nb:h-7 nb:px-2.5 nb:text-label",
        icon: "nb:size-7",
      },
    },
    defaultVariants: { variant: "secondary", size: "default" },
  },
);

export type ButtonProps = ComponentProps<"button"> & VariantProps<typeof buttonVariants>;

export function Button({ className, variant, size, type = "button", ...props }: ButtonProps) {
  return <button type={type} className={cn(buttonVariants({ variant, size }), className)} {...props} />;
}
