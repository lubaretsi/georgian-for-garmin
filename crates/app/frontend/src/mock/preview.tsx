// `pnpm dev` in a plain browser: the app over the simulated Venu 2, in a window frame, with the
// watch simulator beside it. Loaded only by the development server, never by the app's build.

import { StrictMode } from "react";
import type { Root } from "react-dom/client";

import { FlowApp } from "../flow/FlowApp";
import { WindowFrame } from "../ui/WindowFrame";
import { MockBackend } from "./MockBackend";
import { Simulator } from "./Simulator";

export function renderPreview(root: Root): void {
  const backend = new MockBackend({ watch: "venu2", connection: "unplugged" });
  root.render(
    <StrictMode>
      <div style={{ display: "flex", gap: 24, padding: 24, alignItems: "flex-start" }}>
        <WindowFrame width={480} height={720} theme="light" lang="ka" platform="macos">
          <FlowApp backend={backend} lang="ka" platform="macos" version="0.3.4" />
        </WindowFrame>
        <Simulator backend={backend} />
      </div>
    </StrictMode>,
  );
}
