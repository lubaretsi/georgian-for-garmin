import { useReducedMotion } from "motion/react";
import { useId } from "react";

/**
 * One device, drawn once and reused on every screen: a round watch with a silver case, a
 * dark bezel and glass, a crown-like upper button and a lower button. Every colour comes from
 * the direction's CSS variables, so it follows light and dark. The face shows the state:
 * the connection glyph, Georgian letters, boxes, or the restart spinner.
 */

export type Face = "connect" | "georgian" | "boxes" | "restart" | "blank";

/** A unique, url()-safe id prefix for this SVG's gradients and masks. */
export function useSvgId(): string {
  return `b${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
}

const GEORGIAN = "ქართ";

/** The watch, centred on (cx, cy) in its parent SVG; `scale` 1 is a 58-unit case radius. */
export function WatchDevice(props: {
  cx: number;
  cy: number;
  scale?: number;
  face: Face;
  highlight?: "power";
  straps?: boolean;
}) {
  const id = useSvgId();
  const s = props.scale ?? 1;
  return (
    <g transform={`translate(${props.cx} ${props.cy}) scale(${s})`}>
      <defs>
        <linearGradient id={`${id}-case`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="var(--b-case-hi)" />
          <stop offset="0.55" stopColor="var(--b-case-lo)" />
          <stop offset="1" stopColor="var(--b-case-hi)" stopOpacity="0.85" />
        </linearGradient>
        <linearGradient id={`${id}-strap`} x1="0" y1="0" x2="1" y2="0">
          <stop offset="0" stopColor="var(--b-strap-b)" />
          <stop offset="0.5" stopColor="var(--b-strap-a)" />
          <stop offset="1" stopColor="var(--b-strap-b)" />
        </linearGradient>
        <linearGradient id={`${id}-fade-top`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#fff" stopOpacity="0" />
          <stop offset="0.75" stopColor="#fff" stopOpacity="1" />
        </linearGradient>
        <linearGradient id={`${id}-fade-bottom`} x1="0" y1="1" x2="0" y2="0">
          <stop offset="0" stopColor="#fff" stopOpacity="0" />
          <stop offset="0.75" stopColor="#fff" stopOpacity="1" />
        </linearGradient>
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

      {/* contact shadow */}
      <ellipse cx="2" cy="9" rx="58" ry="58" fill="var(--b-device-shadow)" opacity="0.55" style={{ filter: "blur(7px)" }} />

      {props.straps !== false
        ? (["top", "bottom"] as const).map((end) => (
            <path
              key={end}
              d={end === "top" ? "M-27 -50 L-25 -104 Q0 -108 25 -104 L27 -50 Z" : "M-27 50 L-25 104 Q0 108 25 104 L27 50 Z"}
              fill={`url(#${id}-strap)`}
              stroke="var(--b-case-edge)"
              strokeOpacity="0.35"
              strokeWidth="0.6"
              mask={`url(#${id}-mask-${end})`}
            />
          ))
        : null}

      {/* buttons, behind the case: upper (crown-like) and lower on the right */}
      <g transform="rotate(-38)">
        <rect x="54" y="-7" width="10" height="14" rx="3" fill="var(--b-button)" />
        <path d="M58 -5v10M61 -5v10" stroke="var(--b-case-edge)" strokeWidth="0.8" />
        {props.highlight === "power" ? <PowerHighlight /> : null}
      </g>
      <g transform="rotate(38)">
        <rect x="54" y="-6" width="8" height="12" rx="3" fill="var(--b-button)" />
      </g>

      {/* case, bezel, glass */}
      <circle r="58" fill={`url(#${id}-case)`} stroke="var(--b-case-edge)" strokeWidth="0.75" />
      <circle r="56.5" fill="none" stroke="#fff" strokeOpacity="0.35" strokeWidth="1" strokeDasharray="90 400" transform="rotate(200)" />
      <circle r="51" fill="var(--b-bezel)" />
      <circle r="45.5" fill="var(--b-glass)" />
      <g clipPath={`url(#${id}-glass)`}>
        <FaceContent face={props.face} />
        <circle r="45" fill={`url(#${id}-gloss)`} />
      </g>
      <circle r="45.5" fill="none" stroke="#fff" strokeOpacity="0.06" />
    </g>
  );
}

/** The power button, marked: the accent button with a soft halo and a "hold" ring around it. */
function PowerHighlight() {
  return (
    <>
      <circle cx="60" cy="0" r="17" fill="none" stroke="var(--b-indicator)" strokeWidth="1.5" opacity="0.45" className="b-pulse" />
      <circle cx="60" cy="0" r="11.5" fill="var(--b-indicator)" opacity="0.16" />
      <rect x="54" y="-8" width="11" height="16" rx="3.5" fill="var(--b-indicator)" />
      <path d="M58.5 -5.5v11M61.5 -5.5v11" stroke="#fff" strokeOpacity="0.55" strokeWidth="0.9" />
    </>
  );
}

const faceText = { fontFamily: "var(--b-font)", fontWeight: 600 } as const;

/** A notification as the watch draws it: an app dot and a sender line, the text, a second line. */
function Notification({ children }: { children: React.ReactNode }) {
  return (
    <g>
      <circle cx="-14" cy="-24" r="3.5" fill="var(--b-success)" />
      <rect x="-8" y="-26" width="24" height="4" rx="2" fill="var(--b-face-dim)" />
      {children}
      <rect x="-18" y="18" width="36" height="3.5" rx="1.75" fill="var(--b-face-dim)" opacity="0.7" />
      <rect x="-12" y="26" width="24" height="3.5" rx="1.75" fill="var(--b-face-dim)" opacity="0.45" />
    </g>
  );
}

export function Tofu({ x, y, w = 8, h = 12, stroke = 1.6 }: { x: number; y: number; w?: number; h?: number; stroke?: number }) {
  return <rect x={x} y={y} width={w} height={h} rx="1" fill="none" stroke="var(--b-face-fg)" strokeWidth={stroke} />;
}

function FaceContent({ face }: { face: Face }) {
  const reduced = useReducedMotion();
  switch (face) {
    case "georgian":
      return (
        <Notification>
          <text y="7" textAnchor="middle" fontSize="18" fill="var(--b-face-fg)" style={faceText}>
            {GEORGIAN}
          </text>
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
    case "restart":
      return (
        <g>
          <circle r="27" fill="none" stroke="var(--b-face-dim)" strokeOpacity="0.35" strokeWidth="3" />
          <circle
            r="27"
            fill="none"
            stroke="var(--b-face-fg)"
            strokeWidth="3"
            strokeLinecap="round"
            strokeDasharray="42 170"
            className="b-spin"
          />
          <path
            d="M0 -11v9M-7.5 -6.5a10.5 10.5 0 1 0 15 0"
            fill="none"
            stroke="var(--b-face-fg)"
            strokeWidth="2.4"
            strokeLinecap="round"
          />
        </g>
      );
    case "connect":
      return (
        <g fill="none" stroke="var(--b-face-fg)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <rect x="-14" y="-17" width="28" height="19" rx="2.5" />
          <path d="M-19 7h38" />
          {[-8, 0, 8].map((x, i) => (
            <circle
              key={x}
              cx={x}
              cy="20"
              r="1.8"
              fill="var(--b-face-fg)"
              stroke="none"
              className={reduced ? undefined : "b-pulse"}
              style={{ animationDelay: `${i * 0.25}s` }}
            />
          ))}
        </g>
      );
    case "blank":
      return null;
  }
}

/** A small rendered sample of the watch face for the answer rows. */
export function MiniFace({ sample }: { sample: "georgian" | "boxes" | "question_marks" | "some_letters" | "none" }) {
  const id = useSvgId();
  return (
    <svg width="40" height="40" viewBox="-22 -22 44 44" aria-hidden="true" className="nb:shrink-0">
      <defs>
        <linearGradient id={`${id}-ring`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="var(--b-case-hi)" />
          <stop offset="1" stopColor="var(--b-case-lo)" />
        </linearGradient>
      </defs>
      <circle r="21.5" fill={`url(#${id}-ring)`} stroke="var(--b-case-edge)" strokeWidth="0.6" />
      <circle r="19" fill="var(--b-bezel)" />
      <circle r="16.5" fill="var(--b-glass)" />
      <MiniContent sample={sample} />
    </svg>
  );
}

function MiniContent({ sample }: { sample: "georgian" | "boxes" | "question_marks" | "some_letters" | "none" }) {
  const text = { textAnchor: "middle" as const, fontSize: 10.5, fill: "var(--b-face-fg)", style: faceText, y: 3.8 };
  switch (sample) {
    case "georgian":
      return <text {...text}>{GEORGIAN}</text>;
    case "question_marks":
      return <text {...text}>????</text>;
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
          <text {...text} x={-5.5}>
            ქა
          </text>
          {[0, 1].map((i) => (
            <Tofu key={i} x={1.5 + i * 6} y={-4} w={4.5} h={7.5} stroke={1.1} />
          ))}
        </>
      );
    case "none":
      return <path d="M-6 0h12" stroke="var(--b-face-dim)" strokeWidth="1.8" strokeLinecap="round" />;
  }
}
