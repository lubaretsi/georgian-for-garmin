import { cva } from "class-variance-authority";
import { CircleAlert, CircleCheck, Info, TriangleAlert } from "lucide-react";
import type { ReactNode } from "react";

import { cn } from "./cn";

const alertVariants = cva("flex gap-3 rounded-group px-3.5 py-3 text-row", {
  variants: {
    tone: {
      info: "bg-group text-fg shadow-group",
      caution: "bg-caution-soft text-fg",
      danger: "bg-danger-soft text-fg",
      success: "bg-success-soft text-fg",
    },
  },
});

const ICONS = { info: Info, caution: TriangleAlert, danger: CircleAlert, success: CircleCheck } as const;
const ICON_COLOR = { info: "text-fg-2", caution: "text-caution-ink", danger: "text-danger-ink", success: "text-success-ink" } as const;

/**
 * A short, important message in the content. `live` makes it an alert that is announced when
 * it appears (errors, §2.9); the others are plain text in the reading order.
 */
export function Alert(props: { tone: "info" | "caution" | "danger" | "success"; title?: ReactNode; live?: boolean; children?: ReactNode }) {
  const Icon = ICONS[props.tone];
  return (
    <div role={props.live ? "alert" : undefined} className={alertVariants({ tone: props.tone })}>
      <Icon size={18} strokeWidth={2} className={cn("mt-px shrink-0", ICON_COLOR[props.tone])} aria-hidden="true" />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        {props.title ? <p className="font-semibold">{props.title}</p> : null}
        {props.children}
      </div>
    </div>
  );
}
