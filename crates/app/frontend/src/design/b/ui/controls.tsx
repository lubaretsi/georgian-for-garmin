import { cva } from "class-variance-authority";
import { Check } from "lucide-react";
import { Checkbox as CheckboxPrimitive, RadioGroup as RadioGroupPrimitive } from "radix-ui";
import { type ReactNode, useId } from "react";

import { cn } from "../lib/cn";

/* ---- Radio rows: a native list of choices, each a real radio, with a trailing sample ---- */

export type RadioRowChoice<T extends string> = { value: T; label: string; trailing?: ReactNode; lang?: string };

export function RadioRows<T extends string>(props: {
  labelledBy: string;
  choices: RadioRowChoice<T>[];
  value: T | undefined;
  onChange: (value: T) => void;
}) {
  return (
    <RadioGroupPrimitive.Root
      aria-labelledby={props.labelledBy}
      value={props.value ?? ""}
      onValueChange={(v) => props.onChange(v as T)}
      className={cn(
        "nb:overflow-hidden nb:rounded-group nb:bg-group nb:shadow-group",
        "nb:[&>*+*]:before:absolute nb:[&>*+*]:before:top-0 nb:[&>*+*]:before:right-0 nb:[&>*+*]:before:left-11",
        "nb:[&>*+*]:before:h-px nb:[&>*+*]:before:bg-sep nb:[&>*+*]:before:content-['']",
      )}
    >
      {props.choices.map((choice) => (
        <RadioGroupPrimitive.Item
          key={choice.value}
          value={choice.value}
          data-focus="inset"
          className={cn(
            "nb:group nb:relative nb:flex nb:min-h-13 nb:w-full nb:items-center nb:gap-3 nb:px-3.5 nb:py-2 nb:text-left",
            "nb:transition-colors nb:duration-150 nb:hover:bg-group-hover nb:active:bg-group-active",
            "nb:data-[state=checked]:bg-accent-soft",
          )}
        >
          <RadioDot />
          <span className="nb:flex-1 nb:text-row nb:text-fg nb:group-data-[state=checked]:font-medium">{choice.label}</span>
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
        "nb:flex nb:size-[18px] nb:shrink-0 nb:items-center nb:justify-center nb:rounded-full",
        "nb:border nb:border-control nb:bg-control-bg nb:shadow-control nb:transition-colors nb:duration-150",
        "nb:group-data-[state=checked]:border-indicator nb:group-data-[state=checked]:bg-indicator",
      )}
    >
      <span
        className={cn(
          "nb:size-[7px] nb:scale-0 nb:rounded-full nb:bg-on-accent nb:transition-transform nb:duration-200 nb:ease-native",
          "nb:group-data-[state=checked]:scale-100",
        )}
      />
    </span>
  );
}

/* ---- Checkbox rows ---- */

export function CheckboxRow(props: { label: string; checked: boolean; onChange: (checked: boolean) => void }) {
  const id = useId();
  return (
    <label
      htmlFor={id}
      className="nb:group nb:flex nb:min-h-11 nb:items-center nb:gap-3 nb:px-3.5 nb:py-2.5 nb:transition-colors nb:hover:bg-group-hover"
    >
      <Checkbox id={id} checked={props.checked} onCheckedChange={(c) => props.onChange(c === true)} />
      <span className="nb:flex-1 nb:text-row nb:text-fg nb:select-none">{props.label}</span>
    </label>
  );
}

export function Checkbox(props: CheckboxPrimitive.CheckboxProps) {
  return (
    <CheckboxPrimitive.Root
      {...props}
      className={cn(
        "nb:flex nb:size-[18px] nb:shrink-0 nb:items-center nb:justify-center nb:rounded-[5px]",
        "nb:border nb:border-control nb:bg-control-bg nb:shadow-control nb:transition-colors nb:duration-150",
        "nb:data-[state=checked]:border-indicator nb:data-[state=checked]:bg-indicator nb:data-[state=checked]:text-on-accent",
        props.className,
      )}
    >
      <CheckboxPrimitive.Indicator forceMount className="nb:transition-opacity nb:duration-150 nb:data-[state=unchecked]:opacity-0">
        <Check size={13} strokeWidth={3.2} />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  );
}

/* ---- Segmented control: a radio group drawn as native segments (tiles) ---- */

const segmentVariants = cva(
  [
    "nb:relative nb:z-0 nb:flex nb:h-full nb:flex-1 nb:items-center nb:justify-center nb:gap-1.5 nb:rounded-[7px] nb:px-3",
    "nb:text-row nb:text-fg nb:transition-[background-color,box-shadow,color] nb:duration-200 nb:ease-native",
    "nb:hover:bg-fill nb:data-[state=checked]:bg-control-bg nb:data-[state=checked]:shadow-thumb",
    "nb:data-[state=checked]:font-medium",
  ],
);

export function Segmented<T extends string>(props: {
  labelledBy: string;
  choices: { value: T; label: string; icon?: ReactNode }[];
  value: T | undefined;
  onChange: (value: T) => void;
  className?: string;
}) {
  return (
    <RadioGroupPrimitive.Root
      aria-labelledby={props.labelledBy}
      orientation="horizontal"
      value={props.value ?? ""}
      onValueChange={(v) => props.onChange(v as T)}
      className={cn(
        "nb:flex nb:h-9 nb:gap-0.5 nb:rounded-[9px] nb:bg-fill nb:p-0.5 nb:shadow-[inset_0_0_0_1px_var(--b-control-border)]",
        props.className,
      )}
    >
      {props.choices.map((choice) => (
        <RadioGroupPrimitive.Item key={choice.value} value={choice.value} className={segmentVariants()}>
          {choice.icon}
          {choice.label}
        </RadioGroupPrimitive.Item>
      ))}
    </RadioGroupPrimitive.Root>
  );
}

/* ---- Badge ---- */

const badgeVariants = cva(
  "nb:inline-flex nb:items-center nb:gap-1 nb:rounded-full nb:px-2 nb:py-px nb:text-label nb:font-medium nb:whitespace-nowrap",
  {
    variants: {
      tone: {
        neutral: "nb:bg-fill nb:text-fg",
        success: "nb:bg-success-soft nb:text-success-ink",
        caution: "nb:bg-caution-soft nb:text-caution-ink",
      },
    },
    defaultVariants: { tone: "neutral" },
  },
);

export function Badge(props: { tone?: "neutral" | "success" | "caution"; icon?: ReactNode; children: ReactNode }) {
  return (
    <span className={badgeVariants({ tone: props.tone })}>
      {props.icon}
      {props.children}
    </span>
  );
}

/* ---- Spinner: the native activity indicator (12 fading spokes) ---- */

export function Spinner({ size = 16, className }: { size?: number; className?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" className={cn("b-spinner nb:text-fg-2", className)}>
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
