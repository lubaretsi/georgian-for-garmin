import type { ReactNode } from "react";

import { Icon } from "./icons";

export type NoticeTone = "info" | "warning" | "error";

/**
 * A highlighted message. `live` makes it announce itself: errors as `role="alert"`, other
 * tones as a polite `role="status"`. Without `live` it is plain content.
 */
export function Notice(props: { tone: NoticeTone; title?: string; live?: boolean; children?: ReactNode }) {
  const role = props.live ? (props.tone === "error" ? "alert" : "status") : undefined;
  return (
    <div className={`notice notice-${props.tone}`} role={role}>
      <Icon name={props.tone} className="notice-icon" />
      <div className="notice-body">
        {props.title ? <h2 className="notice-title">{props.title}</h2> : null}
        {props.children}
      </div>
    </div>
  );
}
