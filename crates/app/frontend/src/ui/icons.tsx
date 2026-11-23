// Inline line icons, drawn in the current text color. Decorative: hidden from screen readers.

const paths = {
  watch: (
    <>
      <rect x="6" y="6" width="12" height="12" rx="3.5" />
      <path d="M9 6 9.6 2.5h4.8L15 6M9 18l.6 3.5h4.8L15 18M12 9.5V12l1.8 1.2" />
    </>
  ),
  cable: (
    <>
      <path d="M9.5 2.5v3.5M14.5 2.5v3.5" />
      <path d="M7 6h10v4a5 5 0 0 1-10 0z" />
      <path d="M12 15v3.5a3 3 0 0 1-3 3H7" />
    </>
  ),
  restart: (
    <>
      <path d="M19.5 12a7.5 7.5 0 1 1-2.2-5.3" />
      <path d="M19.5 3.5v4.5H15" />
    </>
  ),
  check: <path d="M5 12.5 9.5 17 19 7.5" />,
  info: (
    <>
      <circle cx="12" cy="12" r="9" />
      <path d="M12 11v5.5M12 7.5v.5" />
    </>
  ),
  warning: (
    <>
      <path d="M12 3.5 21.5 20h-19z" />
      <path d="M12 10v4.5M12 17v.5" />
    </>
  ),
  error: (
    <>
      <circle cx="12" cy="12" r="9" />
      <path d="m9 9 6 6M15 9l-6 6" />
    </>
  ),
  chevron: <path d="m9 6 6 6-6 6" />,
  close: <path d="m6 6 12 12M18 6 6 18" />,
  more: (
    <>
      <circle cx="5.5" cy="12" r="1.2" />
      <circle cx="12" cy="12" r="1.2" />
      <circle cx="18.5" cy="12" r="1.2" />
    </>
  ),
  copy: (
    <>
      <rect x="8.5" y="8.5" width="11" height="11" rx="2" />
      <path d="M15.5 8.5V6a1.5 1.5 0 0 0-1.5-1.5H6A1.5 1.5 0 0 0 4.5 6v8A1.5 1.5 0 0 0 6 15.5h2.5" />
    </>
  ),
  phone: (
    <>
      <rect x="7" y="2.5" width="10" height="19" rx="2.5" />
      <path d="M11 18.5h2" />
    </>
  ),
  share: (
    <>
      <path d="M12 3.5v11M7.5 8 12 3.5 16.5 8" />
      <path d="M5 12.5V19a1.5 1.5 0 0 0 1.5 1.5h11A1.5 1.5 0 0 0 19 19v-6.5" />
    </>
  ),
} as const;

export type IconName = keyof typeof paths;

export function Icon(props: { name: IconName; size?: number; className?: string }) {
  const size = props.size ?? 20;
  return (
    <svg
      className={props.className ? `icon ${props.className}` : "icon"}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      {paths[props.name]}
    </svg>
  );
}
