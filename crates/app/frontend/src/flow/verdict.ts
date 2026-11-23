// What the app makes of a watch it read (contract §5.1), and how the restart step checks a
// write took effect (§4.1).

import type { InspectDriveOutput, InspectOutput } from "../core/types.gen";
import type { Confidence, Op, Verdict, WatchInfo } from "../model/screens";

/** How sure the project is that Georgian works on this watch (§5.1, `confidence`). */
export function confidenceOf(inspect: InspectOutput): Confidence {
  if (inspect.support.status === "tested") return "confirmed";
  if (inspect.tested.status === "tested_other_software") return "confirmed_other_software";
  if (inspect.packs.some((pack) => pack.parts.some((part) => part.tested === "tested"))) {
    return "confirmed_some_fonts";
  }
  return "unconfirmed";
}

/** The first matching rule of §5.1's table. */
export function verdictOf(inspect: InspectOutput): Verdict {
  const { state, support } = inspect;
  if (support.status === "unsupported") {
    // Restore leads only where an earlier build of ours is seen (§5.1 rule 1); More keeps it.
    const ours = inspect.fonts.some((f) => f.restore && (f.file === "georgian" || f.file === "georgian_beta"));
    return { kind: "unsupported", reason: support.reason ?? null, restorable: inspect.restorable && ours };
  }
  switch (state) {
    case "unknown":
      return { kind: "unreadable" };
    case "unsupported":
      return { kind: "no_font" };
    case "waiting":
      return { kind: "restart_pending", variant: "georgian" };
    case "stock_waiting":
      return { kind: "restart_pending", variant: "stock" };
    case "other":
      return { kind: "blocked", reason: waitingOtherFont(inspect) ? "waiting_other_font" : "unknown_font" };
    case "installed":
      return { kind: "installed", confidence: confidenceOf(inspect) };
    case "not_installed":
      return { kind: "install", confidence: confidenceOf(inspect) };
    case "partly":
    case "beta":
    case "not_visible":
      return { kind: "install", confidence: confidenceOf(inspect), note: state };
  }
}

/**
 * A target part's file that is neither ours nor Garmin's sits in the normal listing, waiting
 * for a restart, rather than being the font the watch imported.
 */
function waitingOtherFont(inspect: InspectOutput): boolean {
  return inspect.fonts.some((font) => font.target && font.file === "other" && !font.imported);
}

/**
 * Whether a restart after `op` shows its result: only bitmap fonts wait in the normal listing
 * until the watch imports them; a TrueType part's state doesn't change at the restart (§4.1).
 */
export function verifiable(packs: readonly { kind: string }[]): boolean {
  return packs.some((pack) => pack.kind === "bitmap");
}

/** Whether `inspect`, read after a restart, shows `op` took effect (§4.1, Verified). */
export function verified(op: Op, inspect: InspectOutput): boolean {
  if (op === "install") return inspect.state === "installed";
  // Restore: nothing of ours or waiting left, and every part restore writes is Garmin's. A
  // watch whose fonts get no Georgian (state `unsupported`) counts by its parts alone.
  return (
    (inspect.state === "not_installed" || inspect.state === "unsupported") &&
    inspect.fonts.filter((font) => font.restore).every((font) => font.file === "stock" || font.file === "none")
  );
}

export function watchInfo(source: InspectOutput | InspectDriveOutput): WatchInfo {
  return {
    model: source.device.model,
    modelPart: source.device.part_number,
    software: source.device.software_version,
    confirmedSoftware: "tested" in source ? source.tested.verified_software : [],
  };
}
