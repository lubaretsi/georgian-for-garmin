import type { ReactNode } from "react";

import { AppIconShape } from "./AppIcon";
import { WORDS } from "./glyphs";
import { CheckBadge, type Face, useSvgId, WatchDevice, WordPath } from "./Watch";

/**
 * The hero pictures. All are decorative (aria-hidden): the heading and text say the same.
 * They share one device, one line weight and the tokens' palette; only the state changes.
 */

function Hero({ viewBox, children }: { viewBox: string; children: ReactNode }) {
  return (
    <svg viewBox={viewBox} className="hero mx-auto w-auto overflow-visible" aria-hidden="true" focusable="false">
      {children}
    </svg>
  );
}

/** The USB cable leaving from behind the case: plugged (it runs off), loose (a plug), pulled (a gap). */
function Cable({ state }: { state: "plugged" | "loose" | "pulled" }) {
  const id = useSvgId();
  const path = state === "plugged" ? "M168 150 C 196 214, 250 150, 330 168" : "M168 150 C 192 208, 236 168, 262 170";
  return (
    <g>
      <defs>
        <linearGradient id={`${id}-out`} x1="230" y1="0" x2="320" y2="0" gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="#fff" stopOpacity="1" />
          <stop offset="1" stopColor="#fff" stopOpacity="0" />
        </linearGradient>
        <mask id={`${id}-mask`} maskUnits="userSpaceOnUse" x="0" y="0" width="340" height="240">
          <rect width="340" height="240" fill={state === "plugged" ? `url(#${id}-out)` : "#fff"} />
        </mask>
      </defs>
      <g mask={`url(#${id}-mask)`}>
        <path d={path} fill="none" stroke="var(--strap-b)" strokeWidth="6.5" strokeLinecap="round" />
        <path d={path} fill="none" stroke="var(--strap-a)" strokeWidth="2" strokeLinecap="round" opacity="0.8" />
      </g>
      {state === "plugged" ? null : (
        <g transform={state === "pulled" ? "translate(280 166) rotate(-8)" : "translate(262 170) rotate(4)"}>
          <rect x="-2" y="-7.5" width="26" height="15" rx="4" fill="var(--case-lo)" stroke="var(--case-edge)" strokeWidth="0.75" />
          <rect x="1" y="-6" width="20" height="3" rx="1.5" fill="#fff" opacity="0.25" />
          <rect x="24" y="-4.5" width="12" height="9" rx="2.5" fill="var(--case-hi)" stroke="var(--case-edge)" strokeWidth="0.75" />
          <rect x="27" y="-1.25" width="7" height="2.5" rx="1.25" fill="var(--case-edge)" />
        </g>
      )}
      {state === "pulled" ? (
        <path d="M262 150 l14 -8 M262 150 l14 0 M262 150 l12 -14" stroke="var(--text-2)" strokeWidth="1.8" strokeLinecap="round" opacity="0.7" />
      ) : null}
    </g>
  );
}

/** connect / searching: the watch with its cable, and a slow radar ring while the app looks. */
export function ConnectScene() {
  return (
    <Hero viewBox="40 0 240 220">
      {[0, 1.4].map((delay) => (
        <circle
          key={delay}
          cx="160"
          cy="110"
          r="74"
          fill="none"
          stroke="var(--ring)"
          strokeWidth="1.5"
          className="anim-ripple"
          style={{ animationDelay: `${delay}s` }}
        />
      ))}
      <Cable state="loose" />
      <WatchDevice cx={160} cy={110} face="connect" />
    </Hero>
  );
}

/** The watch on its own, its face showing the state; `cable` when it's connected. */
export function WatchScene(props: { face: Face; cable?: "plugged" | "pulled"; power?: boolean; done?: boolean }) {
  return (
    <Hero viewBox="40 0 240 220">
      {props.cable ? <Cable state={props.cable} /> : null}
      <WatchDevice cx={160} cy={110} face={props.face} power={props.power} done={props.done} />
    </Hero>
  );
}

/** A notification bubble with the app icon and a Georgian greeting. */
function Bubble({ x, y }: { x: number; y: number }) {
  return (
    <g transform={`translate(${x} ${y})`} className="anim-float">
      <path
        d="M10 0 H86 Q96 0 96 10 V30 Q96 40 86 40 H28 L16 50 L18 40 H10 Q0 40 0 30 V10 Q0 0 10 0 Z"
        fill="var(--group)"
        stroke="var(--case-edge)"
        strokeOpacity="0.5"
        strokeWidth="0.75"
        style={{ filter: "drop-shadow(0 2px 4px var(--device-shadow))" }}
      />
      <g transform="translate(8 10)">
        <AppIconShape size={20} />
      </g>
      <WordPath word={WORDS.greeting} x={34} y={25} size={13} fill="var(--text)" />
    </g>
  );
}

/**
 * watch / install (§10, A's touch): before → after. The watch showing boxes becomes the watch
 * showing Georgian, and a Georgian notification bubbles out.
 */
export function InstallScene() {
  return (
    <Hero viewBox="6 0 316 220">
      <WatchDevice cx={66} cy={124} scale={0.56} face="boxes" dim />
      <path d="M108 92 Q132 66 158 82" fill="none" stroke="var(--indicator)" strokeWidth="2.4" strokeLinecap="round" strokeDasharray="0.1 6.5" />
      <path d="M150 74 L160 83 L148 87" fill="none" stroke="var(--indicator)" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
      <WatchDevice cx={214} cy={118} scale={0.86} face="georgian" />
      <Bubble x={222} y={12} />
    </Hero>
  );
}

function Phone({ x, y, rotate, children }: { x: number; y: number; rotate: number; children?: ReactNode }) {
  const id = useSvgId();
  return (
    <g transform={`translate(${x} ${y}) rotate(${rotate})`}>
      <defs>
        <linearGradient id={`${id}-phone`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="var(--case-hi)" />
          <stop offset="1" stopColor="var(--case-lo)" />
        </linearGradient>
      </defs>
      <rect x="2" y="6" width="68" height="136" rx="15" fill="var(--device-shadow)" opacity="0.5" style={{ filter: "blur(6px)" }} />
      <rect width="68" height="136" rx="15" fill={`url(#${id}-phone)`} stroke="var(--case-edge)" strokeWidth="0.75" />
      <rect x="3.5" y="3.5" width="61" height="129" rx="12" fill="var(--glass)" />
      <rect x="25" y="8" width="18" height="5" rx="2.5" fill="#000" opacity="0.8" />
      <rect x="7" y="22" width="54" height="31" rx="8" fill="var(--face-fg)" opacity="0.95" />
      <circle cx="15" cy="30" r="3.5" fill="var(--success)" />
      <rect x="22" y="28.25" width="24" height="3.5" rx="1.75" fill="var(--face-dim)" />
      <WordPath word={WORDS.georgian} x={12} y={47} size={12} fill="var(--glass)" />
      <rect x="7" y="58" width="54" height="18" rx="7" fill="var(--face-fg)" opacity="0.12" />
      <rect x="7" y="80" width="54" height="18" rx="7" fill="var(--face-fg)" opacity="0.07" />
      {children}
    </g>
  );
}

/**
 * outcome / works (§10, A's touch): restrained celebration. A soft green tint, the phone's
 * notification arriving on the watch as Georgian, a green check badge and a few confetti.
 */
export function CelebrateScene() {
  const arc = "M112 74 C 140 30, 178 30, 196 58";
  return (
    <Hero viewBox="24 6 290 204">
      <circle cx="232" cy="112" r="92" fill="var(--celebrate)" />
      <Phone x={46} y={44} rotate={-4} />
      <path d={arc} fill="none" stroke="var(--success)" strokeOpacity="0.5" strokeWidth="1.5" strokeDasharray="2 5" strokeLinecap="round" />
      <WatchDevice cx={232} cy={118} scale={0.86} face="georgian" />
      <CheckBadge x={268} y={80} r={13} />
      {/* confetti, few and calm */}
      <g className="anim-twinkle">
        <rect x="150" y="104" width="11" height="4.5" rx="2.25" fill="var(--indicator)" transform="rotate(-24 155 106)" />
      </g>
      <g className="anim-twinkle" style={{ animationDelay: "0.8s" }}>
        <circle cx="298" cy="40" r="3.5" fill="#f2b33d" />
      </g>
      <g className="anim-twinkle" style={{ animationDelay: "1.6s" }}>
        <rect x="290" y="150" width="11" height="4.5" rx="2.25" fill="var(--success)" transform="rotate(32 295 152)" />
      </g>
      <g className="anim-twinkle" style={{ animationDelay: "0.4s" }}>
        <circle cx="176" cy="30" r="3" fill="var(--success)" />
      </g>
      <g className="anim-twinkle" style={{ animationDelay: "1.2s" }}>
        <path d="M200 182 l3 -7 l3 7 l-3 7 Z" fill="#f2b33d" />
      </g>
    </Hero>
  );
}

/**
 * share: the report as a card (the app's icon, labelled lines), with what happened to it: a
 * green check once GitHub opened, a red mark when it didn't.
 */
export function ShareScene({ state }: { state: "ready" | "opened" | "failed" }) {
  const rows = [0, 1, 2, 3];
  return (
    <Hero viewBox="40 0 240 220">
      <g transform="rotate(-3 160 110)">
        <rect x="104" y="34" width="112" height="152" rx="14" fill="var(--device-shadow)" opacity="0.5" style={{ filter: "blur(7px)" }} />
        <rect x="102" y="28" width="116" height="156" rx="14" fill="var(--group)" stroke="var(--case-edge)" strokeOpacity="0.45" strokeWidth="0.75" />
        <g transform="translate(116 42)">
          <AppIconShape size={22} />
        </g>
        <rect x="146" y="47" width="52" height="5" rx="2.5" fill="var(--text)" opacity="0.7" />
        <rect x="146" y="57" width="34" height="4" rx="2" fill="var(--text-2)" opacity="0.5" />
        {rows.map((i) => (
          <g key={i}>
            <rect x="116" y={84 + i * 22} width="38" height="4" rx="2" fill="var(--text-2)" opacity="0.55" />
            <rect x={176 - (i % 2) * 10} y={84 + i * 22} width={28 + (i % 2) * 10} height="4" rx="2" fill="var(--text)" opacity="0.35" />
            {i < rows.length - 1 ? <rect x="116" y={95 + i * 22} width="88" height="0.75" fill="var(--case-edge)" opacity="0.35" /> : null}
          </g>
        ))}
      </g>
      {state === "opened" ? <CheckBadge x={214} y={170} r={15} /> : null}
      {state === "failed" ? (
        <g transform="translate(214 170)">
          <circle r="15" fill="var(--danger-ink)" stroke="var(--bg)" strokeWidth="3" />
          <path d="M-5 -5l10 10M5 -5l-10 10" stroke="var(--bg)" strokeWidth="2.6" strokeLinecap="round" />
        </g>
      ) : null}
    </Hero>
  );
}

/** outcome / no_notification: the phone's notification doesn't reach the watch. */
export function MissedScene() {
  return (
    <Hero viewBox="24 6 290 204">
      <Phone x={46} y={44} rotate={-4} />
      <path d="M112 74 C 140 30, 178 30, 196 58" fill="none" stroke="var(--ring)" strokeWidth="1.5" strokeDasharray="2 5" strokeLinecap="round" />
      <g transform="translate(154 40)">
        <circle r="10" fill="var(--group)" stroke="var(--case-edge)" strokeOpacity="0.4" strokeWidth="0.75" />
        <path d="M-3.5 -3.5l7 7M3.5 -3.5l-7 7" stroke="var(--danger-ink)" strokeWidth="2" strokeLinecap="round" />
      </g>
      <WatchDevice cx={232} cy={118} scale={0.86} face="time" />
    </Hero>
  );
}
