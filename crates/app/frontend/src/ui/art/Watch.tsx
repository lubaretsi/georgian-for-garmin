import { type ReactNode, useId } from "react";

import { type Word, WORDS } from "./glyphs";

/**
 * One device, drawn once and reused everywhere (§10): a round watch with a silver case, a dark
 * bezel and glass, a crown-like upper button and a lower button. Every colour comes from the
 * tokens, so it follows light and dark. Its face shows the state.
 */

export type Face =
  | "connect" // looking for the watch: a computer and pulsing dots
  | "georgian" // a notification in Georgian letters
  | "boxes" // a notification in boxes
  | "question_marks" // a notification in question marks
  | "some_letters" // a notification half in boxes
  | "time" // the idle watch face
  | "restart" // the power glyph in a turning ring
  | "verifying" // a turning ring
  | "busy" // a padlock: another program holds the watch
  | "drive" // a USB drive
  | "alert" // something needs attention
  | { progress: number | null }; // writing: a ring filling up (null: unknown)

/** A unique, url()-safe id prefix for one SVG's gradients and masks. */
export function useSvgId(): string {
  return `s${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
}

/** The watch, centred on (cx, cy) in its parent SVG; `scale` 1 is a 58-unit case radius. */
export function WatchDevice(props: {
  cx: number;
  cy: number;
  scale?: number;
  face: Face;
  /** Marks the power button (restart). */
  power?: boolean;
  /** A green check badge on the case (it works). */
  done?: boolean;
  straps?: boolean;
  /** Drawn quieter: the "before" watch. */
  dim?: boolean;
}) {
  const id = useSvgId();
  const s = props.scale ?? 1;
  return (
    <g transform={`translate(${props.cx} ${props.cy}) scale(${s})`} opacity={props.dim ? 0.78 : undefined}>
      <defs>
        <linearGradient id={`${id}-case`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="var(--case-hi)" />
          <stop offset="0.55" stopColor="var(--case-lo)" />
          <stop offset="1" stopColor="var(--case-hi)" stopOpacity="0.85" />
        </linearGradient>
        <linearGradient id={`${id}-strap`} x1="0" y1="0" x2="1" y2="0">
          <stop offset="0" stopColor="var(--strap-b)" />
          <stop offset="0.5" stopColor="var(--strap-a)" />
          <stop offset="1" stopColor="var(--strap-b)" />
        </linearGradient>
        {(["top", "bottom"] as const).map((end) => (
          <linearGradient key={end} id={`${id}-fade-${end}`} x1="0" y1={end === "top" ? 0 : 1} x2="0" y2={end === "top" ? 1 : 0}>
            <stop offset="0" stopColor="#fff" stopOpacity="0" />
            <stop offset="0.75" stopColor="#fff" stopOpacity="1" />
          </linearGradient>
        ))}
        <mask id={`${id}-mask-top`} maskUnits="userSpaceOnUse" x="-40" y="-110" width="80" height="80">
          <rect x="-40" y="-110" width="80" height="80" fill={`url(#${id}-fade-top)`} />
        </mask>
        <mask id={`${id}-mask-bottom`} maskUnits="userSpaceOnUse" x="-40" y="30" width="80" height="80">
          <rect x="-40" y="30" width="80" height="80" fill={`url(#${id}-fade-bottom)`} />
        </mask>
        <radialGradient id={`${id}-gloss`} cx="0.3" cy="0.2" r="0.8">
          <stop offset="0" stopColor="#fff" stopOpacity="0.13" />
          <stop offset="0.5" stopColor="#fff" stopOpacity="0.03" />
          <stop offset="1" stopColor="#fff" stopOpacity="0" />
        </radialGradient>
        <clipPath id={`${id}-glass`}>
          <circle r="45" />
        </clipPath>
      </defs>

      <ellipse cx="2" cy="9" rx="58" ry="58" fill="var(--device-shadow)" opacity="0.55" style={{ filter: "blur(7px)" }} />

      {props.straps !== false
        ? (["top", "bottom"] as const).map((end) => (
            <path
              key={end}
              d={end === "top" ? "M-27 -50 L-25 -104 Q0 -108 25 -104 L27 -50 Z" : "M-27 50 L-25 104 Q0 108 25 104 L27 50 Z"}
              fill={`url(#${id}-strap)`}
              stroke="var(--case-edge)"
              strokeOpacity="0.35"
              strokeWidth="0.6"
              mask={`url(#${id}-mask-${end})`}
            />
          ))
        : null}

      {/* buttons, behind the case: upper (crown-like) and lower, on the right */}
      <g transform="rotate(-38)">
        <rect x="54" y="-7" width="10" height="14" rx="3" fill="var(--device-button)" />
        <path d="M58 -5v10M61 -5v10" stroke="var(--case-edge)" strokeWidth="0.8" />
        {props.power ? <PowerHighlight /> : null}
      </g>
      <g transform="rotate(38)">
        <rect x="54" y="-6" width="8" height="12" rx="3" fill="var(--device-button)" />
      </g>

      <circle r="58" fill={`url(#${id}-case)`} stroke="var(--case-edge)" strokeWidth="0.75" />
      <circle r="56.5" fill="none" stroke="#fff" strokeOpacity="0.35" strokeWidth="1" strokeDasharray="90 400" transform="rotate(200)" />
      <circle r="51" fill="var(--bezel)" />
      <circle r="45.5" fill="var(--glass)" />
      <g clipPath={`url(#${id}-glass)`}>
        <FaceContent face={props.face} />
        <circle r="45" fill={`url(#${id}-gloss)`} />
      </g>
      <circle r="45.5" fill="none" stroke="#fff" strokeOpacity="0.06" />

      {props.done ? <CheckBadge x={42} y={-42} r={15} /> : null}
    </g>
  );
}

/** The green check that says "it works", with a ring of the page colour around it. */
export function CheckBadge({ x, y, r }: { x: number; y: number; r: number }) {
  const k = r / 13;
  return (
    <g transform={`translate(${x} ${y})`}>
      <circle r={r} fill="var(--success)" stroke="var(--bg)" strokeWidth={3} />
      <path
        d={`M${-5.5 * k} ${0.5 * k}l${3.8 * k} ${3.8 * k} ${7.2 * k} ${-7.6 * k}`}
        fill="none"
        stroke="#fffbfb"
        strokeWidth={2.6 * k}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </g>
  );
}

/** The power button, marked: the accent button with a soft halo and a "hold" ring. */
function PowerHighlight() {
  return (
    <>
      <circle cx="60" cy="0" r="17" fill="none" stroke="var(--indicator)" strokeWidth="1.5" opacity="0.45" className="anim-pulse" />
      <circle cx="60" cy="0" r="11.5" fill="var(--indicator)" opacity="0.16" />
      <rect x="54" y="-8" width="11" height="16" rx="3.5" fill="var(--indicator)" />
      <path d="M58.5 -5.5v11M61.5 -5.5v11" stroke="#fff" strokeOpacity="0.55" strokeWidth="0.9" />
    </>
  );
}

/** A word drawn from its glyph outlines (glyphs.ts): `size` is the font size, `y` the baseline. */
export function WordPath(props: { word: Word; x?: number; y: number; size: number; anchor?: "start" | "middle"; fill: string }) {
  const s = props.size / 1000;
  const left = (props.x ?? 0) - (props.anchor === "middle" ? (props.word.width * s) / 2 : 0);
  return <path d={props.word.d} transform={`translate(${left} ${props.y}) scale(${s})`} fill={props.fill} />;
}

const FACE = "var(--face-fg)";

/** A notification as the watch draws it: an app dot and a sender line, the text, two lines. */
function Notification({ children }: { children: ReactNode }) {
  return (
    <g>
      <circle cx="-14" cy="-24" r="3.5" fill="var(--success)" />
      <rect x="-8" y="-26" width="24" height="4" rx="2" fill="var(--face-dim)" />
      {children}
      <rect x="-18" y="18" width="36" height="3.5" rx="1.75" fill="var(--face-dim)" opacity="0.7" />
      <rect x="-12" y="26" width="24" height="3.5" rx="1.75" fill="var(--face-dim)" opacity="0.45" />
    </g>
  );
}

/** The "missing glyph" box the watch draws for letters its font lacks. */
export function Tofu({ x, y, w = 8, h = 12, stroke = 1.6 }: { x: number; y: number; w?: number; h?: number; stroke?: number }) {
  return <rect x={x} y={y} width={w} height={h} rx="1" fill="none" stroke="var(--face-fg)" strokeWidth={stroke} />;
}

const glyph = {
  fill: "none",
  stroke: "var(--face-fg)",
  strokeWidth: 2.2,
  strokeLinecap: "round",
  strokeLinejoin: "round",
} as const;

function Ring({ dash, spin }: { dash?: string; spin?: boolean }) {
  return (
    <>
      <circle r="27" fill="none" stroke="var(--face-dim)" strokeOpacity="0.35" strokeWidth="3" />
      <circle
        r="27"
        fill="none"
        stroke="var(--face-fg)"
        strokeWidth="3"
        strokeLinecap="round"
        strokeDasharray={dash ?? "42 170"}
        transform="rotate(-90)"
        className={spin ? "anim-spin" : undefined}
      />
    </>
  );
}

function FaceContent({ face }: { face: Face }) {
  if (typeof face === "object") {
    const circumference = 2 * Math.PI * 27;
    if (face.progress === null) return <Ring spin />;
    const filled = Math.max(0.04, Math.min(1, face.progress)) * circumference;
    return (
      <g>
        <Ring dash={`${filled} ${circumference}`} />
        <path d="M0 -9v14M-6 0l6 6 6-6" {...glyph} />
      </g>
    );
  }
  switch (face) {
    case "georgian":
      return (
        <Notification>
          <WordPath word={WORDS.georgian} y={7} size={18} anchor="middle" fill={FACE} />
        </Notification>
      );
    case "boxes":
      return (
        <Notification>
          {[0, 1, 2, 3].map((i) => (
            <Tofu key={i} x={-21 + i * 11} y={-8} />
          ))}
        </Notification>
      );
    case "question_marks":
      return (
        <Notification>
          <WordPath word={WORDS.questionMarks} y={7} size={18} anchor="middle" fill={FACE} />
        </Notification>
      );
    case "some_letters":
      return (
        <Notification>
          <WordPath word={WORDS.someLetters} x={-10} y={7} size={18} anchor="middle" fill={FACE} />
          <Tofu x={3} y={-8} />
          <Tofu x={14} y={-8} />
        </Notification>
      );
    case "time":
      return (
        <g>
          <WordPath word={WORDS.time} y={6} size={22} anchor="middle" fill={FACE} />
          <rect x="-12" y="16" width="24" height="3.5" rx="1.75" fill="var(--face-dim)" opacity="0.6" />
        </g>
      );
    case "restart":
      return (
        <g>
          <Ring spin />
          <path d="M0 -11v9M-7.5 -6.5a10.5 10.5 0 1 0 15 0" {...glyph} strokeWidth={2.4} />
        </g>
      );
    case "verifying":
      return (
        <g>
          <Ring spin />
          {[-8, 0, 8].map((x, i) => (
            <circle key={x} cx={x} cy="0" r="2" fill="var(--face-fg)" className="anim-pulse" style={{ animationDelay: `${i * 0.25}s` }} />
          ))}
        </g>
      );
    case "connect":
      return (
        <g {...glyph} strokeWidth={2}>
          <rect x="-14" y="-17" width="28" height="19" rx="2.5" />
          <path d="M-19 7h38" />
          {[-8, 0, 8].map((x, i) => (
            <circle
              key={x}
              cx={x}
              cy="20"
              r="1.8"
              fill="var(--face-fg)"
              stroke="none"
              className="anim-pulse"
              style={{ animationDelay: `${i * 0.25}s` }}
            />
          ))}
        </g>
      );
    case "busy":
      return (
        <g {...glyph}>
          <rect x="-12" y="-4" width="24" height="19" rx="3.5" />
          <path d="M-7 -4v-6a7 7 0 0 1 14 0v6" />
          <circle cx="0" cy="5.5" r="1.6" fill="var(--face-fg)" stroke="none" />
        </g>
      );
    case "drive":
      return (
        <g {...glyph}>
          {/* a USB stick, connector up */}
          <rect x="-9" y="-5" width="18" height="24" rx="3.5" />
          <path d="M-6 -5v-11h12v11" />
          <path d="M-2.8 -12.5h1.6M1.2 -12.5h1.6" strokeWidth={2} />
          <circle cx="0" cy="9" r="2.2" fill="var(--face-fg)" stroke="none" opacity="0.7" />
        </g>
      );
    case "alert":
      return (
        <g {...glyph} stroke="var(--caution-face, #f0c36b)">
          <path d="M0 -17L17 13H-17Z" />
          <path d="M0 -6v8" />
          <circle cx="0" cy="7.5" r="1.4" fill="var(--caution-face, #f0c36b)" stroke="none" />
        </g>
      );
  }
}

export type Sample = "georgian" | "boxes" | "question_marks" | "some_letters" | "none";

/** A small rendered watch face for the answer rows and the result list. */
export function MiniFace({ sample, size = 40 }: { sample: Sample; size?: number }) {
  const id = useSvgId();
  return (
    <svg width={size} height={size} viewBox="-22 -22 44 44" aria-hidden="true" className="shrink-0">
      <defs>
        <linearGradient id={`${id}-ring`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="var(--case-hi)" />
          <stop offset="1" stopColor="var(--case-lo)" />
        </linearGradient>
      </defs>
      <circle r="21.5" fill={`url(#${id}-ring)`} stroke="var(--case-edge)" strokeWidth="0.6" />
      <circle r="19" fill="var(--bezel)" />
      <circle r="16.5" fill="var(--glass)" />
      <MiniContent sample={sample} />
    </svg>
  );
}

function MiniContent({ sample }: { sample: Sample }) {
  const word = (w: Word, x = 0) => <WordPath word={w} x={x} y={3.8} size={10.5} anchor="middle" fill={FACE} />;
  switch (sample) {
    case "georgian":
      return word(WORDS.georgian);
    case "question_marks":
      return word(WORDS.questionMarks);
    case "boxes":
      return (
        <>
          {[0, 1, 2, 3].map((i) => (
            <Tofu key={i} x={-11.5 + i * 6} y={-4} w={4.5} h={7.5} stroke={1.1} />
          ))}
        </>
      );
    case "some_letters":
      return (
        <>
          {word(WORDS.someLetters, -5.5)}
          {[0, 1].map((i) => (
            <Tofu key={i} x={1.5 + i * 6} y={-4} w={4.5} h={7.5} stroke={1.1} />
          ))}
        </>
      );
    case "none":
      return <path d="M-6 0h12" stroke="var(--face-dim)" strokeWidth="1.8" strokeLinecap="round" />;
  }
}
