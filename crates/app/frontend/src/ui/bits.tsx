// Small presentational pieces shared by the screens.

import type { ReactNode } from "react";

import { Icon, type IconName } from "./icons";

/** The connected watch: its model and software. */
export function WatchCard(props: { model: string; software: string }) {
  return (
    <div className="watch-card">
      <span className="watch-card-icon">
        <Icon name="watch" size={26} />
      </span>
      <span className="watch-card-text">
        <span className="watch-card-model">{props.model}</span>
        <span className="watch-card-software">{props.software}</span>
      </span>
    </div>
  );
}

/** A large icon above a screen's heading. */
export function Illustration(props: { icon: IconName; tone?: "accent" | "success" | "muted" }) {
  return (
    <div className={`illustration illustration-${props.tone ?? "accent"}`}>
      <Icon name={props.icon} size={34} />
    </div>
  );
}

/** "Waiting…" with a gentle pulse (still under reduced motion). */
export function Waiting(props: { children: ReactNode }) {
  return (
    <p className="waiting">
      <span className="waiting-dot" aria-hidden="true" />
      <span>{props.children}</span>
    </p>
  );
}

/** Progress, determinate when known. */
export function ProgressBar(props: { label: string; value: { done: number; total: number } | null; text?: string }) {
  const { value } = props;
  const pct = value && value.total > 0 ? Math.min(100, Math.round((value.done / value.total) * 100)) : null;
  return (
    <div className="progress">
      <div
        className={`progress-track${pct === null ? " progress-indeterminate" : ""}`}
        role="progressbar"
        aria-label={props.label}
        aria-valuemin={pct === null ? undefined : 0}
        aria-valuemax={pct === null ? undefined : 100}
        aria-valuenow={pct ?? undefined}
        aria-valuetext={props.text}
      >
        <div className="progress-fill" style={pct === null ? undefined : { width: `${pct}%` }} />
      </div>
      {props.text ? <p className="progress-text">{props.text}</p> : null}
    </div>
  );
}

/** A QR code drawn by the core (an SVG string), on a light background in both themes. */
export function Qr(props: { svg: string; label: string }) {
  return (
    <div className="qr" role="img" aria-label={props.label} dangerouslySetInnerHTML={{ __html: props.svg }} />
  );
}

/** A labelled list of values (what's shared, what the app sees). */
export function Facts(props: { items: { label: string; value: string }[] }) {
  return (
    <dl className="facts">
      {props.items.map((item) => (
        <div key={item.label} className="fact">
          <dt>{item.label}</dt>
          <dd>{item.value}</dd>
        </div>
      ))}
    </dl>
  );
}

/** A small label above the heading ("Attempt 2"). */
export function Badge(props: { children: ReactNode }) {
  return <p className="badge">{props.children}</p>;
}
