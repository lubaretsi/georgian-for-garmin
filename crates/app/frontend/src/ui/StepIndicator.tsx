import { Check } from "lucide-react";
import { motion } from "motion/react";

import type { Copy } from "../copy";
import { cn } from "./cn";

export const STEPS = ["connect", "install", "restart", "check"] as const;
export type StepId = (typeof STEPS)[number];

/**
 * The journey as a compact segmented indicator in the toolbar (§10): the current step is the
 * raised segment with its name, done steps show a check, later steps their number. With
 * `allDone` (Georgian works) every step shows a check, the current one in green. Every name
 * is read by screen readers; nothing here is clickable.
 */
export function StepIndicator({ current, allDone, labels }: { current: StepId; allDone?: boolean; labels: Copy["steps"] }) {
  const index = STEPS.indexOf(current);
  return (
    <nav aria-label={labels.label} className="min-w-0">
      <ol className="flex h-7 items-center gap-0.5 rounded-[9px] bg-fill p-0.5">
        {STEPS.map((step, i) => {
          const state = i < index || (allDone && i !== index) ? "done" : i === index ? "current" : "next";
          return (
            <li
              key={step}
              aria-current={state === "current" ? "step" : undefined}
              className={cn(
                "relative flex h-full items-center justify-center gap-1.5 rounded-[7px]",
                state === "current" ? "px-2.5" : "w-7",
              )}
            >
              {state === "current" ? (
                <motion.span
                  layoutId="step-thumb"
                  transition={{ type: "spring", stiffness: 520, damping: 40 }}
                  className="absolute inset-0 rounded-[7px] bg-control-bg shadow-thumb"
                  aria-hidden="true"
                />
              ) : null}
              <span
                aria-hidden="true"
                className={cn(
                  "relative flex size-[18px] items-center justify-center rounded-full text-label font-semibold tabular-nums",
                  state === "current" && (allDone ? "bg-success text-on-accent" : "bg-accent text-on-accent"),
                  state === "done" && "text-fg",
                  state === "next" && "text-fg-2",
                )}
              >
                {state === "done" || allDone ? <Check size={state === "current" ? 12 : 14} strokeWidth={2.8} /> : i + 1}
              </span>
              <span className={cn("relative text-label font-semibold whitespace-nowrap text-fg", state !== "current" && "sr-only")}>
                {labels[step]}
                {state === "done" || allDone ? <span className="sr-only">, {labels.done}</span> : null}
              </span>
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
