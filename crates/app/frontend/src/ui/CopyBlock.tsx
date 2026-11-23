import { useCopy } from "./clipboard";
import { Icon } from "./icons";

export type CopyLabels = { copy: string; copied: string; copyFailed: string };

/**
 * Text the user may need elsewhere (the test text, Linux commands, the shared text), with a
 * Copy button. The text itself stays selectable.
 */
export function CopyBlock(props: {
  text: string;
  /** Accessible name of the block, e.g. what the text is. */
  label: string;
  labels: CopyLabels;
  variant?: "sample" | "code" | "report";
  lang?: string;
  onCopied?: () => void;
}) {
  const [state, copy] = useCopy();
  const variant = props.variant ?? "code";
  return (
    <figure className={`copy-block copy-block-${variant}`} aria-label={props.label}>
      <pre className="copy-block-text" lang={props.lang} tabIndex={variant === "report" ? 0 : undefined}>
        {props.text}
      </pre>
      <div className="copy-block-bar">
        <span className="copy-block-status" role="status">
          {state === "copied" ? props.labels.copied : state === "failed" ? props.labels.copyFailed : ""}
        </span>
        <button
          type="button"
          className="button button-secondary button-small"
          onClick={async () => {
            if (await copy(props.text)) props.onCopied?.();
          }}
        >
          <Icon name={state === "copied" ? "check" : "copy"} size={18} />
          <span>{props.labels.copy}</span>
        </button>
      </div>
    </figure>
  );
}
