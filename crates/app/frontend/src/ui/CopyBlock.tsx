import { Check, Copy as CopyIcon } from "lucide-react";

import { Button } from "./Button";
import { useCopy } from "./clipboard";
import { cn } from "./cn";

export type CopyLabels = { copy: string; copied: string; copyFailed: string };

/**
 * Text the user may need elsewhere (the test text, Linux commands, the shared or diagnostic
 * text), selectable, with a Copy button unless the footer already offers one.
 * - sample: the test notification text, large, in the app's own type;
 * - code: commands, monospaced;
 * - report: long text in a scrolling, focusable box.
 */
export function CopyBlock(props: {
  text: string;
  /** Accessible name of the block, e.g. what the text is. */
  label: string;
  labels?: CopyLabels;
  variant?: "sample" | "code" | "report";
  lang?: string;
  onCopied?: () => void;
}) {
  const [state, copy] = useCopy();
  const variant = props.variant ?? "code";
  return (
    <figure aria-label={props.label} className="overflow-hidden rounded-group bg-group shadow-group">
      <pre
        lang={props.lang}
        tabIndex={variant === "report" ? 0 : undefined}
        data-focus="inset"
        className={cn(
          "px-3.5 py-3 whitespace-pre-wrap break-words select-text",
          variant === "sample" && "text-[17px] leading-6 font-medium text-fg",
          variant === "code" && "font-mono text-label text-fg",
          variant === "report" && "max-h-56 overflow-y-auto font-mono text-label text-fg",
        )}
      >
        {props.text}
      </pre>
      {props.labels ? (
        <div className="flex min-h-11 items-center gap-2 border-t border-sep px-3.5 py-1.5">
          <span role="status" className={cn("min-w-0 flex-1 text-label", state === "failed" ? "text-danger-ink" : "text-fg-2")}>
            {state === "copied" ? props.labels.copied : state === "failed" ? props.labels.copyFailed : ""}
          </span>
          <Button
            size="sm"
            onClick={async () => {
              if (await copy(props.text)) props.onCopied?.();
            }}
          >
            {state === "copied" ? <Check size={14} aria-hidden="true" /> : <CopyIcon size={14} aria-hidden="true" />}
            {props.labels.copy}
          </Button>
        </div>
      ) : null}
    </figure>
  );
}
