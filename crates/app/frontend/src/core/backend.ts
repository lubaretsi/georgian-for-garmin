// What the UI asks of the app: the Tauri commands (crates/app/src/main.rs), with the same names
// and the core's output unchanged (contract §3.1). `TauriBackend` implements it over `invoke`;
// Storybook and the tests use `MockBackend` (src/mock/). Every shape is generated from the Rust
// types (types.gen.ts).

import type {
  CheckUpdateOutput,
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
  SwitchOutput,
} from "./types.gen";

export interface Backend {
  /** Rejects with `Failure`. */
  inspect(): Promise<InspectOutput>;
  /** The core's `switch_to_mtp`: a watch in Garmin mode back to MTP, as Garmin Express does (§4.2). */
  switchToMtp(): Promise<SwitchOutput>;
  inspectDrive(): Promise<InspectDriveOutput>;
  install(onProgress?: (p: Progress) => void): Promise<StageOutput>;
  restoreStock(onProgress?: (p: Progress) => void): Promise<StageOutput>;
  issueLink(input: IssueLinkInput): Promise<IssueLink>;
  /** Opens the browser (not in the demo build or the mock). */
  shareReport(input: IssueLinkInput): Promise<Shared>;
  openIssue(issue: number): Promise<string>;
  /** Opens the README's watch list; no arbitrary URLs. */
  openWatchList(): Promise<string>;
  /**
   * The core's `check_update` (§12): the latest release and whether it is newer. The app asks
   * GitHub once per run; a second call gets the first one's result.
   */
  checkUpdate(): Promise<CheckUpdateOutput>;
  /** Opens the page of the newer release `checkUpdate` found; no arbitrary URLs. */
  openUpdate(): Promise<string>;
  phonePage(language: Lang): Promise<PhonePage>;
  /** Empty in stable builds. */
  testMarks(): Promise<MarkPicture[]>;
  platform(): Promise<Platform>;
  demoMode(): Promise<boolean>;
  savedLanguage(): Promise<Lang | null>;
  saveLanguage(language: Lang): Promise<void>;
  /** Whether this model, software and result were shared from this app before (app settings). */
  sharedBefore(key: ShareKey): Promise<boolean>;
  markShared(key: ShareKey): Promise<void>;
  /** The core's `diagnose` (§11); never rejects for device problems: they are data. */
  diagnose(input: DiagnoseInput): Promise<DiagnoseOutput>;
  /** Writes `text` to a new file in Downloads and returns its path. */
  saveDiagnostics(text: string): Promise<string>;
  /** The system clipboard. */
  copyText(text: string): Promise<void>;
  /** Shows a file `saveDiagnostics` wrote in Finder, Explorer or the file manager. */
  revealFile(path: string): Promise<void>;
}

export type Lang = "ka" | "en";
export type Platform = "macos" | "windows" | "linux";

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
