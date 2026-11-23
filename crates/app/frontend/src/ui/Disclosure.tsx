import type { ReactNode } from "react";

import { Icon } from "./icons";

/** A collapsed section (native details/summary: keyboard and screen readers come built in). */
export function Disclosure(props: { summary: string; defaultOpen?: boolean; children: ReactNode }) {
  return (
    <details className="disclosure" open={props.defaultOpen}>
      <summary>
        <Icon name="chevron" className="disclosure-chevron" size={18} />
        <span>{props.summary}</span>
      </summary>
      <div className="disclosure-body">{props.children}</div>
    </details>
  );
}
