import { ChevronDown } from "lucide-react";
import { Collapsible, Progress as ProgressPrimitive } from "radix-ui";
import type { ComponentProps, ReactNode } from "react";

import { cn } from "./cn";

/** A card whose body opens and closes (Radix Collapsible), with an animated height. */
export function CollapsibleCard(props: {
  title: ReactNode;
  icon?: ReactNode;
  defaultOpen?: boolean;
  children: ReactNode;
  className?: string;
}) {
  return (
    <Collapsible.Root
      defaultOpen={props.defaultOpen}
      className={cn("wa:group/col wa:rounded-2xl wa:bg-surface wa:shadow-card wa:border wa:border-line", props.className)}
    >
      <Collapsible.Trigger
        data-ring-inset
        className="wa:flex wa:w-full wa:min-h-13 wa:items-center wa:gap-3 wa:rounded-2xl wa:px-4 wa:py-3 wa:text-left wa:font-semibold wa:text-ink wa:transition-colors wa:hover:bg-sunken/50"
      >
        {props.icon ? (
          <span aria-hidden className="wa:grid wa:size-8 wa:place-items-center wa:rounded-full wa:bg-accent-soft wa:text-accent-soft-ink wa:[&_svg]:size-[18px]">
            {props.icon}
          </span>
        ) : null}
        <span className="wa:flex-1">{props.title}</span>
        <ChevronDown
          aria-hidden
          className="wa:size-5 wa:text-ink-2 wa:transition-transform wa:duration-200 wa:group-data-[state=open]/col:rotate-180"
        />
      </Collapsible.Trigger>
      <Collapsible.Content className="a-collapse wa:overflow-hidden">
        <div className="wa:px-4 wa:pb-4">{props.children}</div>
      </Collapsible.Content>
    </Collapsible.Root>
  );
}

/** A determinate progress bar (Radix Progress). */
export function Progress({ value, className, ...props }: ComponentProps<typeof ProgressPrimitive.Root>) {
  return (
    <ProgressPrimitive.Root
      value={value}
      className={cn("wa:relative wa:h-2.5 wa:w-full wa:overflow-hidden wa:rounded-full wa:bg-sunken", className)}
      {...props}
    >
      <ProgressPrimitive.Indicator
        className="wa:h-full wa:w-full wa:rounded-full wa:bg-accent wa:transition-transform wa:duration-500 wa:ease-out-soft"
        style={{ transform: `translateX(-${100 - (value ?? 0)}%)` }}
      />
    </ProgressPrimitive.Root>
  );
}

/** A small spinner drawn as a warm arc. */
export function Spinner({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 20 20" aria-hidden className={cn("wa:size-[18px]", className)}>
      <circle cx="10" cy="10" r="7.5" fill="none" stroke="currentColor" strokeOpacity="0.22" strokeWidth="2.5" />
      <path
        className="a-spinner"
        style={{ transformOrigin: "10px 10px" }}
        d="M10 2.5a7.5 7.5 0 0 1 7.5 7.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="2.5"
        strokeLinecap="round"
      />
    </svg>
  );
}
