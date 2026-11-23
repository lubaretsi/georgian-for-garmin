import type { Lang } from "../core/backend";
import { type Copy, fmt, list } from "../copy";
import type { ActionId, Confidence, WatchInfo } from "../model/screens";
import type { FooterAction } from "../ui/ActionBar";
import type { CopyLabels } from "../ui/CopyBlock";
import type { ScreenContext } from "./types";

export function copyLabels(copy: Copy): CopyLabels {
  return { copy: copy.actions.copy, copied: copy.actions.copied, copyFailed: copy.actions.copyFailed };
}

/** The one line that says how sure we are Georgian works on this watch (§5.2). */
export function confidenceLine(copy: Copy, lang: Lang, watch: WatchInfo, confidence: Confidence): string {
  const values = { model: watch.model, software: watch.software };
  if (confidence === "confirmed_other_software") {
    return watch.confirmedSoftware.length > 0
      ? fmt(copy.confidence.confirmed_other_software, { ...values, versions: list(lang, watch.confirmedSoftware) })
      : fmt(copy.confidence.confirmed_other_software_unlisted, values);
  }
  return fmt(copy.confidence[confidence], values);
}

/** Why checking on the watch helps; null when Georgian is confirmed for this watch. */
export function checkWhy(copy: Copy, watch: WatchInfo, confidence: Confidence): string | null {
  if (confidence === "confirmed") return null;
  return fmt(copy.checkWhy[confidence], { model: watch.model, software: watch.software });
}

export function shareFontList(ctx: ScreenContext): FooterAction {
  return {
    label: ctx.copy.actions.shareFontList,
    onPress: () => ctx.emit("share", { action: "share", what: "font_list" }),
  };
}

export function reportProblem(ctx: ScreenContext): FooterAction {
  return {
    label: ctx.copy.actions.reportProblem,
    onPress: () => ctx.emit("share", { action: "share", what: "problem" }),
  };
}

export function emitting(ctx: ScreenContext, label: string, id: ActionId): FooterAction {
  return { label, onPress: () => ctx.emit(id) };
}

export function softwareLine(copy: Copy, watch: WatchInfo): string {
  return fmt(copy.watchCard.software, { software: watch.software });
}
