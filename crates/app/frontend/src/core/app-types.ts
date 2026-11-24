// The app's own shapes, which the CLI's schemas don't declare: written by hand from
// crates/app/src/main.rs (`Failure`, `Shared`), crates/app/src/share.rs (`PhonePage`,
// `MarkPicture`) and crates/core/src/error.rs (`Rollback`). Phase 2 derives `JsonSchema` for
// them and generates these too.

import type { Busy, ErrorCode, Holder, IssueLinkInput, Kind, WatchReport } from "./types.gen";

/**
 * PENDING: the core's `diagnose` operation (contract §11) is being built on `feat/diagnose`;
 * until it lands, its input and output are typed here by hand, after that branch's shape, and
 * only the mock answers. Once it lands, `pnpm gen:types` generates them and these go.
 */
export type DiagnoseEvent = {
  /** Local time, without a zone: "2026-11-24T03:14:05". */
  time: string;
  kind: "poll" | "operation_start" | "operation_end" | "answer" | "share";
  /** The error code or the state, at most 200 characters: "no_device", "install: staged_verified". */
  detail: string;
  /** The watch's model name, if one was read. */
  model: string | null;
};
export type DiagnoseInput = { events?: DiagnoseEvent[] };
/** `Holder`, with `this_app` when the busy detection names this very process. */
export type DiagnoseHolder = Omit<Holder, "kind"> & { kind: Holder["kind"] | "this_app" };
export type DiagnoseOutput = {
  /** `build`: "release" or "debug", plus ", dev operations" or ", beta fonts". */
  app: { version: string; build: string };
  system: { os: string; os_version: string; arch: string };
  /** Garmin devices only (vendor 0x091E); never serial numbers. `note`: why not listed, or what this platform can't tell. */
  usb: {
    listed: boolean;
    note: string | null;
    devices: {
      product_id: number;
      product_name: string | null;
      mtp: boolean;
      /** Empty on Windows. */
      interfaces: { class: number; subclass: number; protocol: number }[];
      holder: DiagnoseHolder | null;
    }[];
  };
  /** A read-only connect attempt through the process's own connection. */
  connection: { ok: boolean; code: ErrorCode | null; message: string | null; busy: Busy | null };
  /** Mounted volumes that look like a Garmin watch. */
  drives: {
    listed: boolean;
    note: string | null;
    volumes: {
      volume: string;
      model: string | null;
      part_number: string | null;
      software_version: string | null;
      error: string | null;
    }[];
  };
  /** inspect's report, only when the connection worked. */
  watch: WatchReport | null;
  events: DiagnoseEvent[];
  /** All of the above as readable text; its first line says it has no serial number or watch ID. */
  text: string;
};

/**
 * `issue_link`'s input, with the `error` it is planned to take alone, as it takes `busy` alone:
 * a connect problem with no watch read (contract §6). PENDING: the core doesn't take it yet
 * (phase 2); until then only the mock answers it.
 */
export type IssueLinkRequest = IssueLinkInput & { error?: { code: ErrorCode; message: string } | null };

/** An operation failure as the frontend receives it; `busy` only for `device_busy`. */
export type Failure = {
  code: ErrorCode;
  message: string;
  busy?: Busy;
  /** A failed install or restore: how undoing it went. */
  rollback?: Rollback;
};

export type Rollback = {
  /** `complete`: every change was undone; `incomplete`: the watch holds what `files` says. */
  status: "complete" | "incomplete";
  files: RollbackFile[];
};

export type RollbackFile = {
  /** "GARMIN/ExtData/<name>". */
  path: string;
  /** `delete`: a file this call created; `restore`: one it replaced, written back. */
  action: "delete" | "restore";
  /** False: the file may still be there (delete), or missing or different (restore). */
  done: boolean;
  /** What happened, in words. */
  result: string;
};

/** `share_report`'s result. */
export type Shared = {
  /** The whole report is in the link; otherwise it is on the clipboard and the form asks for a paste. */
  complete: boolean;
  /** The link opened (the demo build opens nothing and shows it instead). */
  url: string;
};

/** The phone test page, in the app's language. */
export type PhonePage = {
  url: string;
  /** The URL's QR code as an SVG `data:` URL. */
  qr: string;
};

/** A beta build's test mark as the marker question shows it. */
export type MarkPicture = {
  part: string;
  kind: Kind;
  variant: string;
  style: string;
  /** The test word in this part's Georgian build as a PNG `data:` URL. */
  picture: string;
};

/** Whether a rejection is a `Failure` (the backend rejects with nothing else). */
export function isFailure(value: unknown): value is Failure {
  return (
    typeof value === "object" &&
    value !== null &&
    "code" in value &&
    typeof value.code === "string" &&
    "message" in value &&
    typeof value.message === "string"
  );
}
