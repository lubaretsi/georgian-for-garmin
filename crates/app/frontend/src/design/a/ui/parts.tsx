import { Watch as WatchIcon } from "lucide-react";
import type { ReactNode } from "react";

import { Badge } from "./badge";
import { cn } from "./cn";

/** A line with a round icon: the facts on a card. */
export function Fact(props: { icon: ReactNode; children: ReactNode; tone?: "accent" | "success" | "neutral" }) {
  return (
    <li className="wa:flex wa:items-start wa:gap-3">
      <span
        aria-hidden
        className={cn(
          "wa:mt-px wa:grid wa:size-7 wa:shrink-0 wa:place-items-center wa:rounded-full wa:[&_svg]:size-4",
          props.tone === "success"
            ? "wa:bg-success-soft wa:text-success-ink"
            : props.tone === "neutral"
              ? "wa:bg-sunken wa:text-ink-2"
              : "wa:bg-accent-soft wa:text-accent-soft-ink",
        )}
      >
        {props.icon}
      </span>
      <span className="wa:min-w-0 wa:flex-1 wa:pt-0.5 wa:text-[15px] wa:leading-normal">{props.children}</span>
    </li>
  );
}

/** A raised card. */
export function Card(props: { children: ReactNode; className?: string }) {
  return (
    <div className={cn("wa:rounded-[20px] wa:bg-surface wa:p-4 wa:shadow-card wa:border wa:border-line/70", props.className)}>
      {props.children}
    </div>
  );
}

/** The connected watch: its model, its software, and the facts about it. */
export function WatchCard(props: { model: string; software: string; children?: ReactNode }) {
  return (
    <Card className="wa:p-0">
      <div className="wa:flex wa:items-center wa:gap-3 wa:px-4 wa:py-3.5">
        <span aria-hidden className="wa:grid wa:size-10 wa:place-items-center wa:rounded-[12px] wa:bg-sunken wa:text-ink wa:[&_svg]:size-5">
          <WatchIcon />
        </span>
        <span className="wa:min-w-0 wa:flex-1 wa:text-[16px] wa:font-semibold wa:leading-tight wa:text-ink">{props.model}</span>
        <Badge tone="neutral">{props.software}</Badge>
      </div>
      {props.children ? <ul className="wa:grid wa:gap-3 wa:border-t wa:border-line wa:px-4 wa:py-3.5">{props.children}</ul> : null}
    </Card>
  );
}

/** A numbered list where each number sits in a warm disc, joined by a thin rail. */
export function Steps(props: { items: readonly ReactNode[]; start?: number }) {
  return (
    <ol className="wa:grid">
      {props.items.map((item, i) => (
        <li key={i} className="wa:relative wa:flex wa:gap-3.5 wa:pb-4 wa:last:pb-0">
          {i < props.items.length - 1 ? (
            <span aria-hidden className="wa:absolute wa:top-[32px] wa:-bottom-0.5 wa:left-[13px] wa:w-0.5 wa:rounded-full wa:bg-line" />
          ) : null}
          <span
            aria-hidden
            className="wa:relative wa:grid wa:size-7 wa:shrink-0 wa:place-items-center wa:rounded-full wa:bg-accent-soft wa:text-[14px] wa:font-bold wa:text-accent-soft-ink"
          >
            {(props.start ?? 1) + i}
          </span>
          <span className="wa:min-w-0 wa:flex-1 wa:pt-0.5 wa:text-[15px] wa:leading-normal">{item}</span>
        </li>
      ))}
    </ol>
  );
}

/** A form legend with an optional "Optional" badge. */
export function Legend(props: { id?: string; children: ReactNode; optional?: string; as?: "legend" | "p" }) {
  const Tag = props.as ?? "p";
  return (
    <Tag id={props.id} className="wa:mb-2 wa:flex wa:w-full wa:flex-wrap wa:items-center wa:gap-x-2 wa:gap-y-1 wa:text-[15px] wa:font-semibold wa:text-ink">
      <span>{props.children}</span>
      {props.optional ? (
        <Badge tone="outline" className="wa:font-medium">
          {props.optional}
        </Badge>
      ) : null}
    </Tag>
  );
}
