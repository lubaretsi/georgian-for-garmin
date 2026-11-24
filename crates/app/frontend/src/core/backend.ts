// What the UI asks of the app: today's Tauri commands (crates/app/src/main.rs), with the same
// names and the core's output unchanged (contract §3.1). Phase 2's `TauriBackend` implements it
// over `invoke`; Storybook and the tests use `MockBackend` (src/mock/).

import type { Result } from "../model/screens";
import type { IssueLinkRequest, MarkPicture, PhonePage, Shared } from "./app-types";
import type { DiagnoseInput, DiagnoseOutput, InspectDriveOutput, InspectOutput, IssueLink, StageOutput } from "./types.gen";

export interface Backend {
  /** Rejects with `Failure`. */
  inspect(): Promise<InspectOutput>;
  inspectDrive(): Promise<InspectDriveOutput>;
  install(onProgress?: (p: Progress) => void): Promise<StageOutput>;
  restoreStock(onProgress?: (p: Progress) => void): Promise<StageOutput>;
  issueLink(input: IssueLinkRequest): Promise<IssueLink>;
  /** Opens the browser (not in the demo build or the mock). */
  shareReport(input: IssueLinkRequest): Promise<Shared>;
  openIssue(issue: number): Promise<string>;
  /** New (phase 2): opens the README's watch list; no arbitrary URLs. */
  openWatchList(): Promise<string>;
  phonePage(language: Lang): Promise<PhonePage>;
  /** Empty in stable builds. */
  testMarks(): Promise<MarkPicture[]>;
  platform(): Promise<Platform>;
  demoMode(): Promise<boolean>;
  savedLanguage(): Promise<Lang | null>;
  saveLanguage(language: Lang): Promise<void>;
  /** New (phase 2: app settings). */
  sharedBefore(key: ShareKey): Promise<boolean>;
  /** New (phase 2: app settings). */
  markShared(key: ShareKey): Promise<void>;
  /** The core's `diagnose` (§11); never rejects for device problems: they are data. */
  diagnose(input: DiagnoseInput): Promise<DiagnoseOutput>;
  /** New (phase 2): writes `text` to Downloads and returns the path. */
  saveDiagnostics(text: string): Promise<string>;
  /** New (phase 2): the system clipboard. */
  copyText(text: string): Promise<void>;
  /** New (phase 2): shows the file in Finder, Explorer or the file manager. */
  revealFile(path: string): Promise<void>;
}

/** Phase 2 wires the core's progress. */
export type Progress = { done: number; total: number };
/** What a user already shared for a model and software; no unit ID. */
export type ShareKey = { modelPart: string; software: string; result: Result | "font_list" };
export type Lang = "ka" | "en";
export type Platform = "macos" | "windows" | "linux";
