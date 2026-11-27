// The simulated watches: each one's fixture steps (fixtures/core/<scenario>/<step>.json), as
// the physical states the simulator moves between.

import type { HolderKind } from "../core/types.gen";
import type { Platform } from "../core/backend";

/** A fixture step: `<scenario>/<step>`. */
export type Step = `${string}/${string}`;

export type WatchDef = {
  id: string;
  label: string;
  /** How it can connect: over MTP, as a drive, or both. */
  mtp: boolean;
  drive: Step | null;
  /** What `inspect` reads before anything is written, and what a restart then imports. */
  start: Step | null;
  startPending?: Step;
  install?: { output: Step; staged: Step; imported: Step } | { failure: Step };
  restore?: { output: Step; staged: Step; imported: Step };
  /** After a failed install (Make the next write fail): what it left, and the error. */
  failedWrite?: { failure: Step; after: Step; restore: { output: Step; staged: Step; imported: Step } };
  /** issue_link's answers for this watch, by what is shared. */
  links: Partial<Record<string, Step>>;
};

const VENU2_RESTORE = {
  output: "venu2/restore_stock",
  staged: "venu2/inspect-restore-staged",
  imported: "venu2/inspect-restored",
} as const;

export const WATCH_DEFS = {
  venu2: {
    id: "venu2",
    label: "Venu 2",
    mtp: true,
    drive: "venu2-drive/inspect_drive",
    start: "venu2/inspect",
    install: { output: "venu2/install", staged: "venu2/inspect-staged", imported: "venu2/inspect-restarted" },
    restore: VENU2_RESTORE,
    failedWrite: {
      failure: "venu2-write-fails/install-rollback-incomplete",
      after: "venu2-write-fails/inspect-after-incomplete",
      restore: {
        output: "venu2-write-fails/restore_stock",
        staged: "venu2-write-fails/inspect-restore-staged",
        imported: "venu2-write-fails/inspect-restored",
      },
    },
    links: { georgian: "venu2/issue_link-georgian", not_installed: "venu2/issue_link-not_installed" },
  },
  unconfirmed: {
    id: "unconfirmed",
    label: "Example Unconfirmed (Korean fonts, short names)",
    mtp: true,
    drive: null,
    start: "unconfirmed/inspect",
    install: {
      output: "unconfirmed/install",
      staged: "unconfirmed/inspect-staged",
      imported: "unconfirmed/inspect-restarted",
    },
    links: { georgian: "unconfirmed/issue_link-georgian" },
  },
  koreanBoxes: {
    id: "koreanBoxes",
    label: "Example Korean (boxes)",
    mtp: true,
    drive: null,
    start: "korean-boxes/inspect",
    install: {
      output: "korean-boxes/install",
      staged: "korean-boxes/inspect-staged",
      imported: "korean-boxes/inspect-restarted",
    },
    links: { boxes: "korean-boxes/issue_link-boxes" },
  },
  koreanUnused: {
    id: "koreanUnused",
    label: "Example Korean Unused (0.3.4's font left)",
    mtp: true,
    drive: null,
    start: "korean-unused/inspect",
    restore: {
      output: "korean-unused/restore_stock",
      staged: "korean-unused/inspect-restore-staged",
      imported: "korean-unused/inspect-restored",
    },
    links: {},
  },
  expectedWaiting: {
    id: "expectedWaiting",
    label: "Example Expected (another font waiting)",
    mtp: true,
    drive: null,
    start: "expected-waiting/inspect",
    startPending: "expected-waiting/inspect-restarted",
    install: {
      output: "expected-waiting/install",
      staged: "expected-waiting/inspect-staged",
      imported: "expected-waiting/inspect-installed",
    },
    // `read_before`: a report whose state is from before the install (§4).
    links: { georgian: "expected-waiting/issue_link-georgian", read_before: "expected-waiting/issue_link-read_before" },
  },
  fontUnused: {
    id: "fontUnused",
    label: "Example Font Unused",
    mtp: true,
    drive: null,
    start: "font-unused/inspect",
    links: { font_list: "font-unused/issue_link-font_list" },
  },
  fontUnusedBetaLeft: {
    id: "fontUnusedBetaLeft",
    label: "Example Font Unused (earlier test version's fonts)",
    mtp: true,
    drive: null,
    start: "font-unused-beta-left/inspect",
    restore: {
      output: "font-unused-beta-left/restore_stock",
      staged: "font-unused-beta-left/inspect-restore-staged",
      imported: "font-unused-beta-left/inspect-restored",
    },
    links: {},
  },
  driveOnly: {
    id: "driveOnly",
    label: "Example Drive Only (USB drive only)",
    mtp: false,
    drive: "drive-only/inspect_drive",
    start: null,
    links: { font_list: "drive-only/issue_link-font_list" },
  },
  predicted: {
    id: "predicted",
    label: "Example Predicted",
    mtp: true,
    drive: null,
    start: "predicted/inspect",
    install: { failure: "predicted/install" },
    links: { font_list: "predicted/issue_link-font_list" },
  },
} satisfies Record<string, WatchDef>;

export type WatchId = keyof typeof WATCH_DEFS;

/** The busy fixture for a holder on a platform: the closest the fixtures have. */
export function busyStep(kind: HolderKind, platform: Platform): Step {
  switch (kind) {
    case "garmin_express":
      return platform === "windows" ? "busy/inspect-garmin_express-windows" : "busy/inspect-garmin_express-macos";
    case "macos_camera_service":
      return "busy/inspect-macos_camera_service-macos";
    case "desktop_mtp":
      return platform === "linux" ? "busy/inspect-desktop_mtp-linux" : "busy/inspect-desktop_mtp-macos";
    case "other":
      return platform === "macos" ? "busy/inspect-other-macos-cut" : "busy/inspect-other-linux";
    case "unknown":
      return "busy/inspect-unknown-linux";
    case "this_app":
      return platform === "windows" ? "busy/inspect-this_app-windows" : "busy/inspect-this_app-macos";
  }
}
