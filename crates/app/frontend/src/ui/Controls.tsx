import { cva } from "class-variance-authority";
import { Check } from "lucide-react";
import { Checkbox as CheckboxPrimitive, RadioGroup as RadioGroupPrimitive } from "radix-ui";
import { type ReactNode, useId } from "react";

import { cn } from "./cn";
import { Hairline } from "./Group";

/* ---- Radio rows: a native list of choices, each a real radio, with a trailing sample ---- */

export type RadioRowChoice<T extends string> = { value: T; label: string; trailing?: ReactNode; lang?: string };

export function RadioRows<T extends string>(props: {
  labelledBy: string;
  describedBy?: string;
  choices: RadioRowChoice<T>[];
  value: T | undefined;
  onChange: (value: T) => void;
}) {
  return (
    <RadioGroupPrimitive.Root
      aria-labelledby={props.labelledBy}
      aria-describedby={props.describedBy}
      value={props.value ?? ""}
      onValueChange={(v) => props.onChange(v as T)}
      className={cn(
        "overflow-hidden rounded-group bg-group shadow-group",
        "[--inset:44px] [&>:first-child>.hairline]:hidden",
      )}
    >
      {props.choices.map((choice) => (
        <RadioGroupPrimitive.Item
          key={choice.value}
          value={choice.value}
          data-focus="inset"
          className={cn(
            "group relative flex min-h-13 w-full items-center gap-3 px-3.5 py-1.5 text-left",
            "transition-colors duration-150 hover:bg-group-hover active:bg-group-active",
            "data-[state=checked]:bg-accent-soft",
          )}
        >
          <Hairline />
          <RadioDot />
          <span lang={choice.lang} className="flex-1 text-row text-fg group-data-[state=checked]:font-medium">
            {choice.label}
          </span>
          {choice.trailing}
        </RadioGroupPrimitive.Item>
      ))}
    </RadioGroupPrimitive.Root>
  );
}

function RadioDot() {
  return (
    <span
      aria-hidden="true"
      className={cn(
        "flex size-[18px] shrink-0 items-center justify-center rounded-full",
        "border border-control bg-control-bg shadow-control transition-colors duration-150",
        "group-data-[state=checked]:border-indicator group-data-[state=checked]:bg-indicator",
      )}
    >
      <span
        className={cn(
          "size-[7px] scale-0 rounded-full bg-on-accent transition-transform duration-200 ease-native",
          "group-data-[state=checked]:scale-100",
        )}
      />
    </span>
  );
}

/* ---- Checkboxes ---- */

export function CheckboxRow(props: { label: string; checked: boolean; onChange: (checked: boolean) => void }) {
  const id = useId();
  return (
    <label htmlFor={id} className="relative flex min-h-11 items-center gap-3 px-3.5 py-2.5 transition-colors hover:bg-group-hover">
      <Hairline />
      <Checkbox id={id} checked={props.checked} onCheckedChange={(c) => props.onChange(c === true)} />
      <span className="flex-1 text-row text-fg select-none">{props.label}</span>
    </label>
  );
}

export function Checkbox(props: CheckboxPrimitive.CheckboxProps) {
  return (
    <CheckboxPrimitive.Root
      {...props}
      className={cn(
        "flex size-[18px] shrink-0 items-center justify-center rounded-[5px]",
        "border border-control bg-control-bg shadow-control transition-colors duration-150",
        "data-[state=checked]:border-indicator data-[state=checked]:bg-indicator data-[state=checked]:text-on-accent",
        props.className,
      )}
    >
      <CheckboxPrimitive.Indicator forceMount className="transition-opacity duration-150 data-[state=unchecked]:opacity-0">
        <Check size={13} strokeWidth={3.2} />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  );
}

/* ---- Segmented control: a radio group drawn as native segments ---- */

const segment = cn(
  "relative flex h-full flex-1 items-center justify-center gap-1.5 rounded-[7px] px-3",
  "text-row text-fg transition-[background-color,box-shadow] duration-200 ease-native",
  "hover:bg-fill-strong data-[state=checked]:bg-control-bg data-[state=checked]:font-medium data-[state=checked]:shadow-thumb",
);

export function Segmented<T extends string>(props: {
  labelledBy: string;
  choices: { value: T; label: string; lang?: string }[];
  value: T | undefined;
  onChange: (value: T) => void;
}) {
  return (
    <RadioGroupPrimitive.Root
      aria-labelledby={props.labelledBy}
      orientation="horizontal"
      value={props.value ?? ""}
      onValueChange={(v) => props.onChange(v as T)}
      className="flex h-9 gap-0.5 rounded-[9px] bg-fill p-0.5 shadow-[inset_0_0_0_1px_var(--control-border)]"
    >
      {props.choices.map((choice) => (
        <RadioGroupPrimitive.Item key={choice.value} value={choice.value} lang={choice.lang} className={segment}>
          {choice.label}
        </RadioGroupPrimitive.Item>
      ))}
    </RadioGroupPrimitive.Root>
  );
}

/* ---- Badge ---- */

const badgeVariants = cva(
  "inline-flex items-center gap-1 rounded-full px-2 py-px text-label font-medium whitespace-nowrap",
  {
    variants: {
      tone: {
        neutral: "bg-fill text-fg",
        success: "bg-success-soft text-success-ink",
        caution: "bg-caution-soft text-caution-ink",
        danger: "bg-danger-soft text-danger-ink",
      },
    },
    defaultVariants: { tone: "neutral" },
  },
);

export type BadgeTone = "neutral" | "success" | "caution" | "danger";

export function Badge(props: { tone?: BadgeTone; lang?: string; children: ReactNode }) {
  return (
    <span lang={props.lang} className={badgeVariants({ tone: props.tone })}>
      {props.children}
    </span>
  );
}

/* ---- Spinner: the native activity indicator (12 fading spokes) ---- */

export function Spinner({ size = 16, className }: { size?: number; className?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" className={cn("anim-spinner text-fg-2", className)}>
      {Array.from({ length: 12 }, (_, i) => (
        <rect
          key={i}
          x="11"
          y="2"
          width="2"
          height="6"
          rx="1"
          fill="currentColor"
          opacity={0.15 + (0.85 * (i + 1)) / 12}
          transform={`rotate(${i * 30} 12 12)`}
        />
      ))}
    </svg>
  );
}
