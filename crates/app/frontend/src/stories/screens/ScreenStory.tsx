import { useEffect, useState } from "react";

import type { Lang, Platform } from "../../core/backend";
import type { ActionId, ActionPayload, ScreenModel } from "../../model/screens";
import { Shell } from "../../shell/Shell";
import { moreFor, resolveModel, type StoryModel } from "./data";

export type ScreenStoryArgs = {
  model: StoryModel;
  /** Pins the platform (busy and help text); otherwise the toolbar's. */
  platform?: Platform;
  /** Shows Restore original font in More. */
  restorable?: boolean;
  onAction: (id: ActionId, payload?: ActionPayload) => void;
};

/**
 * One screen in the shell, as the app would show it. The header's language switch calls
 * `onLanguage` (the story's render sets the toolbar language); answer changes on the check
 * screen apply locally so the form can be tried. Every action is also logged in the Actions
 * panel.
 */
export function ScreenStory(
  props: ScreenStoryArgs & { lang: Lang; platform: Platform; onLanguage: (lang: Lang) => void },
) {
  const [model, setModel] = useState<ScreenModel>(() => resolveModel(props.model, props.lang));
  useEffect(() => setModel(resolveModel(props.model, props.lang)), [props.model, props.lang]);

  const watch = "watch" in model ? model.watch : null;
  return (
    <Shell
      model={model}
      more={moreFor(props.lang, watch, props.restorable ?? false)}
      lang={props.lang}
      platform={props.platform}
      onAction={(id, payload) => {
        if (payload?.action === "answer" && model.screen === "check") setModel({ ...model, answers: payload.answers });
        props.onAction(id, payload);
      }}
      onLanguage={props.onLanguage}
    />
  );
}
