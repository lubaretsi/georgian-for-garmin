// TauriBackend over a mocked `invoke`: each method calls its command with the command's
// argument names, progress arrives over the channel, and every rejection becomes a `Failure`.

import type { Channel, InvokeArgs } from "@tauri-apps/api/core";
import { describe, expect, test } from "vitest";

import { failure } from "../mock/fixtures";
import type { Backend } from "./backend";
import { failureOf, type Invoke, type MakeChannel, TauriBackend } from "./TauriBackend";
import type { Progress } from "./types.gen";

type Call = { command: string; args?: InvokeArgs };

/** A backend whose `invoke` records each call and answers with `answer`. */
function backend(answer: (call: Call) => unknown) {
  const calls: Call[] = [];
  const invoke = (async (command: string, args?: InvokeArgs) => {
    const call = { command, ...(args && { args }) };
    calls.push(call);
    return answer(call);
  }) as Invoke;
  // A stand-in for Tauri's Channel: the mock invoke calls its handler directly.
  const channel: MakeChannel = (onMessage) => ({ onmessage: onMessage }) as unknown as Channel<Progress>;
  return { calls, tauri: new TauriBackend(invoke, channel) };
}

const KEY = { modelPart: "006-B3703-00", software: "19.05", result: "boxes" } as const;

describe("TauriBackend", () => {
  test("each method calls its command with the command's arguments", async () => {
    const { calls, tauri } = backend(({ command }) => (command === "platform" ? "linux" : null));
    const input = { error: { code: "internal" as const, message: "x" } };
    const each: [keyof Backend, () => Promise<unknown>, Call][] = [
      ["inspect", () => tauri.inspect(), { command: "inspect" }],
      ["switchToMtp", () => tauri.switchToMtp(), { command: "switch_to_mtp" }],
      ["inspectDrive", () => tauri.inspectDrive(), { command: "inspect_drive" }],
      ["issueLink", () => tauri.issueLink(input), { command: "issue_link", args: { input } }],
      ["shareReport", () => tauri.shareReport(input), { command: "share_report", args: { input } }],
      ["openIssue", () => tauri.openIssue(33), { command: "open_issue", args: { issue: 33 } }],
      ["openWatchList", () => tauri.openWatchList(), { command: "open_watch_list" }],
      ["checkUpdate", () => tauri.checkUpdate(), { command: "check_update" }],
      ["openUpdate", () => tauri.openUpdate(), { command: "open_update" }],
      ["phonePage", () => tauri.phonePage("en"), { command: "phone_page", args: { language: "en" } }],
      ["testMarks", () => tauri.testMarks(), { command: "test_marks" }],
      ["platform", () => tauri.platform(), { command: "platform" }],
      ["demoMode", () => tauri.demoMode(), { command: "demo_mode" }],
      ["savedLanguage", () => tauri.savedLanguage(), { command: "saved_language" }],
      ["saveLanguage", () => tauri.saveLanguage("en"), { command: "save_language", args: { language: "en" } }],
      ["sharedBefore", () => tauri.sharedBefore(KEY), { command: "shared_before", args: { key: KEY } }],
      ["markShared", () => tauri.markShared(KEY), { command: "mark_shared", args: { key: KEY } }],
      ["diagnose", () => tauri.diagnose({ events: [] }), { command: "diagnose", args: { input: { events: [] } } }],
      ["saveDiagnostics", () => tauri.saveDiagnostics("t"), { command: "save_diagnostics", args: { text: "t" } }],
      ["copyText", () => tauri.copyText("t"), { command: "copy_text", args: { text: "t" } }],
      ["revealFile", () => tauri.revealFile("/p"), { command: "reveal_file", args: { path: "/p" } }],
    ];
    for (const [method, run, expected] of each) {
      calls.length = 0;
      await run();
      expect(calls, method).toEqual([expected]);
    }
  });

  test("install and restore report progress over their channel", async () => {
    const { calls, tauri } = backend(({ args }) => {
      const channel = (args as Record<string, unknown>).onProgress as { onmessage: (p: Progress) => void };
      channel.onmessage({ done: 0, total: 4 });
      channel.onmessage({ done: 4, total: 4 });
      return { status: "staged_verified" };
    });
    for (const run of [(p: (s: Progress) => void) => tauri.install(p), (p: (s: Progress) => void) => tauri.restoreStock(p)]) {
      const seen: Progress[] = [];
      await run((step) => seen.push(step));
      expect(seen).toEqual([
        { done: 0, total: 4 },
        { done: 4, total: 4 },
      ]);
    }
    expect(calls.map((c) => c.command)).toEqual(["install", "restore_stock"]);
    // Without a listener the channel is still passed, and its messages go nowhere.
    await tauri.install();
  });

  test("a language or platform the frontend doesn't know isn't passed on", async () => {
    expect(await backend(() => "fr").tauri.savedLanguage()).toBeNull();
    expect(await backend(() => null).tauri.savedLanguage()).toBeNull();
    expect(await backend(() => "ka").tauri.savedLanguage()).toBe("ka");
    await expect(backend(() => "beos").tauri.platform()).rejects.toMatchObject({ code: "internal" });
  });
});

describe("every rejection is a Failure", () => {
  test("a command's own failure passes unchanged, busy and rollback included", async () => {
    for (const command of [failure("busy", "inspect-garmin_express-macos"), failure("errors", "install-verification_failed")]) {
      const { tauri } = backend(() => {
        throw command;
      });
      await expect(tauri.inspect()).rejects.toEqual(command);
    }
  });

  test("what Tauri refuses is invalid_input; anything else is internal", () => {
    expect(failureOf("invalid args `key` for command `shared_before`: missing field `result`")).toEqual({
      code: "invalid_input",
      message: "invalid args `key` for command `shared_before`: missing field `result`",
    });
    expect(failureOf("command nope not found").code).toBe("invalid_input");
    expect(failureOf("the IPC channel closed")).toEqual({ code: "internal", message: "the IPC channel closed" });
    expect(failureOf(new TypeError("window.__TAURI_INTERNALS__ is undefined"))).toEqual({
      code: "internal",
      message: "window.__TAURI_INTERNALS__ is undefined",
    });
    expect(failureOf(undefined)).toEqual({ code: "internal", message: "undefined" });
    expect(failureOf({ code: 7 })).toEqual({ code: "internal", message: "[object Object]" });
  });
});
