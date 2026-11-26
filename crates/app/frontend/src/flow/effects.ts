// Runs the flow's effects against a `Backend` and feeds every result back as an event.

import { type Backend, isFailure } from "../core/backend";
import type { Failure, Progress } from "../core/types.gen";
import type { Effect, FlowEvent } from "./state";

/** A rejection as the flow takes it: the backend's `Failure`, or the app's own fault. */
export function toFailure(error: unknown): Failure {
  if (isFailure(error)) return error;
  return { code: "internal", message: error instanceof Error ? error.message : String(error) };
}

export class EffectRunner {
  private timers = new Set<ReturnType<typeof setTimeout>>();
  private stopped = false;

  constructor(
    private readonly backend: Backend,
    private readonly dispatch: (event: FlowEvent) => void,
    /** Divides every delay: the story tests run the flow faster. */
    private readonly speed = 1,
  ) {}

  run(effects: readonly Effect[]): void {
    for (const effect of effects) this.one(effect);
  }

  /** Stops every timer; results still in flight are dropped. */
  stop(): void {
    this.stopped = true;
    for (const timer of this.timers) clearTimeout(timer);
    this.timers.clear();
  }

  resume(): void {
    this.stopped = false;
  }

  private send(event: FlowEvent): void {
    if (!this.stopped) this.dispatch(event);
  }

  private later(delay: number, task: () => void): void {
    const timer = setTimeout(() => {
      this.timers.delete(timer);
      if (!this.stopped) task();
    }, delay / this.speed);
  }

  /** Calls `call` and sends its result, or its failure, as an event. */
  private settle<T>(call: () => Promise<T>, ok: (value: T) => FlowEvent, failed?: (failure: Failure) => FlowEvent) {
    call().then(
      (value) => this.send(ok(value)),
      (error: unknown) => {
        if (failed) this.send(failed(toFailure(error)));
      },
    );
  }

  private one(effect: Effect): void {
    const b = this.backend;
    switch (effect.kind) {
      case "inspect": {
        const { token, slowAfter } = effect;
        this.later(effect.delay, () => {
          let answered = false;
          if (slowAfter !== null) {
            this.later(slowAfter, () => {
              if (!answered) this.send({ type: "inspect_slow", token });
            });
          }
          this.settle(
            () => b.inspect().finally(() => (answered = true)),
            (output) => ({ type: "inspected", token, output }),
            (failure) => ({ type: "inspect_failed", token, failure }),
          );
        });
        return;
      }
      case "inspect_drive": {
        const { token } = effect;
        this.settle(
          () => b.inspectDrive(),
          (output) => ({ type: "drive_inspected", token, output }),
          (failure) => ({ type: "drive_failed", token, failure }),
        );
        return;
      }
      case "switch_to_mtp":
        this.settle(
          () => b.switchToMtp(),
          (output) => ({ type: "switched", output }),
          (failure) => ({ type: "switch_failed", failure }),
        );
        return;
      case "help_timer": {
        const { token } = effect;
        this.later(effect.delay, () => this.send({ type: "help_due", token }));
        return;
      }
      case "starting_timer": {
        const { token } = effect;
        this.later(effect.delay, () => this.send({ type: "started_up", token }));
        return;
      }
      case "run": {
        const { op } = effect;
        const onProgress = (progress: Progress) => this.send({ type: "progress", progress });
        this.settle(
          () => (op === "install" ? b.install(onProgress) : b.restoreStock(onProgress)),
          (output) => ({ type: "op_done", op, output }),
          (failure) => ({ type: "op_failed", op, failure }),
        );
        return;
      }
      case "issue_link":
        this.settle(
          () => b.issueLink(effect.input),
          (link) => ({ type: "link_ready", link }),
          (failure) => ({ type: "link_failed", failure }),
        );
        return;
      case "share_report":
        this.settle(
          () => b.shareReport(effect.input),
          (shared) => ({ type: "shared", shared }),
          (failure) => ({ type: "share_failed", failure }),
        );
        return;
      // Opening the browser and saving settings have nothing to show; a failure changes nothing.
      case "open_issue":
        b.openIssue(effect.issue).catch(() => undefined);
        return;
      case "open_watch_list":
        b.openWatchList().catch(() => undefined);
        return;
      case "mark_shared":
        b.markShared(effect.key).catch(() => undefined);
        return;
      case "save_language":
        b.saveLanguage(effect.lang).catch(() => undefined);
        return;
      case "phone_page": {
        const { lang } = effect;
        this.settle(
          () => b.phonePage(lang),
          (page) => ({ type: "phone_loaded", lang, page }),
        );
        return;
      }
      case "test_marks":
        this.settle(
          () => b.testMarks(),
          (marks) => ({ type: "marks_loaded", marks }),
        );
        return;
      case "diagnose": {
        const { events } = effect;
        this.settle(
          () => b.diagnose({ events }),
          (output) => ({ type: "diagnosed", output }),
          (failure) => ({ type: "diagnostics_failed", failure }),
        );
        return;
      }
      case "save_diagnostics": {
        const { text } = effect;
        this.settle(
          () => b.saveDiagnostics(text),
          (path) => ({ type: "diagnostics_saved", path }),
          (failure) => ({ type: "diagnostics_failed", failure }),
        );
        return;
      }
      case "copy_text": {
        const { text } = effect;
        this.settle(
          () => b.copyText(text),
          () => ({ type: "diagnostics_copied" }),
          (failure) => ({ type: "diagnostics_failed", failure }),
        );
        return;
      }
      case "reveal_file":
        b.revealFile(effect.path).catch(() => undefined);
        return;
      case "shared_before": {
        const { key } = effect;
        this.settle(
          () => b.sharedBefore(key),
          (value) => ({ type: "shared_before", key, value }),
        );
        return;
      }
    }
  }
}
