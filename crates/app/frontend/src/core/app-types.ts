// The app's own shapes, which the CLI's schemas don't declare: written by hand from
// crates/app/src/main.rs (`Failure`, `Shared`), crates/app/src/share.rs (`PhonePage`,
// `MarkPicture`) and crates/core/src/error.rs (`Rollback`). Phase 2 derives `JsonSchema` for
// them and generates these too.

import type { Busy, ErrorCode, Kind } from "./types.gen";

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
