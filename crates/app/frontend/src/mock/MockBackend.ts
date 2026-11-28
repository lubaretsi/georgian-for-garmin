// A `Backend` over a simulated watch, for Storybook and the tests. It never opens a device.
//
// The watch is a small physical model: connected over MTP, as a USB drive, or unplugged; the
// font it uses and the one waiting for a restart (each a real fixture step, see watches.ts);
// a restart (the USB connection drops for a moment) imports what waits; a program holding the
// watch; a write that fails and leaves files behind; what the next test notification shows.

import type { Backend, Lang, Platform } from "../core/backend";
import type {
  CheckUpdateOutput,
  DiagnoseInput,
  DiagnoseOutput,
  Failure,
  HolderKind,
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
} from "../core/types.gen";
import { localTime, shareKeyId } from "../flow/state";
import type { Result } from "../model/screens";
import { failure, fixtureFile, output } from "./fixtures";
import { PHONE_PAGE_QR } from "./phone-page";
import { busyStep, type Step, WATCH_DEFS, type WatchDef, type WatchId } from "./watches";

/** `garmin`: connected in Garmin mode, neither a watch nor a drive until switched (§4.2). */
export type Connection = "mtp" | "drive" | "garmin" | "unplugged";
/** How the app's next switch to MTP goes. */
export type Switching = "switches" | "timed_out" | "locked" | "unsupported";

/** What the simulator panel shows and changes. */
export type Simulation = {
  watch: WatchId;
  connection: Connection;
  /** The connection to come back to after a restart. */
  restartingTo: Connection | null;
  /** What `inspect` reads now, and what the next restart imports. */
  current: Step | null;
  pending: Step | null;
  holder: HolderKind | null;
  /** A connect problem every read fails with (e.g. Linux permission denied). */
  problem: Step | null;
  failNextWrite: boolean;
  /** How the next switch from Garmin mode to MTP goes. */
  switching: Switching;
  /** The next restart doesn't import what waits. */
  skipNextImport: boolean;
  /** A failed write left files: restore uses the failure scenario's fixtures. */
  afterFailedWrite: boolean;
  /** What the next test notification shows on the watch; `auto` follows the font. */
  phoneResult: Result | "auto";
  /** Every link the app opened, newest last. */
  opened: string[];
  /** Diagnostic files saved, newest last, and the one last shown in its folder. */
  saved: string[];
  shownFile: string | null;
  /** The last text copied to the clipboard (the mock keeps it; nothing is copied). */
  copied: string | null;
  /** How many times the app asked for the latest release. */
  updateChecks: number;
};

export type MockOptions = {
  watch: WatchId;
  connection?: Connection;
  platform?: Platform;
  holder?: HolderKind | null;
  problem?: Step | null;
  /** Keys already shared (`sharedBefore` answers true). */
  sharedBefore?: ShareKey[];
  marks?: MarkPicture[];
  /** Divides every delay: faster in the story tests. */
  speed?: number;
  /** How the switch from Garmin mode to MTP goes; it switches by default. */
  switching?: Switching;
  /** What `check_update` answers: the latest release; without it the check fails, as offline. */
  update?: CheckUpdateOutput;
};

const INSPECT_MS = 120;
const PART_MS = 350;
const RESTART_MS = 3000;
const NEW_ISSUE = "https://github.com/lubaretsi/georgian-for-garmin/issues/new";
/** The README's watch list, as the app's `open_watch_list` opens it. */
const WATCH_LIST = "https://github.com/lubaretsi/georgian-for-garmin#supported-watches";
/** The mock app's version (the flow stories show it in More › About). */
export const MOCK_VERSION = "0.3.4";

/** `check_update`'s answer when `version` is published, newer than `MOCK_VERSION`. */
export function newerRelease(version: string): CheckUpdateOutput {
  return {
    current: MOCK_VERSION,
    latest: version,
    newer: true,
    url: `https://github.com/lubaretsi/georgian-for-garmin/releases/tag/v${version}`,
  };
}

const NO_DEVICE: Step = "connect-problems/inspect-no_device";
const DIAGNOSE_MS = 400;
/** The switch to MTP: a few seconds on a real watch. */
const SWITCH_MS = 1500;
/** The fixtures of a watch in Garmin mode and of the switch's outcomes. */
const GARMIN_MODE: Step = "garmin-mode/inspect-garmin_mode";
const GARMIN_MODE_WINDOWS: Step = "garmin-mode/inspect-garmin_mode-windows";
const SWITCHED: Record<Switching, Step> = {
  switches: "garmin-mode/switch_to_mtp-switched",
  timed_out: "garmin-mode/switch_to_mtp-timed_out",
  locked: "garmin-mode/switch_to_mtp-device_locked",
  unsupported: "garmin-mode/switch_to_mtp-unsupported_device",
};
/** The USB product ID of a Garmin device in Garmin mode. */
const GARMIN_MODE_PRODUCT_ID = 0x0003;
/** The Venu 2's USB product ID (0x4E77), the one product ID the reports show. */
const VENU2_PRODUCT_ID = 0x4e77;
const SYSTEMS: Record<Platform, DiagnoseOutput["system"]> = {
  macos: { os: "Mac OS", os_version: "15.6.1", arch: "aarch64" },
  windows: { os: "Windows", os_version: "11 (26100)", arch: "x86_64" },
  linux: { os: "Ubuntu", os_version: "24.04", arch: "x86_64" },
};

/** "20261124-031405", the saved file's date and time. */
function stamp(date: Date): string {
  return localTime(date).replaceAll("-", "").replaceAll(":", "").replace("T", "-");
}

/**
 * The mock's stand-in for `diagnose`'s `text`; the core's is the real format. Like it, it
 * starts by saying it holds no serial number or watch ID.
 */
function diagnosticsText(d: Omit<DiagnoseOutput, "text">): string {
  const hex = (n: number) => `0x${n.toString(16).toUpperCase().padStart(4, "0")}`;
  const lines = [
    "Georgian for Garmin diagnostic info. It contains no serial number or watch ID.",
    `app ${d.app.version} (${d.app.build})`,
    `system ${d.system.os} ${d.system.os_version} ${d.system.arch}`,
    d.usb.devices.length ? "usb" : "usb no Garmin device",
    ...d.usb.devices.map(
      (u) =>
        `  ${hex(u.product_id)} ${u.mtp ? "mtp" : u.product_id === GARMIN_MODE_PRODUCT_ID ? "garmin mode" : "mass storage"}` +
        (u.holder ? ` held by ${u.holder.kind}${u.holder.name ? ` "${u.holder.name}"` : ""}` : ""),
    ),
    ...(d.usb.note ? [`  note: ${d.usb.note}`] : []),
    `connection ${d.connection.ok ? "ok" : `${d.connection.code}: ${d.connection.message}`}`,
    `drives ${d.drives.volumes.map((v) => `${v.volume} ${v.model ?? "?"} ${v.software_version ?? ""}`.trim()).join(", ") || "none"}`,
    d.watch ? `watch\n${d.watch.text}` : "watch none",
    `events ${d.events.length}`,
    ...d.events.map((e) => `  ${e.time} ${e.kind} ${e.detail}${e.model ? ` (${e.model})` : ""}`),
  ];
  return `${lines.join("\n")}\n`;
}

/** `<scenario>/<step>` → its two parts. */
function split(step: Step): [string, string] {
  const slash = step.indexOf("/");
  return [step.slice(0, slash), step.slice(slash + 1)];
}

export class MockBackend implements Backend {
  private sim: Simulation;
  private readonly listeners = new Set<() => void>();
  private readonly shared: Set<string>;
  private readonly platformName: Platform;
  private readonly marks: MarkPicture[];
  private readonly speed: number;
  private readonly update: CheckUpdateOutput | null;
  private language: Lang | null = null;

  constructor(options: MockOptions) {
    const def: WatchDef = WATCH_DEFS[options.watch];
    this.sim = {
      watch: options.watch,
      connection: options.connection ?? "mtp",
      restartingTo: null,
      current: def.start,
      pending: def.startPending ?? null,
      holder: options.holder ?? null,
      problem: options.problem ?? null,
      failNextWrite: false,
      switching: options.switching ?? "switches",
      skipNextImport: false,
      afterFailedWrite: false,
      phoneResult: "auto",
      opened: [],
      saved: [],
      shownFile: null,
      copied: null,
      updateChecks: 0,
    };
    this.shared = new Set((options.sharedBefore ?? []).map(shareKeyId));
    this.platformName = options.platform ?? "macos";
    this.marks = options.marks ?? [];
    this.speed = options.speed ?? 1;
    this.update = options.update ?? null;
  }

  get def(): WatchDef {
    return WATCH_DEFS[this.sim.watch];
  }

  // The simulator's side.

  simulation = (): Simulation => this.sim;

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  private change(patch: Partial<Simulation>): void {
    this.sim = { ...this.sim, ...patch };
    for (const listener of this.listeners) listener();
  }

  plugIn(connection: "mtp" | "drive" | "garmin"): void {
    this.change({ connection, restartingTo: null });
  }

  unplug(): void {
    this.change({ connection: "unplugged", restartingTo: null });
  }

  /** The watch in Garmin mode asks "Use MTP?", and MTP is chosen on it. */
  chooseMtp(): void {
    if (this.sim.connection === "garmin") this.change({ connection: "mtp" });
  }

  setSwitching(switching: Switching): void {
    this.change({ switching });
  }

  /** Restarts the watch: it imports what waits, and a connected watch drops off USB meanwhile. */
  restart(): void {
    const { pending, skipNextImport, connection } = this.sim;
    const imported = skipNextImport ? {} : { current: pending ?? this.sim.current, pending: null };
    if (connection === "unplugged") {
      this.change({ ...imported, skipNextImport: false });
      return;
    }
    this.change({ ...imported, skipNextImport: false, connection: "unplugged", restartingTo: connection });
    setTimeout(() => {
      if (this.sim.restartingTo) this.change({ connection: this.sim.restartingTo, restartingTo: null });
    }, RESTART_MS / this.speed);
  }

  hold(holder: HolderKind | null): void {
    this.change({ holder });
  }

  setProblem(problem: Step | null): void {
    this.change({ problem });
  }

  failNextWrite(): void {
    this.change({ failNextWrite: true });
  }

  skipNextImport(): void {
    this.change({ skipNextImport: true });
  }

  setPhoneResult(phoneResult: Result | "auto"): void {
    this.change({ phoneResult });
  }

  /** What the watch shows for the next test notification. */
  notification(): Result {
    if (this.sim.phoneResult !== "auto") return this.sim.phoneResult;
    const current = this.sim.current;
    return current && this.inspectAt(current).state === "installed" ? "georgian" : "boxes";
  }

  // The backend's side.

  private wait(ms: number): Promise<void> {
    const { promise, resolve } = Promise.withResolvers<void>();
    setTimeout(resolve, ms / this.speed);
    return promise;
  }

  private inspectAt(step: Step): InspectOutput {
    return output(...split(step), "inspect");
  }

  private failureAt(step: Step): Failure {
    return failure(...split(step));
  }

  /** What a read finds now: the watch's state step, or the failure. */
  private read(): { ok: Step } | { failed: Failure } {
    const { connection, problem, holder, current } = this.sim;
    if (connection === "garmin" && this.def.mtp) {
      return { failed: this.failureAt(this.platformName === "windows" ? GARMIN_MODE_WINDOWS : GARMIN_MODE) };
    }
    if (connection !== "mtp" || !this.def.mtp || !current) return { failed: this.failureAt(NO_DEVICE) };
    if (problem) return { failed: this.failureAt(problem) };
    if (holder) return { failed: this.failureAt(busyStep(holder, this.platformName)) };
    return { ok: current };
  }

  async inspect(): Promise<InspectOutput> {
    await this.wait(INSPECT_MS);
    const read = this.read();
    if ("failed" in read) throw read.failed;
    return this.inspectAt(read.ok);
  }

  /** The core's `switch_to_mtp` on this simulated watch: Windows can't, as there (§4.2). */
  async switchToMtp(): Promise<SwitchOutput> {
    await this.wait(SWITCH_MS);
    if (this.sim.connection !== "garmin") {
      throw { code: "no_device", message: "No Garmin watch in Garmin mode was found." } satisfies Failure;
    }
    if (this.platformName === "windows") throw this.failureAt(GARMIN_MODE_WINDOWS);
    const [scenario, step] = split(SWITCHED[this.sim.switching]);
    if (fixtureFile(scenario, step).error) throw failure(scenario, step);
    const result = output(scenario, step, "switch_to_mtp");
    if (result.status === "switched") this.change({ connection: "mtp" });
    return result;
  }

  /** The core's `diagnose` (§11), as it would answer on this simulated computer. */
  async diagnose(input: DiagnoseInput): Promise<DiagnoseOutput> {
    await this.wait(DIAGNOSE_MS);
    const read = this.read();
    const watch = "ok" in read ? this.inspectAt(read.ok).report : null;
    const failed = "failed" in read ? read.failed : null;
    const busy = failed?.busy ?? null;
    const { connection } = this.sim;
    const drive = connection === "drive" && this.def.drive ? output(...split(this.def.drive), "inspect_drive") : null;
    const found: Omit<DiagnoseOutput, "text"> = {
      app: { version: "0.3.4", build: "release" },
      system: SYSTEMS[this.platformName],
      usb: {
        listed: true,
        note: this.platformName === "windows" ? "Windows shows only the devices it set up for MTP." : null,
        devices:
          connection === "unplugged"
            ? []
            : [
                {
                  product_id: busy?.product_id ?? (connection === "garmin" ? GARMIN_MODE_PRODUCT_ID : VENU2_PRODUCT_ID),
                  product_name: busy?.product_name ?? null,
                  mtp: connection === "mtp",
                  interfaces:
                    this.platformName === "windows"
                      ? []
                      : [
                          connection === "mtp"
                            ? { class: 255, subclass: 255, protocol: 0 }
                            : connection === "garmin"
                              ? { class: 255, subclass: 0, protocol: 0 }
                              : { class: 8, subclass: 6, protocol: 80 },
                        ],
                  holder: busy?.holder ?? null,
                },
              ],
      },
      connection: failed
        ? { ok: false, code: failed.code, message: failed.message, busy }
        : { ok: true, code: null, message: null, busy: null },
      drives: {
        listed: true,
        note: null,
        volumes: drive
          ? [
              {
                volume: this.platformName === "windows" ? "E:" : "GARMIN",
                model: drive.device.model,
                part_number: drive.device.part_number,
                software_version: drive.device.software_version,
                error: null,
              },
            ]
          : [],
      },
      watch,
      events: input.events ?? [],
    };
    return { ...found, text: diagnosticsText(found) };
  }

  async saveDiagnostics(text: string): Promise<string> {
    await this.wait(INSPECT_MS);
    const name = `georgian-for-garmin-diagnostics-${stamp(new Date())}.txt`;
    const path = this.platformName === "windows" ? `%USERPROFILE%\\Downloads\\${name}` : `~/Downloads/${name}`;
    this.change({ saved: [...this.sim.saved, `${path} (${text.length} characters)`] });
    return path;
  }

  async copyText(text: string): Promise<void> {
    this.change({ copied: text });
  }

  async revealFile(path: string): Promise<void> {
    this.change({ shownFile: path });
  }

  async inspectDrive(): Promise<InspectDriveOutput> {
    await this.wait(INSPECT_MS);
    const drive = this.def.drive;
    if (this.sim.connection !== "drive" || !drive) throw this.failureAt(NO_DEVICE);
    return output(...split(drive), "inspect_drive");
  }

  private async write(
    plan: { output: Step; staged: Step; imported: Step } | { failure: Step } | undefined,
    onProgress?: (p: Progress) => void,
  ): Promise<StageOutput> {
    await this.wait(INSPECT_MS);
    if (this.sim.connection !== "mtp") throw this.failureAt(NO_DEVICE);
    if (this.sim.holder) throw this.failureAt(busyStep(this.sim.holder, this.platformName));
    if (!plan) throw this.failureAt("predicted/install");
    if ("failure" in plan) throw this.failureAt(plan.failure);
    const [scenario, name] = split(plan.output);
    const stage = output(scenario, name, fixtureFile(scenario, name).operation === "install" ? "install" : "restore_stock");
    const total = stage.packs.reduce((n, pack) => n + pack.parts.length, 0);
    for (let done = 1; done <= total; done++) {
      await this.wait(PART_MS);
      const failed = this.def.failedWrite;
      if (this.sim.failNextWrite && failed && done === 2) {
        this.change({ failNextWrite: false, afterFailedWrite: true, current: failed.after, pending: null });
        throw this.failureAt(failed.failure);
      }
      onProgress?.({ done, total });
    }
    this.change({ current: plan.staged, pending: plan.imported });
    return stage;
  }

  install(onProgress?: (p: Progress) => void): Promise<StageOutput> {
    return this.write(this.def.install, onProgress);
  }

  restoreStock(onProgress?: (p: Progress) => void): Promise<StageOutput> {
    const failed = this.def.failedWrite;
    const plan = this.sim.afterFailedWrite && failed ? failed.restore : this.def.restore;
    return this.write(plan, onProgress).then((stage) => {
      this.change({ afterFailedWrite: false });
      return stage;
    });
  }

  async issueLink(input: IssueLinkInput): Promise<IssueLink> {
    await this.wait(INSPECT_MS);
    // A connect problem's report: the error alone, one fixture per problem without a user fix.
    if (input.error) return output("connect-problems", `issue_link-${input.error.code}`, "issue_link");
    if (!input.report) return output("busy", "issue_link-problem", "issue_link");
    const key = input.outcome?.read_before && this.def.links.read_before ? "read_before" : input.answers?.result;
    const step = this.def.links[key ?? "font_list"] ?? Object.values(this.def.links)[0];
    if (step) return output(...split(step), "issue_link");
    // No fixture for this watch: the report as inspect gave it.
    return { title: input.report.title, url: NEW_ISSUE, report_in_url: false, report_text: input.report.text, fields: [] };
  }

  async shareReport(input: IssueLinkInput): Promise<Shared> {
    const link = await this.issueLink(input);
    this.change({ opened: [...this.sim.opened, link.url] });
    return { complete: link.report_in_url, url: link.url };
  }

  async openIssue(issue: number): Promise<string> {
    const url = `https://github.com/lubaretsi/georgian-for-garmin/issues/${issue}`;
    this.change({ opened: [...this.sim.opened, url] });
    return url;
  }

  async checkUpdate(): Promise<CheckUpdateOutput> {
    this.change({ updateChecks: this.sim.updateChecks + 1 });
    await this.wait(INSPECT_MS);
    if (!this.update) throw { code: "network_error", message: "Mock: offline." } satisfies Failure;
    return this.update;
  }

  /** Opens the newer release's page, as the app does: only after a check found one. */
  async openUpdate(): Promise<string> {
    if (!this.update?.newer) throw { code: "invalid_input", message: "no newer version was found" } satisfies Failure;
    this.change({ opened: [...this.sim.opened, this.update.url] });
    return this.update.url;
  }

  async openWatchList(): Promise<string> {
    this.change({ opened: [...this.sim.opened, WATCH_LIST] });
    return WATCH_LIST;
  }

  async phonePage(language: Lang): Promise<PhonePage> {
    return PHONE_PAGE_QR[language];
  }

  async testMarks(): Promise<MarkPicture[]> {
    return this.marks;
  }

  async platform(): Promise<Platform> {
    return this.platformName;
  }

  async demoMode(): Promise<boolean> {
    return false;
  }

  async savedLanguage(): Promise<Lang | null> {
    return this.language;
  }

  async saveLanguage(language: Lang): Promise<void> {
    this.language = language;
  }

  async sharedBefore(key: ShareKey): Promise<boolean> {
    return this.shared.has(shareKeyId(key));
  }

  async markShared(key: ShareKey): Promise<void> {
    this.shared.add(shareKeyId(key));
  }
}
