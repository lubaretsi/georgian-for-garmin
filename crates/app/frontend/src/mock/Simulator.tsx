// The watch simulator beside a flow story's window: the physical side of the journey
// (contract §8, Flows). A developer tool: English only, not part of the app.

import { type CSSProperties, useId, useSyncExternalStore } from "react";

import type { Result } from "../model/screens";
import type { MockBackend } from "./MockBackend";

const RESULTS: { value: Result | "auto"; label: string }[] = [
  { value: "auto", label: "What its font shows" },
  { value: "georgian", label: "Georgian letters" },
  { value: "boxes", label: "Boxes" },
  { value: "question_marks", label: "Question marks" },
  { value: "some_letters", label: "Some letters, some boxes" },
  { value: "no_notification", label: "Nothing (not passed on)" },
];

/** What the watch shows, with words beside the symbols (axe can't judge a symbol-only line). */
const NOTIFICATION: Record<Result, string> = {
  georgian: "ქართული",
  boxes: "□□□□□□□ (boxes)",
  question_marks: "??????? (question marks)",
  some_letters: "ქა□□□□□ (some letters)",
  no_notification: "(nothing arrives)",
  not_installed: "□□□□□□□ (boxes)",
};

const panel: CSSProperties = {
  width: 260,
  padding: 16,
  border: "1px solid var(--border)",
  borderRadius: 8,
  background: "var(--surface)",
  color: "var(--text)",
  font: "14px/1.4 system-ui, sans-serif",
  display: "grid",
  gap: 12,
  alignContent: "start",
};
const group: CSSProperties = { display: "grid", gap: 6, border: 0, padding: 0, margin: 0 };
const button: CSSProperties = {
  font: "inherit",
  padding: "6px 10px",
  borderRadius: 6,
  border: "1px solid var(--border)",
  background: "var(--surface-raised)",
  color: "var(--text)",
  textAlign: "left",
  cursor: "pointer",
};

export function Simulator({ backend }: { backend: MockBackend }) {
  const sim = useSyncExternalStore(backend.subscribe, backend.simulation);
  const def = backend.def;
  const selectId = useId();
  const connection = sim.restartingTo ? "restarting…" : { mtp: "connected (MTP)", drive: "connected as a USB drive", unplugged: "unplugged" }[sim.connection];
  const notification = NOTIFICATION[backend.notification()];
  return (
    <aside aria-label="Watch simulator" style={panel}>
      <h2 style={{ margin: 0, fontSize: 16 }}>Watch simulator</h2>
      <p style={{ margin: 0 }}>
        <strong>{def.label}</strong>
        <br />
        <span data-testid="sim-connection">{connection}</span>
        {sim.holder && <> · held by {sim.holder}</>}
        <br />
        Font: <code>{sim.current ?? "-"}</code>
        {sim.pending && (
          <>
            <br />
            Waiting for a restart: <code>{sim.pending}</code>
          </>
        )}
      </p>
      <fieldset style={group}>
        <legend style={{ fontWeight: 600 }}>USB</legend>
        <button type="button" style={button} disabled={!def.mtp} onClick={() => backend.plugIn("mtp")}>
          Plug in (MTP)
        </button>
        <button type="button" style={button} disabled={!def.drive} onClick={() => backend.plugIn("drive")}>
          Plug in (drive)
        </button>
        <button type="button" style={button} onClick={() => backend.unplug()}>
          Unplug
        </button>
      </fieldset>
      <fieldset style={group}>
        <legend style={{ fontWeight: 600 }}>Watch</legend>
        <button type="button" style={button} onClick={() => backend.restart()}>
          Restart watch
        </button>
        <button type="button" style={button} disabled={sim.skipNextImport} onClick={() => backend.skipNextImport()}>
          {sim.skipNextImport ? "The next restart won't import" : "Next restart doesn't import"}
        </button>
        <button
          type="button"
          style={button}
          disabled={!def.failedWrite || sim.failNextWrite}
          onClick={() => backend.failNextWrite()}
        >
          {sim.failNextWrite ? "The next write will fail" : "Make the next write fail"}
        </button>
      </fieldset>
      <fieldset style={group}>
        <legend style={{ fontWeight: 600 }}>Other programs</legend>
        <button type="button" style={button} disabled={!!sim.holder} onClick={() => backend.hold("garmin_express")}>
          Garmin Express holds it
        </button>
        <button type="button" style={button} disabled={!sim.holder} onClick={() => backend.hold(null)}>
          Release
        </button>
      </fieldset>
      <div style={group}>
        <label htmlFor={selectId} style={{ fontWeight: 600 }}>
          Phone: the next test notification shows
        </label>
        <select
          id={selectId}
          style={button}
          value={sim.phoneResult}
          onChange={(event) => backend.setPhoneResult(event.target.value as Result | "auto")}
        >
          {RESULTS.map((r) => (
            <option key={r.value} value={r.value}>
              {r.label}
            </option>
          ))}
        </select>
        <p style={{ margin: 0 }}>
          On the watch: <span lang="ka" data-testid="sim-notification">{notification}</span>
        </p>
      </div>
      {sim.opened.length > 0 && (
        <div style={group}>
          <h3 style={{ margin: 0, fontSize: 14 }}>Opened in the browser</h3>
          {/*
            Shortened: a share link carries the whole report (thousands of characters), and a list
            item taller than the window leaves axe unable to read its contrast.
          */}
          <ul data-testid="sim-opened" style={{ margin: 0, padding: 0, listStyle: "none", overflowWrap: "anywhere" }}>
            {sim.opened.map((url, i) => (
              <li key={`${i}-${url}`}>{url.length > 120 ? `${url.slice(0, 119)}…` : url}</li>
            ))}
          </ul>
        </div>
      )}
      {(sim.saved.length > 0 || sim.copied) && (
        <div style={group} data-testid="sim-files">
          <h3 style={{ margin: 0, fontSize: 14 }}>Files the app saved</h3>
          <ul style={{ margin: 0, padding: 0, listStyle: "none", overflowWrap: "anywhere" }}>
            {sim.saved.map((file, i) => (
              <li key={`${i}-${file}`}>Saved {file}</li>
            ))}
            {sim.shownFile && <li>Shown in its folder: {sim.shownFile}</li>}
            {sim.copied && <li>Copied to the clipboard ({sim.copied.length} characters)</li>}
          </ul>
        </div>
      )}
    </aside>
  );
}
