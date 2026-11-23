import { useEffect, useState } from "react";

import type { Lang, Platform } from "@/core/backend";
import { copyFor } from "@/copy";
import type { ActionId, ActionPayload, MoreModel, ScreenModel } from "@/model/screens";

import "./theme.css";
import { AppShell } from "./shell/AppShell";
import { WindowChrome } from "./shell/WindowChrome";
import { cn } from "./ui/cn";

/**
 * One screen of direction A inside a native-looking window, on a quiet desk backdrop.
 * Answer changes on the check screen apply locally so the form can be tried.
 */
export function Preview(props: {
  model: ScreenModel;
  more: MoreModel;
  lang: Lang;
  platform: Platform;
  theme: "light" | "dark";
  width: number;
  height: number;
  onAction: (id: ActionId, payload?: ActionPayload) => void;
  onLanguage: (lang: Lang) => void;
}) {
  const [model, setModel] = useState(props.model);
  useEffect(() => setModel(props.model), [props.model]);
  const dark = props.theme === "dark";
  return (
    <div
      className={cn(
        "design-a wa:grid wa:min-h-screen wa:place-items-center wa:p-10",
        dark
          ? "wa:bg-[radial-gradient(120%_80%_at_50%_0%,#2a2420_0%,#110e0c_70%)]"
          : "wa:bg-[radial-gradient(120%_80%_at_50%_0%,#f1ebe4_0%,#dcd3c9_75%)]",
      )}
      data-theme={props.theme}
      lang={props.lang}
    >
      <WindowChrome platform={props.platform} title={copyFor(props.lang).app.name} width={props.width} height={props.height} dark={dark}>
        <AppShell
          model={model}
          more={props.more}
          lang={props.lang}
          platform={props.platform}
          onAction={(id, payload) => {
            if (payload?.action === "answer" && model.screen === "check") setModel({ ...model, answers: payload.answers });
            props.onAction(id, payload);
          }}
          onLanguage={props.onLanguage}
        />
      </WindowChrome>
    </div>
  );
}
