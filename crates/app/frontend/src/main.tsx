// `pnpm dev`: the new UI over the simulated Venu 2 (phase 1 has no Tauri backend yet).

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "./ui/global.css";
import { FlowApp } from "./flow/FlowApp";
import { MockBackend } from "./mock/MockBackend";
import { Simulator } from "./mock/Simulator";
import { WindowFrame } from "./ui/WindowFrame";

const backend = new MockBackend({ watch: "venu2", connection: "unplugged" });
const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root");

createRoot(root).render(
  <StrictMode>
    <div style={{ display: "flex", gap: 24, padding: 24, alignItems: "flex-start" }}>
      <WindowFrame width={480} height={720} theme="light" lang="ka" platform="macos">
        <FlowApp backend={backend} lang="ka" platform="macos" version="0.3.4" />
      </WindowFrame>
      <Simulator backend={backend} />
    </div>
  </StrictMode>,
);
