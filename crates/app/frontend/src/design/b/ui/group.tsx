import { cva, type VariantProps } from "class-variance-authority";
import type { HTMLAttributes, ReactNode } from "react";

import { cn } from "../lib/cn";

/**
 * Inset grouped lists, as in System Settings: a rounded container, rows split by hairlines
 * that start after the row's icon, and a small header and footnote outside the container.
 */
export function Group({
  className,
  as: As = "div",
  ...props
}: HTMLAttributes<HTMLElement> & { as?: "div" | "dl" | "ol" | "ul" }) {
  return (
    <As
      className={cn(
        "nb:overflow-hidden nb:rounded-group nb:bg-group nb:shadow-group",
        // Hairlines between rows, inset to the text column (--b-inset), as native lists do.
        "nb:[&>*+*]:relative nb:[&>*+*]:before:absolute nb:[&>*+*]:before:top-0 nb:[&>*+*]:before:right-0",
        "nb:[&>*+*]:before:left-[var(--b-inset,14px)] nb:[&>*+*]:before:h-px nb:[&>*+*]:before:bg-sep",
        "nb:[&>*+*]:before:content-['']",
        className,
      )}
      {...props}
    />
  );
}

export function GroupHeader({ className, children, aside, id }: { className?: string; children: ReactNode; aside?: ReactNode; id?: string }) {
  return (
    <div className={cn("nb:mb-1.5 nb:flex nb:items-baseline nb:gap-2 nb:px-3.5", className)}>
      <h2 id={id} className="nb:text-label nb:font-semibold nb:text-fg-2">
        {children}
      </h2>
      {aside ? <span className="nb:ml-auto nb:text-label nb:text-fg-2">{aside}</span> : null}
    </div>
  );
}

export function GroupFootnote({ className, icon, children }: { className?: string; icon?: ReactNode; children: ReactNode }) {
  return (
    <p className={cn("nb:mt-2 nb:flex nb:gap-2 nb:px-3.5 nb:text-row nb:text-fg-2", className)}>
      {icon ? <span className="nb:mt-0.5 nb:text-fg-2" aria-hidden="true">{icon}</span> : null}
      <span>{children}</span>
    </p>
  );
}

const tileVariants = cva(
  "nb:flex nb:size-7 nb:shrink-0 nb:items-center nb:justify-center nb:rounded-tile nb:text-[#fffbfb] nb:shadow-[inset_0_0_0_0.5px_rgb(0_0_0/0.12)]",
  {
    variants: {
      tone: {
        graphite: "nb:[background:var(--b-tile)]",
        green: "nb:[background:var(--b-tile-green)]",
        accent: "nb:[background:var(--b-tile-accent)]",
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

/** A row: optional leading element, a title with an optional second line, and a trailing value. */
export function Row({
  lead,
  title,
  detail,
  trailing,
  className,
  titleClassName,
}: {
  lead?: ReactNode;
  title: ReactNode;
  detail?: ReactNode;
  trailing?: ReactNode;
  className?: string;
  titleClassName?: string;
}) {
  return (
    <div className={cn("nb:flex nb:min-h-11 nb:items-center nb:gap-3 nb:px-3.5 nb:py-2.5", className)}>
      {lead}
      <div className="nb:min-w-0 nb:flex-1">
        <div className={cn("nb:text-row nb:text-fg", titleClassName)}>{title}</div>
        {detail ? <div className="nb:mt-0.5 nb:text-row nb:text-fg-2">{detail}</div> : null}
      </div>
      {trailing}
    </div>
  );
}

/** A label and value row ("Software — 5.10"). */
export function ValueRow({ label, value }: { label: ReactNode; value: ReactNode }) {
  return (
    <div className="nb:flex nb:min-h-11 nb:items-center nb:gap-4 nb:px-3.5 nb:py-2.5">
      <dt className="nb:text-row nb:text-fg">{label}</dt>
      <dd className="nb:ml-auto nb:min-w-0 nb:text-right nb:text-row nb:text-fg-2">{value}</dd>
    </div>
  );
}
