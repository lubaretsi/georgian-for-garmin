import { useId } from "react";

/** The Georgian letter ქ from the app icon (crates/app/icon.svg: Noto Sans Georgian, weight 600). */
const KANI =
  "M511.8 772Q483.2 772 460 763.1Q436.7 754.2 420.2 737.6Q403.6 721 394.9 698.3Q386.1 675.6 386.1 648Q386.1 632.9 388.8 619.9Q391.5 606.8 396.3 595.9H458.1Q455.6 605.7 453.5 618.6Q451.5 631.4 451.5 642.7Q451.5 664.8 457.9 682.6Q464.4 700.3 477.9 710.6Q491.4 720.9 512 720.9Q534.5 720.9 547.7 710.7Q560.9 700.6 566.7 681.6Q572.5 662.6 572.5 636.6V499.9Q572.5 457.4 559.1 433.7Q545.8 409.9 514.3 409.9Q500.5 409.9 489.2 416.7Q477.8 423.4 471.4 436.4Q464.9 449.4 464.9 468.8Q464.9 479 466.1 488.8Q467.3 498.7 469.9 508.8H407.1Q404.1 500.3 401.7 489.6Q399.4 478.8 399.4 464Q399.4 430 413.3 406.7Q427.1 383.3 449.2 371.1Q471.3 358.9 495.4 358.9Q514.9 358.9 529.3 364.3Q543.6 369.7 554.5 379Q565.4 388.4 573.3 399.9H576Q574.9 391.7 573.6 376.3Q572.3 360.9 572.3 349.3V252H637.9V638.2Q637.9 680.3 623.2 710.3Q608.4 740.3 580.4 756.2Q552.3 772 511.8 772Z";

/** The app's icon: ქ on the red rounded square, the same drawing as crates/app/icon.svg. */
export function AppIcon(props: { size: number; x?: number; y?: number; className?: string }) {
  const id = useId();
  return (
    <svg
      viewBox="100 100 824 824"
      x={props.x}
      y={props.y}
      width={props.size}
      height={props.size}
      aria-hidden
      focusable="false"
      className={props.className}
    >
      <defs>
        <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#e3243f" />
          <stop offset="1" stopColor="#b00c26" />
        </linearGradient>
      </defs>
      <rect x="100" y="100" width="824" height="824" rx="185" fill={`url(#${id})`} />
      <path fill="#fff" d={KANI} />
    </svg>
  );
}
