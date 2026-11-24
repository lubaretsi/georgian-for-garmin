import { Progress as ProgressPrimitive } from "radix-ui";

/** A native-style progress bar; indeterminate (a sliding segment) when the total isn't known. */
export function Progress(props: { label: string; value: { done: number; total: number } | null; text?: string }) {
  const pct = props.value && props.value.total > 0 ? Math.round((props.value.done / props.value.total) * 100) : null;
  return (
    <div className="flex flex-col gap-2">
      <ProgressPrimitive.Root
        aria-label={props.label}
        value={pct}
        className="relative h-1.5 overflow-hidden rounded-full bg-fill-strong"
      >
        <ProgressPrimitive.Indicator
          className={
            pct === null
              ? "anim-pulse absolute inset-y-0 left-0 w-full rounded-full bg-accent/60"
              : "h-full rounded-full bg-accent transition-[width] duration-300 ease-native"
          }
          style={pct === null ? undefined : { width: `${pct}%` }}
        />
      </ProgressPrimitive.Root>
      {props.text ? <p className="text-label text-fg-2 tabular-nums">{props.text}</p> : null}
    </div>
  );
}
