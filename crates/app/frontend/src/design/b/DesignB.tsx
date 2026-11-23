import "./theme.css";

import type { Lang, Platform } from "../../core/backend";
import { copyFor } from "../../copy";
import { Shell, type ShellProps } from "./shell/Shell";
import { type Theme, WindowFrame } from "./shell/WindowFrame";

/**
 * Direction B in its window, on a quiet desktop backdrop: what the Design/B stories show.
 * `data-shot` marks the area the screenshots take.
 */
export function DesignB(props: ShellProps & { theme: Theme; platform: Platform; lang: Lang; width: number; height: number }) {
  const dark = props.theme === "dark";
  return (
    <div
      data-shot
      className="design-b-stage"
      style={{
        display: "inline-block",
        padding: 40,
        background: dark
          ? "radial-gradient(120% 90% at 30% 0%, #3a3d4a 0%, #1d1e24 60%, #141418 100%)"
          : "radial-gradient(120% 90% at 30% 0%, #e9ecf3 0%, #d9dce5 55%, #cdd0da 100%)",
      }}
    >
      <WindowFrame
        platform={props.platform}
        theme={props.theme}
        lang={props.lang}
        width={props.width}
        height={props.height}
        title={copyFor(props.lang).app.name}
      >
        <Shell {...props} />
      </WindowFrame>
    </div>
  );
}
