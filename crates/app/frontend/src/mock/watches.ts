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
  lily2active: {
    id: "lily2active",
    label: "Lily 2 Active",
    mtp: true,
    drive: null,
    start: "lily2active/inspect",
    install: {
      output: "lily2active/install",
      staged: "lily2active/inspect-staged",
      imported: "lily2active/inspect-restarted",
    },
    links: { georgian: "lily2active/issue_link-georgian" },
  },
  instinct3: {
    id: "instinct3",
    label: "Instinct 3 AMOLED 50mm",
    mtp: true,
    drive: null,
    start: "instinct3/inspect",
    install: { output: "instinct3/install", staged: "instinct3/inspect-staged", imported: "instinct3/inspect-restarted" },
    links: { boxes: "instinct3/issue_link-boxes" },
  },
  venu3: {
    id: "venu3",
    label: "Venu 3 (another font waiting)",
    mtp: true,
    drive: null,
    start: "venu3-waiting/inspect",
    startPending: "venu3-waiting/inspect-restarted",
    install: {
      output: "venu3-waiting/install",
      staged: "venu3-waiting/inspect-staged",
      imported: "venu3-waiting/inspect-installed",
    },
    links: {},
  },
  fenix6: {
    id: "fenix6",
    label: "fenix 6S Pro",
    mtp: true,
    drive: null,
    start: "fenix6/inspect",
    links: { font_list: "fenix6/issue_link-font_list" },
  },
  fenix6BetaLeft: {
    id: "fenix6BetaLeft",
    label: "fenix 6S Pro (earlier test version's fonts)",
    mtp: true,
    drive: null,
    start: "fenix6-beta-left/inspect",
    restore: {
      output: "fenix6-beta-left/restore_stock",
      staged: "fenix6-beta-left/inspect-restore-staged",
      imported: "fenix6-beta-left/inspect-restored",
    },
    links: {},
  },
  forerunner245: {
    id: "forerunner245",
    label: "Forerunner 245 (USB drive only)",
    mtp: false,
    drive: "forerunner245-drive/inspect_drive",
    start: null,
    links: { font_list: "forerunner245-drive/issue_link-font_list" },
  },
  forerunner265: {
    id: "forerunner265",
    label: "Forerunner 265",
    mtp: true,
    drive: null,
    start: "forerunner265/inspect",
    install: { failure: "forerunner265/install" },
    links: { font_list: "forerunner265/issue_link-font_list" },
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
  }
}
