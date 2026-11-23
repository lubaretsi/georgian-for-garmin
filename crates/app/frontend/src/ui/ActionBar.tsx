import { useCopy } from "./clipboard";
import type { CopyLabels } from "./CopyBlock";

/**
 * A footer button. It either calls `onPress` (usually emitting an ActionId) or, with
 * `copyText`, copies that text and says so on the button for a moment.
 */
export type FooterAction = {
  label: string;
  onPress?: () => void;
  copyText?: string;
  disabled?: boolean;
};

/**
 * The fixed footer: at most one secondary and one main action. When they don't fit side by
 * side they stack, the main action on top. It never scrolls away: the screen's content
 * scrolls above it.
 */
export function ActionBar(props: {
  primary?: FooterAction | undefined;
  secondary?: FooterAction | undefined;
  labels: CopyLabels;
}) {
  if (!props.primary && !props.secondary) return null;
  return (
    <div className="action-bar">
      {props.secondary ? <FooterButton action={props.secondary} kind="secondary" labels={props.labels} /> : null}
      {props.primary ? <FooterButton action={props.primary} kind="primary" labels={props.labels} /> : null}
    </div>
  );
}

function FooterButton(props: { action: FooterAction; kind: "primary" | "secondary"; labels: CopyLabels }) {
  const [state, copy] = useCopy();
  const { action } = props;
  const label =
    state === "copied" ? props.labels.copied : state === "failed" ? props.labels.copyFailed : action.label;
  return (
    <button
      type="button"
      className={`button button-${props.kind}`}
      data-main-action={props.kind === "primary" ? "" : undefined}
      disabled={action.disabled}
      onClick={() => {
        if (action.copyText !== undefined) void copy(action.copyText);
        action.onPress?.();
      }}
    >
      <span aria-live={action.copyText !== undefined ? "polite" : undefined}>{label}</span>
    </button>
  );
}
