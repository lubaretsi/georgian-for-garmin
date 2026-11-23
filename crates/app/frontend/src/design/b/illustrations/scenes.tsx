import { useReducedMotion } from "motion/react";

import { useSvgId, WatchDevice } from "./watch";

/**
 * The hero scenes. Each is decorative (aria-hidden): the heading and text say the same.
 * They share one device, one line weight and the same palette; only the state changes.
 */

function Hero({ viewBox, children }: { viewBox: string; children: React.ReactNode }) {
  return (
    <svg viewBox={viewBox} className="b-hero nb:mx-auto nb:w-auto nb:overflow-visible" aria-hidden="true" focusable="false">
      {children}
    </svg>
  );
}

/** The USB cable, leaving from behind the case and ending in a plug or running off the edge. */
function Cable({ plugged }: { plugged: boolean }) {
  const id = useSvgId();
  const path = plugged ? "M168 150 C 196 214, 250 150, 330 168" : "M168 150 C 192 208, 236 168, 262 170";
  return (
    <g>
      <defs>
        <linearGradient id={`${id}-out`} x1="230" y1="0" x2="320" y2="0" gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="#fff" stopOpacity="1" />
          <stop offset="1" stopColor="#fff" stopOpacity="0" />
        </linearGradient>
        <mask id={`${id}-mask`} maskUnits="userSpaceOnUse" x="0" y="0" width="340" height="240">
          <rect width="340" height="240" fill={plugged ? `url(#${id}-out)` : "#fff"} />
        </mask>
      </defs>
      <g mask={`url(#${id}-mask)`}>
        <path d={path} fill="none" stroke="var(--b-strap-b)" strokeWidth="6.5" strokeLinecap="round" />
        <path d={path} fill="none" stroke="var(--b-strap-a)" strokeWidth="2" strokeLinecap="round" opacity="0.8" />
      </g>
      {plugged ? null : (
        <g transform="translate(262 170) rotate(4)">
          <rect x="-2" y="-7.5" width="26" height="15" rx="4" fill="var(--b-case-lo)" stroke="var(--b-case-edge)" strokeWidth="0.75" />
          <rect x="1" y="-6" width="20" height="3" rx="1.5" fill="#fff" opacity="0.25" />
          <rect x="24" y="-4.5" width="12" height="9" rx="2.5" fill="var(--b-case-hi)" stroke="var(--b-case-edge)" strokeWidth="0.75" />
          <rect x="27" y="-1.25" width="7" height="2.5" rx="1.25" fill="var(--b-case-edge)" />
        </g>
      )}
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
          stroke="var(--b-ring)"
          strokeWidth="1.5"
          className="b-ripple"
          style={{ animationDelay: `${delay}s` }}
        />
      ))}
      <Cable plugged={false} />
      <WatchDevice cx={160} cy={110} face="connect" />
    </Hero>
  );
}

/** watch / install: connected, and still showing boxes. */
export function InstallScene() {
  return (
    <Hero viewBox="40 0 240 220">
      <Cable plugged />
      <WatchDevice cx={160} cy={110} face="boxes" />
    </Hero>
  );
}

/** restart: unplugged, the restart spinner on the face, the power button marked. */
export function RestartScene() {
  return (
    <Hero viewBox="40 0 240 220">
      <WatchDevice cx={160} cy={110} face="restart" highlight="power" />
    </Hero>
  );
}

/** outcome / works: the notification travels from the phone and arrives as Georgian letters. */
export function SuccessScene() {
  const id = useSvgId();
  const reduced = useReducedMotion();
  const arc = "M112 74 C 140 30, 178 30, 196 58";
  return (
    <Hero viewBox="34 14 262 192">
      <defs>
        <linearGradient id={`${id}-phone`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="var(--b-case-hi)" />
          <stop offset="1" stopColor="var(--b-case-lo)" />
        </linearGradient>
      </defs>

      {/* phone */}
      <g transform="translate(46 44) rotate(-4)">
        <rect x="2" y="6" width="68" height="136" rx="15" fill="var(--b-device-shadow)" opacity="0.5" style={{ filter: "blur(6px)" }} />
        <rect width="68" height="136" rx="15" fill={`url(#${id}-phone)`} stroke="var(--b-case-edge)" strokeWidth="0.75" />
        <rect x="3.5" y="3.5" width="61" height="129" rx="12" fill="var(--b-glass)" />
        <rect x="25" y="8" width="18" height="5" rx="2.5" fill="#000" opacity="0.8" />
        {/* the notification banner */}
        <rect x="7" y="22" width="54" height="31" rx="8" fill="var(--b-face-fg)" opacity="0.95" />
        <circle cx="15" cy="30" r="3.5" fill="var(--b-success)" />
        <rect x="22" y="28.25" width="24" height="3.5" rx="1.75" fill="var(--b-face-dim)" />
        <text x="12" y="47" fontSize="12" fill="var(--b-glass)" style={{ fontFamily: "var(--b-font)", fontWeight: 600 }}>
          ქართ
        </text>
        <rect x="7" y="58" width="54" height="18" rx="7" fill="var(--b-face-fg)" opacity="0.12" />
        <rect x="7" y="80" width="54" height="18" rx="7" fill="var(--b-face-fg)" opacity="0.07" />
      </g>

      {/* the path the notification takes */}
      <path d={arc} fill="none" stroke="var(--b-ring)" strokeWidth="1.5" strokeDasharray="2 5" strokeLinecap="round" />
      {reduced ? null : (
        <circle r="4" fill="var(--b-success)">
          <animateMotion dur="2.6s" repeatCount="indefinite" path={arc} keyPoints="0;1;1" keyTimes="0;0.6;1" calcMode="linear" />
          <animate attributeName="opacity" values="0;1;1;0;0" keyTimes="0;0.1;0.55;0.62;1" dur="2.6s" repeatCount="indefinite" />
        </circle>
      )}

      <WatchDevice cx={232} cy={118} scale={0.86} face="georgian" />

      {/* arrived: the success mark */}
      <g transform="translate(272 76)">
        <circle r="13" fill="var(--b-success)" stroke="var(--b-bg)" strokeWidth="3" />
        <path d="M-5.5 0.5l3.8 3.8 7.2-7.6" fill="none" stroke="#fffbfb" strokeWidth="2.6" strokeLinecap="round" strokeLinejoin="round" />
      </g>
    </Hero>
  );
}
