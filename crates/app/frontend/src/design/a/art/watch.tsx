import type { ReactNode } from "react";

/*
 * The illustration vocabulary: flat shapes, one warm ink outline (2.5 units, round joins),
 * a single accent, colours from CSS variables (`--ill-*`, set per theme on `.design-a`).
 */

export const INK = "var(--ill-ink)";
export const STROKE = 2.5;

/** Shared stroke props for outlined shapes. */
export const outline = { stroke: INK, strokeWidth: STROKE, strokeLinejoin: "round", strokeLinecap: "round" } as const;

/**
 * A round Garmin-style watch: straps, a cream case, two buttons on the right, a dark face.
 * `face` is drawn on top of the face, centred at (cx, cy); its radius is `faceRadius(r)`.
 */
export function Watch(props: {
  cx: number;
  cy: number;
  r: number;
  face?: ReactNode;
  /** Draws the upper button in the accent (the one to hold for a restart). */
  pressButton?: boolean;
  muted?: boolean;
}) {
  const { cx, cy, r } = props;
  const strapW = r * 1.12;
  const strapH = r * 0.95;
  const holes = [0.55, 0.78].map((t) => cy + r + strapH * t - 2);
  const button = (angle: number, accent: boolean) => (
    <rect
      x={cx + r - 3}
      y={cy - r * 0.13}
      width={r * 0.2}
      height={r * 0.26}
      rx={2.5}
      fill={accent ? "var(--ill-accent)" : "var(--ill-case)"}
      transform={`rotate(${angle} ${cx} ${cy})`}
      {...outline}
    />
  );
  return (
    <g opacity={props.muted ? 0.9 : 1}>
      {/* straps */}
      <rect x={cx - strapW / 2} y={cy - r - strapH + 6} width={strapW} height={strapH} rx={r * 0.26} fill="var(--ill-strap)" {...outline} />
      <rect x={cx - strapW / 2} y={cy + r - 6} width={strapW} height={strapH} rx={r * 0.26} fill="var(--ill-strap)" {...outline} />
      {holes.map((y) => (
        <circle key={y} cx={cx} cy={y} r={1.8} fill="var(--ill-strap-2)" />
      ))}
      {/* buttons */}
      {button(-34, props.pressButton ?? false)}
      {button(34, false)}
      {/* case, bezel, face */}
      <circle cx={cx} cy={cy} r={r} fill="var(--ill-case)" {...outline} />
      <circle cx={cx} cy={cy} r={r * 0.84} fill="var(--ill-bezel)" />
      <circle cx={cx} cy={cy} r={faceRadius(r)} fill="var(--ill-face)" />
      {/* glass highlight */}
      <path
        d={arc(cx, cy, faceRadius(r) - 4, 200, 250)}
        fill="none"
        stroke="var(--ill-face-ink)"
        strokeOpacity={0.16}
        strokeWidth={2.5}
        strokeLinecap="round"
      />
      {props.face}
    </g>
  );
}

export function faceRadius(r: number): number {
  return r * 0.76;
}

/** An SVG arc path on a circle, angles in degrees (0 = 3 o'clock, clockwise). */
export function arc(cx: number, cy: number, r: number, from: number, to: number): string {
  const p = (a: number) => {
    const rad = (a * Math.PI) / 180;
    return `${(cx + r * Math.cos(rad)).toFixed(2)} ${(cy + r * Math.sin(rad)).toFixed(2)}`;
  };
  const large = Math.abs(to - from) > 180 ? 1 : 0;
  return `M ${p(from)} A ${r} ${r} 0 ${large} 1 ${p(to)}`;
}

/** Text on the watch face (real Georgian, in the app's own font). */
export function FaceText(props: { x: number; y: number; size: number; children: string; dim?: boolean; weight?: number }) {
  return (
    <text
      x={props.x}
      y={props.y}
      fontSize={props.size}
      fontWeight={props.weight ?? 600}
      textAnchor="middle"
      dominantBaseline="central"
      fill={props.dim ? "var(--ill-face-dim)" : "var(--ill-face-ink)"}
    >
      {props.children}
    </text>
  );
}

/** Four "missing glyph" boxes, as a watch draws letters its font doesn't have. */
export function Boxes(props: { cx: number; cy: number; size: number; color?: string }) {
  const gap = props.size * 0.32;
  const total = props.size * 4 + gap * 3;
  return (
    <g fill="none" stroke={props.color ?? "var(--ill-face-dim)"} strokeWidth={1.4}>
      {[0, 1, 2, 3].map((i) => (
        <rect
          key={i}
          x={props.cx - total / 2 + i * (props.size + gap)}
          y={props.cy - props.size * 0.62}
          width={props.size}
          height={props.size * 1.24}
          rx={1}
        />
      ))}
    </g>
  );
}

/** A four-point sparkle. */
export function Sparkle(props: { x: number; y: number; size: number; fill: string; className?: string; delay?: 1 | 2 }) {
  const { x, y, size: s } = props;
  const k = s * 0.22;
  return (
    <path
      className={props.className}
      data-delay={props.delay}
      d={`M ${x} ${y - s} Q ${x + k} ${y - k} ${x + s} ${y} Q ${x + k} ${y + k} ${x} ${y + s} Q ${x - k} ${y + k} ${x - s} ${y} Q ${x - k} ${y - k} ${x} ${y - s} Z`}
      fill={props.fill}
    />
  );
}

/** The soft ground shadow under an object. */
export function Ground(props: { cx: number; cy: number; rx: number }) {
  return <ellipse cx={props.cx} cy={props.cy} rx={props.rx} ry={4.5} fill="var(--ill-shadow)" />;
}
