import { Check } from "lucide-react";
import { Checkbox as CheckboxPrimitive, RadioGroup as RadioGroupPrimitive } from "radix-ui";
import type { ComponentProps, ReactNode } from "react";
import { useId } from "react";

import { cn } from "./cn";
import { Legend } from "./parts";

/* ---- RadioGroup (tiles) ------------------------------------------------------------- */

export function RadioGroup({ className, ...props }: ComponentProps<typeof RadioGroupPrimitive.Root>) {
  return <RadioGroupPrimitive.Root data-slot="radio-group" className={cn("wa:grid wa:gap-2.5", className)} {...props} />;
}

/**
 * A large selectable tile: a real radio (role="radio", arrow keys move within the group)
 * drawn as a card. `media` sits on top or to the side; the tick shows the checked state.
 */
export function RadioTile({
  className,
  media,
  label,
  layout = "stack",
  ...props
}: ComponentProps<typeof RadioGroupPrimitive.Item> & { media?: ReactNode; label: ReactNode; layout?: "stack" | "row" }) {
  return (
    <RadioGroupPrimitive.Item
      data-slot="radio-tile"
      className={cn(
        "wa:group wa:relative wa:flex wa:rounded-2xl wa:bg-surface wa:text-left wa:shadow-card",
        "wa:border-[1.5px] wa:border-line wa:transition-[border-color,background-color,box-shadow] wa:duration-150",
        "wa:hover:border-control",
        "wa:data-[state=checked]:border-accent wa:data-[state=checked]:bg-accent-soft wa:data-[state=checked]:shadow-none",
        layout === "stack"
          ? "wa:flex-col wa:items-center wa:gap-2 wa:px-2 wa:pt-3 wa:pb-3"
          : "wa:flex-row wa:items-center wa:gap-3.5 wa:py-1.5 wa:pr-4 wa:pl-3",
        className,
      )}
      {...props}
    >
      {media}
      <span
        className={cn(
          "wa:text-[14px] wa:leading-snug wa:font-semibold wa:text-ink",
          layout === "stack" ? "wa:text-center wa:px-1" : "wa:flex-1",
        )}
      >
        {label}
      </span>
      <span
        aria-hidden
        className={cn(
          "wa:grid wa:size-[22px] wa:shrink-0 wa:place-items-center wa:rounded-full wa:border-[1.5px] wa:border-control wa:bg-surface",
          "wa:transition-colors wa:group-data-[state=checked]:border-accent wa:group-data-[state=checked]:bg-accent",
          layout === "stack" ? "wa:absolute wa:top-2.5 wa:right-2.5" : "",
        )}
      >
        <RadioGroupPrimitive.Indicator className="wa:text-on-accent">
          <Check strokeWidth={3.2} className="wa:size-3.5" />
        </RadioGroupPrimitive.Indicator>
      </span>
    </RadioGroupPrimitive.Item>
  );
}

/** A two-or-three option radio group drawn as a segmented control. */
export function Segmented<T extends string>(props: {
  legend: string;
  optional?: string;
  options: { value: T; label: string; icon?: ReactNode }[];
  value: T | undefined;
  onChange: (value: T) => void;
}) {
  const id = useId();
  return (
    <div className="wa:grid">
      <Legend id={id} optional={props.optional}>
        {props.legend}
      </Legend>
      <RadioGroupPrimitive.Root
        aria-labelledby={id}
        value={props.value ?? ""}
        onValueChange={(v) => props.onChange(v as T)}
        orientation="horizontal"
        className="wa:grid wa:auto-cols-fr wa:grid-flow-col wa:gap-1 wa:rounded-[14px] wa:bg-sunken wa:p-1 wa:border wa:border-control"
      >
        {props.options.map((o) => (
          <RadioGroupPrimitive.Item
            key={o.value}
            value={o.value}
            className={cn(
              "wa:inline-flex wa:min-h-10 wa:items-center wa:justify-center wa:gap-2 wa:rounded-[10px] wa:px-3 wa:text-[15px] wa:font-semibold wa:text-ink-2",
              "wa:transition-[background-color,color,box-shadow] wa:hover:text-ink",
              "wa:data-[state=checked]:bg-surface wa:data-[state=checked]:text-ink wa:data-[state=checked]:shadow-raised",
              "wa:[&_svg]:size-4",
            )}
          >
            {o.icon}
            {o.label}
          </RadioGroupPrimitive.Item>
        ))}
      </RadioGroupPrimitive.Root>
    </div>
  );
}

/* ---- Checkbox ----------------------------------------------------------------------- */

export function Checkbox({ className, ...props }: ComponentProps<typeof CheckboxPrimitive.Root>) {
  return (
    <CheckboxPrimitive.Root
      data-slot="checkbox"
      className={cn(
        "wa:grid wa:size-[22px] wa:shrink-0 wa:place-items-center wa:rounded-[7px] wa:border-[1.5px] wa:border-control wa:bg-surface",
        "wa:transition-colors wa:data-[state=checked]:border-accent wa:data-[state=checked]:bg-accent wa:data-[state=checked]:text-on-accent",
        "wa:disabled:opacity-50",
        className,
      )}
      {...props}
    >
      <CheckboxPrimitive.Indicator>
        <Check strokeWidth={3.2} className="wa:size-3.5" />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  );
}

/** A grouped list of checkbox rows in one card (iOS-settings style), with its legend. */
export function CheckboxList<T extends string>(props: {
  legend: string;
  optional?: string;
  options: { value: T; label: string }[];
  value: T[];
  onChange: (value: T[]) => void;
}) {
  const id = useId();
  return (
    <fieldset className="wa:grid wa:gap-2">
      <Legend as="legend" optional={props.optional}>
        {props.legend}
      </Legend>
      <div className="wa:overflow-hidden wa:rounded-2xl wa:bg-surface wa:shadow-card wa:border wa:border-line">
        {props.options.map((o, i) => {
          const checked = props.value.includes(o.value);
          return (
            <label
              key={o.value}
              htmlFor={`${id}-${o.value}`}
              className={cn(
                "wa:flex wa:min-h-12 wa:cursor-pointer wa:items-center wa:gap-3 wa:px-4 wa:py-2.5 wa:text-[15px] wa:leading-snug wa:transition-colors wa:hover:bg-sunken/60",
                i > 0 && "wa:border-t wa:border-line",
              )}
            >
              <Checkbox
                id={`${id}-${o.value}`}
                checked={checked}
                onCheckedChange={(next) =>
                  props.onChange(next === true ? [...props.value, o.value] : props.value.filter((v) => v !== o.value))
                }
              />
              <span>{o.label}</span>
            </label>
          );
        })}
      </div>
    </fieldset>
  );
}
