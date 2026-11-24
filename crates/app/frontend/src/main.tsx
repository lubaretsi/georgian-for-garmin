// The entry point. In the app's window, the UI runs over the app's Tauri commands
// (TauriBackend), filling the window. In a plain browser on the development server
// (`pnpm dev`) it runs over the simulated watch instead (src/mock/preview.tsx), which the app's
// build leaves out.

import { getVersion } from "@tauri-apps/api/app";
import { isTauri } from "@tauri-apps/api/core";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "./ui/global.css";
import type { Lang, Platform } from "./core/backend";
import { TauriBackend } from "./core/TauriBackend";
import { FlowApp } from "./flow/FlowApp";

const element = document.getElementById("root");
if (!element) throw new Error("index.html has no #root");
const root = createRoot(element);

if (isTauri()) {
  const backend = new TauriBackend();
  // The window opens even if one of these fails: Linux's words, the default language.
  void Promise.all([
    backend.platform().catch((): Platform => "linux"),
    backend.savedLanguage().catch((): Lang | null => null),
    getVersion().catch(() => ""),
  ]).then(([platform, saved, version]) =>
    root.render(
      <StrictMode>
        <FlowApp backend={backend} lang={saved ?? "ka"} platform={platform} version={version} />
      </StrictMode>,
    ),
  );
} else if (import.meta.env.DEV) {
  void import("./mock/preview").then(({ renderPreview }) => renderPreview(root));
}
