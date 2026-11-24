// The whole app: the flow (reducer and effects) over a `Backend`, drawn by the shell.

import { useEffect, useState } from "react";

import type { Backend, Lang, Platform } from "../core/backend";
import type { ActionId, ActionPayload } from "../model/screens";
import { Shell } from "../shell/Shell";
import { moreModel, toScreen } from "./screen";
import { FlowStore, useFlowState } from "./store";

export function FlowApp(props: {
  backend: Backend;
  lang: Lang;
  platform: Platform;
  /** Whether macOS's window buttons sit over the toolbar; defaults to `platform === "macos"`. */
  overlayTitleBar?: boolean;
  /** The demo build: More's About says so. */
  demo?: boolean;
  version: string;
  /** Divides every delay of the flow (the story tests run it faster). */
  speed?: number;
}) {
  const { backend, lang, version, speed } = props;
  const [store] = useState(() => new FlowStore(backend, lang, version, speed));
  const state = useFlowState(store);

  useEffect(() => {
    store.start();
    return () => store.stop();
  }, [store]);

  // The language chosen outside the app (Storybook's toolbar).
  useEffect(() => {
    if (store.getState().lang !== lang) store.dispatch({ type: "language", lang });
  }, [store, lang]);

  return (
    <Shell
      model={toScreen(state)}
      more={moreModel(state)}
      lang={state.lang}
      platform={props.platform}
      overlayTitleBar={props.overlayTitleBar}
      demo={props.demo}
      onAction={(id: ActionId, payload?: ActionPayload) => store.dispatch({ type: "action", id, ...(payload && { payload }) })}
      onLanguage={(next: Lang) => store.dispatch({ type: "language", lang: next })}
    />
  );
}
