import { RESULT_SAMPLES } from "@/copy/shared";

import { AppIcon } from "./app-icon";
import { arc, Boxes, FaceText, Ground, INK, outline, Sparkle, STROKE, Watch } from "./watch";

/*
 * One hero scene per step. Every scene shares a 300×170 canvas, the same ink outline,
 * the same watch and the same soft ground, so the screens read as one illustrated story.
 * They are decorative (`aria-hidden`): the heading and text carry the meaning.
 */

function Canvas(props: { children: React.ReactNode }) {
  return (
    <svg viewBox="0 0 300 170" className="wa:h-full wa:w-auto wa:max-w-full wa:overflow-visible" aria-hidden focusable="false">
      {props.children}
    </svg>
  );
}

/** connect: the watch on its cable, the plug heading for the computer, a gentle search pulse. */
export function ConnectScene() {
  const w = { cx: 104, cy: 84, r: 38 };
  const cable = "M 128 116 C 150 162 196 150 204 112 C 210 86 222 84 236 84";
  return (
    <Canvas>
      <circle cx={w.cx} cy={w.cy} r={76} fill="var(--ill-blob)" opacity={0.55} />
      {[0, 1, 2].map((i) => (
        <circle
          key={i}
          className="a-pulse"
          data-delay={i === 0 ? undefined : i}
          cx={w.cx}
          cy={w.cy}
          r={54 + i * 11}
          fill="none"
          stroke="var(--ill-accent)"
          strokeOpacity={0.45 - i * 0.12}
          strokeWidth={2}
        />
      ))}
      <Ground cx={w.cx} cy={160} rx={46} />
      {/* the cable: an ink outline with a light core */}
      <path d={cable} fill="none" stroke={INK} strokeWidth={8.5} strokeLinecap="round" />
      <path d={cable} fill="none" stroke="var(--ill-paper)" strokeWidth={3.5} strokeLinecap="round" />
      <Watch
        {...w}
        face={
          <g>
            {[0, 1, 2].map((i) => (
              <circle
                key={i}
                className="a-blink"
                data-delay={i === 0 ? undefined : i}
                cx={w.cx - 10 + i * 10}
                cy={w.cy}
                r={3.2}
                fill="var(--ill-face-ink)"
              />
            ))}
          </g>
        }
      />
      {/* USB plug */}
      <rect x={232} y={74} width={26} height={20} rx={6} fill="var(--ill-case)" {...outline} />
      <rect x={258} y={78.5} width={11} height={11} rx={2} fill="var(--ill-face-dim)" {...outline} />
      {/* the computer's side, with a port */}
      <path d="M 300 34 L 288 34 Q 282 34 282 40 L 282 128 Q 282 134 288 134 L 300 134" fill="var(--ill-paper)" {...outline} />
      <rect x={281} y={77} width={8} height={14} rx={2} fill="var(--ill-ink)" />
      {/* motion ticks */}
      <g stroke="var(--ill-accent)" strokeWidth={2.5} strokeLinecap="round">
        <path d="M 273 64 L 277 58" />
        <path d="M 273 104 L 277 110" />
      </g>
      <Sparkle x={42} y={34} size={6} fill="var(--ill-sun)" className="a-twinkle" />
      <Sparkle x={172} y={26} size={4.5} fill="var(--ill-accent)" className="a-twinkle" delay={1} />
    </Canvas>
  );
}

/** install: before (boxes on a muted watch) → after (Georgian on the watch, a notification). */
export function InstallScene() {
  const before = { cx: 52, cy: 98, r: 24 };
  const after = { cx: 168, cy: 92, r: 40 };
  return (
    <Canvas>
      <circle cx={after.cx} cy={after.cy} r={78} fill="var(--ill-blob)" opacity={0.55} />
      <Ground cx={before.cx} cy={158} rx={26} />
      <Ground cx={after.cx} cy={164} rx={44} />
      <g opacity={0.72}>
        <Watch {...before} face={<Boxes cx={before.cx} cy={before.cy} size={4.6} />} />
      </g>
      {/* the change: a dotted arc with an arrowhead */}
      <path
        d="M 76 60 Q 100 30 124 52"
        fill="none"
        stroke="var(--ill-accent)"
        strokeWidth={2.5}
        strokeLinecap="round"
        strokeDasharray="0.1 7"
      />
      <path d="M 116 52 L 125 53 L 123 44" fill="none" stroke="var(--ill-accent)" strokeWidth={2.5} strokeLinecap="round" strokeLinejoin="round" />
      <Watch
        {...after}
        face={
          <g>
            <circle cx={after.cx} cy={after.cy - 15} r={3} fill="var(--ill-accent)" />
            <FaceText x={after.cx} y={after.cy + 4} size={17}>
              {RESULT_SAMPLES.georgian}
            </FaceText>
          </g>
        }
      />
      {/* a Georgian notification bubbling out of the watch */}
      <g className="a-float">
        <path
          d="M 204 22 H 288 Q 300 22 300 34 V 52 Q 300 64 288 64 H 222 L 208 74 L 210 63 Q 192 60 192 46 V 34 Q 192 22 204 22 Z"
          fill="var(--ill-paper)"
          {...outline}
        />
        <AppIcon x={199} y={31} size={24} />
        <text x={230} y={44} fontSize={13} fontWeight={600} dominantBaseline="central" fill="var(--ill-ink)">
          სალამი!
        </text>
      </g>
      <Sparkle x={28} y={44} size={5} fill="var(--ill-sun)" className="a-twinkle" />
      <Sparkle x={226} y={128} size={6} fill="var(--ill-accent)" className="a-twinkle" delay={2} />
    </Canvas>
  );
}

/** restart: a circular arrow around the watch, the button to hold lit up. */
export function RestartScene() {
  const w = { cx: 150, cy: 86, r: 40 };
  const ring = 66;
  const end = 312;
  const rad = (end * Math.PI) / 180;
  const tip = { x: w.cx + ring * Math.cos(rad), y: w.cy + ring * Math.sin(rad) };
  return (
    <Canvas>
      <circle cx={w.cx} cy={w.cy} r={78} fill="var(--ill-blob)" opacity={0.55} />
      <Ground cx={w.cx} cy={162} rx={44} />
      <g className="a-spin-slow" style={{ transformOrigin: `${w.cx}px ${w.cy}px` }}>
        <path d={arc(w.cx, w.cy, ring, 30, end)} fill="none" stroke="var(--ill-accent)" strokeWidth={5} strokeLinecap="round" />
        <path
          d={`M ${tip.x - 12} ${tip.y - 3} L ${tip.x + 1} ${tip.y + 1} L ${tip.x - 5} ${tip.y + 12}`}
          fill="none"
          stroke="var(--ill-accent)"
          strokeWidth={5}
          strokeLinecap="round"
          strokeLinejoin="round"
          transform={`rotate(${end + 100} ${tip.x} ${tip.y})`}
        />
      </g>
      <Watch
        {...w}
        pressButton
        face={
          <g fill="none" stroke="var(--ill-face-ink)" strokeWidth={3} strokeLinecap="round">
            <path d={arc(w.cx, w.cy, 11, -50, 230)} />
            <path d={`M ${w.cx} ${w.cy - 16} V ${w.cy - 3}`} />
          </g>
        }
      />
      {/* "hold" ticks by the upper button */}
      <g className="a-press" stroke="var(--ill-accent)" strokeWidth={2.5} strokeLinecap="round">
        <path d="M 196 46 L 202 40" />
        <path d="M 201 56 L 209 54" />
        <path d="M 188 40 L 190 32" />
      </g>
      <Sparkle x={70} y={40} size={5} fill="var(--ill-sun)" className="a-twinkle" />
      <Sparkle x={238} y={126} size={5.5} fill="var(--ill-accent)" className="a-twinkle" delay={1} />
    </Canvas>
  );
}

/** check: the phone sends a notification across to the watch, which shows Georgian. */
export function CheckScene() {
  const w = { cx: 214, cy: 92, r: 38 };
  const path = "M 112 40 Q 152 6 186 42";
  return (
    <Canvas>
      <circle cx={150} cy={92} r={80} fill="var(--ill-blob)" opacity={0.55} />
      <Ground cx={82} cy={162} rx={34} />
      <Ground cx={w.cx} cy={162} rx={40} />
      {/* phone */}
      <rect x={52} y={30} width={62} height={124} rx={14} fill="var(--ill-bezel)" {...outline} />
      <rect x={57} y={35} width={52} height={114} rx={10} fill="var(--ill-paper)" />
      <rect x={74} y={39} width={18} height={4} rx={2} fill="var(--ill-bezel)" />
      <rect x={61} y={52} width={44} height={30} rx={7} fill="var(--ill-case)" stroke="var(--ill-line)" strokeWidth={1.5} />
      <AppIcon x={65} y={57} size={12} />
      <rect x={81} y={59} width={20} height={4} rx={2} fill="var(--ill-ink)" opacity={0.75} />
      <rect x={65} y={73} width={34} height={4} rx={2} fill="var(--ill-ink)" opacity={0.3} />
      <rect x={61} y={90} width={44} height={10} rx={5} fill="var(--ill-line)" />
      <rect x={61} y={106} width={30} height={10} rx={5} fill="var(--ill-line)" />
      {/* the path across, with a travelling dot */}
      <path d={path} fill="none" stroke="var(--ill-accent)" strokeWidth={2.5} strokeLinecap="round" strokeDasharray="0.1 7" />
      <circle className="a-travel" r={5} fill="var(--ill-accent)" style={{ offsetPath: `path("${path}")`, offsetDistance: "60%" }} />
      <Watch
        {...w}
        face={
          <g>
            <circle cx={w.cx} cy={w.cy - 14} r={3} fill="var(--ill-accent)" />
            <FaceText x={w.cx} y={w.cy + 4} size={16}>
              {RESULT_SAMPLES.georgian}
            </FaceText>
          </g>
        }
      />
      <Sparkle x={150} y={128} size={5} fill="var(--ill-sun)" className="a-twinkle" />
      <Sparkle x={272} y={42} size={5} fill="var(--ill-accent)" className="a-twinkle" delay={1} />
    </Canvas>
  );
}

/** outcome / works: Georgian on the watch, a check, restrained confetti. */
export function SuccessScene() {
  const w = { cx: 150, cy: 86, r: 42 };
  return (
    <Canvas>
      <circle cx={w.cx} cy={w.cy} r={76} fill="var(--ill-success-soft)" />
      <circle cx={w.cx} cy={w.cy} r={76} fill="none" stroke="var(--ill-success)" strokeOpacity={0.35} strokeWidth={2} strokeDasharray="0.1 8" strokeLinecap="round" />
      <Ground cx={w.cx} cy={164} rx={46} />
      <Watch
        {...w}
        face={
          <FaceText x={w.cx} y={w.cy + 1} size={18}>
            {RESULT_SAMPLES.georgian}
          </FaceText>
        }
      />
      {/* the check badge */}
      <circle cx={w.cx + 34} cy={w.cy + 32} r={15} fill="var(--ill-success)" stroke="var(--ill-ink)" strokeWidth={STROKE} />
      <path
        d={`M ${w.cx + 27} ${w.cy + 32} L ${w.cx + 32} ${w.cy + 37} L ${w.cx + 41} ${w.cy + 27}`}
        fill="none"
        stroke="#fff"
        strokeWidth={3.2}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      {/* confetti, kept few and calm */}
      <g className="a-twinkle">
        <rect x={56} y={44} width={12} height={5} rx={2.5} fill="var(--ill-accent)" transform="rotate(-24 62 46)" />
      </g>
      <g className="a-twinkle" data-delay="1">
        <circle cx={236} cy={46} r={4} fill="var(--ill-sun)" />
      </g>
      <g className="a-twinkle" data-delay="2">
        <rect x={232} y={124} width={12} height={5} rx={2.5} fill="var(--ill-success)" transform="rotate(30 238 126)" />
      </g>
      <g className="a-twinkle" data-delay="1">
        <circle cx={66} cy={130} r={3.5} fill="var(--ill-accent)" />
      </g>
      <Sparkle x={92} y={22} size={6} fill="var(--ill-sun)" className="a-twinkle" delay={2} />
      <Sparkle x={214} y={18} size={5} fill="var(--ill-accent)" className="a-twinkle" />
      <Sparkle x={258} y={88} size={4.5} fill="var(--ill-sun)" className="a-twinkle" delay={1} />
      <Sparkle x={42} y={90} size={4} fill="var(--ill-success)" className="a-twinkle" delay={2} />
    </Canvas>
  );
}
