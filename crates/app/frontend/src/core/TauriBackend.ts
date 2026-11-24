// The `Backend` over the app's Tauri commands (crates/app/src/main.rs): each method is one
// `invoke`, with the command's name and arguments, and install and restore send their progress
// over a `Channel`. Every rejection reaches the flow as a `Failure`: a command's own failure as
// the command returned it, anything Tauri itself refuses (a command or argument it can't
// match) as `invalid_input`, and the rest as `internal`.

import { Channel, type InvokeArgs, invoke as tauriInvoke } from "@tauri-apps/api/core";

import { type Backend, isFailure, type Lang, type Platform } from "./backend";
import type {
  DiagnoseInput,
  DiagnoseOutput,
  Failure,
  InspectDriveOutput,
  InspectOutput,
  IssueLink,
  IssueLinkInput,
  MarkPicture,
  PhonePage,
  Progress,
  ShareKey,
  Shared,
  StageOutput,
} from "./types.gen";

export type Invoke = <T>(command: string, args?: InvokeArgs) => Promise<T>;
/** Makes the channel a command reports progress on; a test passes a stand-in. */
export type MakeChannel = (onMessage: (progress: Progress) => void) => Channel<Progress>;

const LANGS: readonly Lang[] = ["ka", "en"];
const PLATFORMS: readonly Platform[] = ["macos", "windows", "linux"];

/**
 * A rejection as the flow takes it. The app's commands reject with a `Failure`, which passes
 * unchanged. Tauri rejects with a string when it can't run the command as asked (`invalid args
 * …` for arguments it can't deserialize, `command … not found`): the frontend asked wrongly,
 * so `invalid_input`. Anything else (a thrown `Error`, a closed IPC) is `internal`.
 */
export function failureOf(error: unknown): Failure {
  if (isFailure(error)) return error;
  if (typeof error === "string") {
    const refused = /^(invalid args|command .* not found)/.test(error);
    return { code: refused ? "invalid_input" : "internal", message: error };
  }
  if (error instanceof Error) return { code: "internal", message: error.message };
  return { code: "internal", message: String(error) };
}

export class TauriBackend implements Backend {
  constructor(
    private readonly invoke: Invoke = tauriInvoke,
    private readonly channel: MakeChannel = (onMessage) => new Channel<Progress>(onMessage),
  ) {}

  /** `command` with `args`; rejects with a `Failure` only. */
  private async call<T>(command: string, args?: InvokeArgs): Promise<T> {
    try {
      return await this.invoke<T>(command, args);
    } catch (error) {
      throw failureOf(error);
    }
  }

  private stage(command: "install" | "restore_stock", onProgress?: (p: Progress) => void): Promise<StageOutput> {
    const onMessage = onProgress ?? (() => undefined);
    return this.call<StageOutput>(command, { onProgress: this.channel(onMessage) });
  }

  inspect(): Promise<InspectOutput> {
    return this.call("inspect");
  }

  inspectDrive(): Promise<InspectDriveOutput> {
    return this.call("inspect_drive");
  }

  install(onProgress?: (p: Progress) => void): Promise<StageOutput> {
    return this.stage("install", onProgress);
  }

  restoreStock(onProgress?: (p: Progress) => void): Promise<StageOutput> {
    return this.stage("restore_stock", onProgress);
  }

  issueLink(input: IssueLinkInput): Promise<IssueLink> {
    return this.call("issue_link", { input });
  }

  shareReport(input: IssueLinkInput): Promise<Shared> {
    return this.call("share_report", { input });
  }

  openIssue(issue: number): Promise<string> {
    return this.call("open_issue", { issue });
  }

  openWatchList(): Promise<string> {
    return this.call("open_watch_list");
  }

  phonePage(language: Lang): Promise<PhonePage> {
    return this.call("phone_page", { language });
  }

  testMarks(): Promise<MarkPicture[]> {
    return this.call("test_marks");
  }

  async platform(): Promise<Platform> {
    const name = await this.call<string>("platform");
    const platform = PLATFORMS.find((p) => p === name);
    if (!platform) throw failureOf(new Error(`unknown platform ${JSON.stringify(name)}`));
    return platform;
  }

  demoMode(): Promise<boolean> {
    return this.call("demo_mode");
  }

  /** The saved language; null for none, or for one this frontend has no words for. */
  async savedLanguage(): Promise<Lang | null> {
    const saved = await this.call<string | null>("saved_language");
    return LANGS.find((lang) => lang === saved) ?? null;
  }

  saveLanguage(language: Lang): Promise<void> {
    return this.call("save_language", { language });
  }

  sharedBefore(key: ShareKey): Promise<boolean> {
    return this.call("shared_before", { key });
  }

  markShared(key: ShareKey): Promise<void> {
    return this.call("mark_shared", { key });
  }

  diagnose(input: DiagnoseInput): Promise<DiagnoseOutput> {
    return this.call("diagnose", { input });
  }

  saveDiagnostics(text: string): Promise<string> {
    return this.call("save_diagnostics", { text });
  }

  copyText(text: string): Promise<void> {
    return this.call("copy_text", { text });
  }

  revealFile(path: string): Promise<void> {
    return this.call("reveal_file", { path });
  }
}
