import { cva, type VariantProps } from "class-variance-authority";
import type { ComponentProps } from "react";

import { cn } from "./cn";

/**
 * Native-sized push buttons: 32 px tall, 14 px text, the platform's corner radius.
 * - primary: the accent fill, the screen's one main action;
 * - secondary: a bordered neutral push button (3:1 border), the quiet companion;
 * - toolbar: borderless, a fill on hover, for the title bar.
 */
export const buttonVariants = cva(
  [
    "inline-flex items-center justify-center gap-1.5 whitespace-nowrap select-none",
    "rounded-control font-medium transition-[background-color,box-shadow,transform,opacity]",
    "duration-150 ease-native active:scale-[0.98]",
    "disabled:pointer-events-none disabled:opacity-45",
  ],
  {
    variants: {
      variant: {
        primary: [
          "bg-accent text-on-accent shadow-control",
          "hover:bg-accent-hover active:bg-accent-active",
        ],
        secondary: [
          "border border-control bg-control-bg text-fg shadow-control",
          "hover:bg-group-hover active:bg-group-active",
        ],
        toolbar: "text-fg hover:bg-fill active:bg-fill-strong",
      },
      size: {
        default: "h-8 min-w-20 px-3.5 text-row",
        sm: "h-7 px-2.5 text-label",
        icon: "size-7",
      },
    },
    defaultVariants: { variant: "secondary", size: "default" },
  },
);

export type ButtonProps = ComponentProps<"button"> & VariantProps<typeof buttonVariants>;

export function Button({ className, variant, size, type = "button", ...props }: ButtonProps) {
  return <button type={type} className={cn(buttonVariants({ variant, size }), className)} {...props} />;
}
