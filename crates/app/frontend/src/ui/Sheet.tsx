import { type KeyboardEvent, type ReactNode, useEffect, useId, useRef } from "react";

import { Icon } from "./icons";

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select, textarea, summary, [tabindex]:not([tabindex="-1"])';

/**
 * A modal sheet over the window: focus moves into it, Tab stays inside, Esc and the close
 * button close it, and focus returns to what opened it. The rest of the window is made
 * inert by the caller (the shell), so screen readers and clicks stay in the sheet.
 */
export function Sheet(props: { title: string; closeLabel: string; onClose: () => void; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  const titleId = useId();

  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    ref.current?.querySelector<HTMLElement>("h2")?.focus();
    return () => opener?.focus();
  }, []);

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      props.onClose();
      return;
    }
    if (e.key !== "Tab" || !ref.current) return;
    const items = [...ref.current.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
      (el) => el.offsetParent !== null || el === document.activeElement,
    );
    const first = items[0];
    const last = items[items.length - 1];
    if (!first || !last) return;
    const active = document.activeElement;
    if (e.shiftKey && (active === first || !ref.current.contains(active))) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && (active === last || !ref.current.contains(active))) {
      e.preventDefault();
      first.focus();
    }
  };

  return (
    <div className="sheet-layer">
      <div className="sheet-scrim" onClick={props.onClose} aria-hidden="true" />
      <div
        ref={ref}
        className="sheet"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onKeyDown={onKeyDown}
      >
        <div className="sheet-header">
          <h2 id={titleId} className="sheet-title" tabIndex={-1}>
            {props.title}
          </h2>
          <button type="button" className="icon-button" aria-label={props.closeLabel} onClick={props.onClose}>
            <Icon name="close" />
          </button>
        </div>
        <div className="sheet-body">{props.children}</div>
      </div>
    </div>
  );
}
