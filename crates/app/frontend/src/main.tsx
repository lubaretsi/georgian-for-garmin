// The entry point. In the app's window, the UI runs over the app's Tauri commands
// (TauriBackend), filling the window. In a plain browser on the development server
// (`pnpm dev`) it runs over the simulated watch instead (src/mock/preview.tsx), which the app's
// build leaves out. When index.html's WebView check found the web engine too old, it has put
// its message in place of #root, and nothing starts.

import { getVersion } from "@tauri-apps/api/app";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "./ui/global.css";
import type { Lang, Platform } from "./core/backend";
import { TauriBackend } from "./core/TauriBackend";
import { FlowApp } from "./flow/FlowApp";

const element = document.getElementById("root");

if (element && isTauri()) {
  const root = createRoot(element);
  const backend = new TauriBackend();
  // macOS draws its window buttons over the toolbar (the overlay title bar), whatever platform's
  // words the demo build shows.
  const overlayTitleBar = /Macintosh|Mac OS X/.test(navigator.userAgent);
  // The window opens even if one of these fails: Linux's words, the default language.
  void Promise.all([
    backend.platform().catch((): Platform => "linux"),
    backend.savedLanguage().catch((): Lang | null => null),
    getVersion().catch(() => ""),
    backend.demoMode().catch(() => false),
  ]).then(([platform, saved, version, demo]) =>
    root.render(
      <StrictMode>
        <FlowApp
          backend={backend}
          lang={saved ?? "ka"}
          platform={platform}
          overlayTitleBar={overlayTitleBar}
          version={version}
          demo={demo}
          onDemoWatch={
            demo ? (action) => void invoke("demo_watch", { action }).catch(() => undefined) : undefined
          }
        />
      </StrictMode>,
    ),
  );
} else if (element && import.meta.env.DEV) {
  const root = createRoot(element);
  void import("./mock/preview").then(({ renderPreview }) => renderPreview(root));
}
