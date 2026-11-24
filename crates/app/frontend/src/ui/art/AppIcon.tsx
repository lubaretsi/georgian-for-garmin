/** The app icon's shape (the red tile with a white "ქ"), for use inside another SVG. */
export function AppIconShape({ size = 16 }: { size?: number }) {
  return (
    <g transform={`scale(${size / 16})`}>
      <rect width="16" height="16" rx="3.6" fill="#e0344f" />
      <text x="8" y="12" textAnchor="middle" fontSize="11" fontWeight="700" fill="#fffbfb" style={{ fontFamily: "var(--font)" }}>
        ქ
      </text>
    </g>
  );
}

/** The app icon, small. */
export function AppIcon({ size = 16 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" aria-hidden="true">
      <AppIconShape />
    </svg>
  );
}
