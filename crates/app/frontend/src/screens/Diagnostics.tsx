import { FolderOpen, Lock } from "lucide-react";

import type { ScreenModel } from "../model/screens";
import { Alert } from "../ui/Alert";
import { Button } from "../ui/Button";
import { CopyBlock } from "../ui/CopyBlock";
import { Disclosure } from "../ui/Disclosure";
import { Facts, GroupFootnote, GroupHeader } from "../ui/Group";
import { emitting } from "./common";
import type { ScreenContext, ScreenView } from "./types";

type Diagnostics = Extract<ScreenModel, { screen: "diagnostics" }>;

/**
 * diagnostics (§11), from More: what the info is for, a summary, the full text collapsed.
 * Save to file is the main action, Copy the secondary; nothing is sent. Back returns to the
 * screen before.
 */
export function DiagnosticsScreen(model: Diagnostics, ctx: ScreenContext): ScreenView {
  const d = ctx.copy.diagnostics;
  const collecting = model.state === "collecting";
  return {
    step: null,
    title: d.title,
    lead: d.lead,
    status: collecting
      ? { text: d.collecting, busy: true }
      : model.state === "copied"
        ? { text: d.copied, busy: false }
        : undefined,
    content: (
      <>
        {model.state === "saved" ? (
          <Alert tone="success" title={d.saved}>
            {model.savedPath ? (
              <p className="font-mono text-label break-all text-fg-2" lang="en">
                {model.savedPath}
              </p>
            ) : null}
            <div className="mt-1">
              <Button size="sm" onClick={() => ctx.emit("show_file")}>
                <FolderOpen size={14} aria-hidden="true" />
                {ctx.copy.actions.showInFolder}
              </Button>
            </div>
          </Alert>
        ) : null}
        {model.state === "failed" ? (
          <Alert tone="danger" live>
            <p>{d.failed}</p>
          </Alert>
        ) : null}
        {model.summary.length > 0 ? (
          <section aria-labelledby="diagnostics-summary">
            <GroupHeader id="diagnostics-summary">{d.summaryTitle}</GroupHeader>
            <Facts items={model.summary} />
            <GroupFootnote icon={<Lock size={14} />}>{d.contents}</GroupFootnote>
          </section>
        ) : (
          <GroupFootnote icon={<Lock size={14} />}>{d.contents}</GroupFootnote>
        )}
        {model.text ? (
          <Disclosure summary={d.fullText}>
            <CopyBlock text={model.text} label={d.fullText} variant="report" lang="en" />
          </Disclosure>
        ) : null}
      </>
    ),
    primary: { ...emitting(ctx, ctx.copy.actions.saveToFile, "save_diagnostics"), disabled: collecting },
    secondary: { ...emitting(ctx, ctx.copy.actions.copy, "copy_diagnostics"), disabled: collecting },
    back: emitting(ctx, ctx.copy.actions.back, "back"),
  };
}
