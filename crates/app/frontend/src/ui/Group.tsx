import { cva, type VariantProps } from "class-variance-authority";
import type { HTMLAttributes, ReactNode } from "react";

import { cn } from "./cn";

/**
 * Inset grouped lists, as in System Settings: a rounded container, rows split by hairlines
 * that start at the text column (`--inset`), a small header and a footnote outside it.
 * Each row draws its own hairline (a real element, so text never sits on a pseudo-element
 * background); the first row's is hidden, also when rows are wrapped in `li`.
 */
export function Group({
  className,
  as: As = "div",
  ...props
}: HTMLAttributes<HTMLElement> & { as?: "div" | "dl" | "ol" | "ul" }) {
  return (
    <As
      className={cn(
        "overflow-hidden rounded-group bg-group shadow-group",
        "[&>:first-child>.hairline]:hidden [&>:first-child>*>.hairline]:hidden",
        className,
      )}
      {...props}
    />
  );
}

/** The separator above a row; its row is `relative`. */
export function Hairline() {
  return (
    <span
      aria-hidden="true"
      className="hairline pointer-events-none absolute top-0 right-0 left-[var(--inset,14px)] h-px bg-sep"
    />
  );
}

/** Rows that lead with a tile: hairlines start after it. */
export const TILE_INSET = "[--inset:55px]";

export function GroupHeader({ className, children, aside, id }: { className?: string; children: ReactNode; aside?: ReactNode; id?: string }) {
  return (
    <div className={cn("mb-1.5 flex items-baseline gap-2 px-3.5", className)}>
      <h2 id={id} className="text-label font-semibold text-fg-2">
        {children}
      </h2>
      {aside ? <span className="ml-auto text-label text-fg-2">{aside}</span> : null}
    </div>
  );
}

export function GroupFootnote({ className, icon, children }: { className?: string; icon?: ReactNode; children: ReactNode }) {
  return (
    <p className={cn("mt-2 flex gap-2 px-3.5 text-row text-fg-2", className)}>
      {icon ? (
        <span className="mt-0.5" aria-hidden="true">
          {icon}
        </span>
      ) : null}
      <span>{children}</span>
    </p>
  );
}

const tileVariants = cva(
  "flex size-7 shrink-0 items-center justify-center rounded-tile text-[#fffbfb] shadow-[inset_0_0_0_0.5px_rgb(0_0_0/0.12)]",
  {
    variants: {
      tone: {
        graphite: "[background:var(--tile)]",
        green: "[background:var(--tile-green)]",
        accent: "[background:var(--tile-accent)]",
      },
    },
    defaultVariants: { tone: "graphite" },
  },
);

/** The small rounded icon square that leads a row. */
export function Tile({ tone, children }: VariantProps<typeof tileVariants> & { children: ReactNode }) {
  return (
    <span className={tileVariants({ tone })} aria-hidden="true">
      {children}
    </span>
  );
}

/** A step number in the same place a tile would be. */
export function StepNumber({ n }: { n: number }) {
  return (
    <span
      aria-hidden="true"
      className="flex size-7 shrink-0 items-center justify-center rounded-full bg-fill text-row font-semibold text-fg tabular-nums"
    >
      {n}
    </span>
  );
}

/** A row: optional leading element, a title with an optional second line, a trailing element. */
export function Row({
  lead,
  title,
  detail,
  trailing,
  className,
  strong,
}: {
  lead?: ReactNode;
  title: ReactNode;
  detail?: ReactNode;
  trailing?: ReactNode;
  className?: string;
  /** The title in medium weight, when a detail line follows. */
  strong?: boolean;
}) {
  return (
    <div className={cn("relative flex min-h-11 items-center gap-3 px-3.5 py-2.5", className)}>
      <Hairline />
      {lead}
      <div className="min-w-0 flex-1">
        <div className={cn("text-row text-fg", strong && "font-medium")}>{title}</div>
        {detail ? <div className="mt-0.5 text-row text-fg-2">{detail}</div> : null}
      </div>
      {trailing}
    </div>
  );
}

/**
 * A label and value row ("Software — 5.10"), inside a `Group as="dl"`. Its hairline is a top
 * border (a `dl` row may hold only `dt` and `dd`), starting at the text like the others.
 */
export function ValueRow({ label, value }: { label: ReactNode; value: ReactNode }) {
  return (
    <div className="ml-3.5 flex min-h-11 items-center gap-4 border-t border-sep py-2.5 pr-3.5 first:border-t-0">
      <dt className="shrink-0 text-row text-fg">{label}</dt>
      <dd className="ml-auto min-w-0 text-right text-row break-words text-fg-2">{value}</dd>
    </div>
  );
}

/** The labelled list of plain values the flow builds (what's shared, what the app sees). */
export function Facts({ items }: { items: { label: string; value: string }[] }) {
  return (
    <Group as="dl">
      {items.map((item) => (
        <ValueRow key={item.label} label={item.label} value={item.value} />
      ))}
    </Group>
  );
}
