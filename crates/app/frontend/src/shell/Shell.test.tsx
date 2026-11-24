import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { MoreModel, ScreenModel, WatchInfo } from "../model/screens";
import { Shell } from "./Shell";

const venu2: WatchInfo = { model: "Venu 2", modelPart: "006-B3703-00", software: "19.05", confirmedSoftware: ["19.05"] };
const more: MoreModel = { restorable: true, seen: null, reportText: null, version: "0.4.0" };
const install: ScreenModel = {
  screen: "watch",
  watch: venu2,
  verdict: { kind: "install", confidence: "confirmed" },
  issue: null,
  reportWanted: false,
  sharedBefore: false,
};
const working: ScreenModel = { screen: "working", op: "install", watch: venu2, progress: { done: 1, total: 4 } };

function setup(model: ScreenModel, lang: "ka" | "en" = "en") {
  const onAction = vi.fn();
  const onLanguage = vi.fn();
  const view = render(
    <Shell model={model} more={more} lang={lang} platform="macos" onAction={onAction} onLanguage={onLanguage} />,
  );
  const rerender = (next: ScreenModel) =>
    view.rerender(<Shell model={next} more={more} lang={lang} platform="macos" onAction={onAction} onLanguage={onLanguage} />);
  return { onAction, onLanguage, rerender };
}

afterEach(cleanup);

// jsdom has no ResizeObserver; Radix measures checkboxes and radios with it.
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
};

describe("Shell", () => {
  it("focuses the heading on every screen change", () => {
    const { rerender } = setup(install);
    expect(document.activeElement).toBe(screen.getByRole("heading", { level: 1 }));
    rerender(working);
    const heading = screen.getByRole("heading", { level: 1 });
    expect(heading.textContent).toBe("Installing Georgian…");
    expect(document.activeElement).toBe(heading);
  });

  it("announces status changes in a polite live region", () => {
    setup(working);
    const live = document.querySelector('[aria-live="polite"][role="status"].sr-only');
    expect(live?.textContent).toBe("1 of 4 files");
  });

  it("runs the main action on Enter when no control has focus", () => {
    const { onAction } = setup(install);
    fireEvent.keyDown(screen.getByRole("heading", { level: 1 }), { key: "Enter" });
    expect(onAction).toHaveBeenCalledWith("install", undefined);
  });

  it("leaves Enter to a focused control", () => {
    const { onAction } = setup(install);
    const more = screen.getByRole("button", { name: /More/ });
    more.focus();
    fireEvent.keyDown(more, { key: "Enter" });
    expect(onAction).not.toHaveBeenCalledWith("install", undefined);
  });

  it("draws the footer from the screen's actions", () => {
    setup(working);
    // Writing can't be interrupted: the footer has its status and no button.
    expect(document.querySelector("footer")?.textContent).toContain("1 of 4 files");
    expect(document.querySelector("footer button")).toBeNull();
  });

  it("opens More as a modal dialog, closes it on Esc and returns focus", async () => {
    const { onAction } = setup(install);
    const button = screen.getByRole("button", { name: /More/ });
    button.focus();
    fireEvent.click(button);
    const dialog = screen.getByRole("dialog");
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(onAction).toHaveBeenCalledWith("show_more", undefined);
    expect(dialog.contains(document.activeElement)).toBe(true);
    expect(document.querySelector("main")?.hasAttribute("inert")).toBe(true);
    // Restore original font is offered because the watch is restorable.
    expect(screen.getByRole("button", { name: "Restore original font" })).toBeTruthy();
    act(() => {
      fireEvent.keyDown(dialog, { key: "Escape" });
    });
    expect(screen.queryByRole("dialog")).toBeNull();
    // The sheet hands focus back once it has unmounted.
    await waitFor(() => expect(document.activeElement).toBe(button));
  });

  it("keeps Tab inside the More sheet", () => {
    setup(install);
    fireEvent.click(screen.getByRole("button", { name: /More/ }));
    const dialog = screen.getByRole("dialog");
    const close = screen.getByRole("button", { name: "Close" });
    close.focus();
    fireEvent.keyDown(dialog, { key: "Tab", shiftKey: true });
    expect(dialog.contains(document.activeElement)).toBe(true);
  });

  it("switches language with the language's own name", () => {
    const { onLanguage } = setup(install, "ka");
    fireEvent.click(screen.getByRole("button", { name: "English" }));
    expect(onLanguage).toHaveBeenCalledWith("en");
  });

  it("leaves room for the traffic lights by the window's chrome, not the platform's words", () => {
    const inset = (props: { platform: "macos" | "windows" | "linux"; overlayTitleBar?: boolean }) => {
      const view = render(<Shell model={install} more={more} lang="en" {...props} onAction={vi.fn()} onLanguage={vi.fn()} />);
      const padding = view.container.querySelector("header")?.style.paddingLeft;
      view.unmount();
      return padding;
    };
    expect(inset({ platform: "macos" })).toBe("80px");
    expect(inset({ platform: "windows" })).toBe("12px");
    expect(inset({ platform: "linux" })).toBe("12px");
    // The demo build on a Mac showing Windows' or Linux's words: the traffic lights stay.
    expect(inset({ platform: "windows", overlayTitleBar: true })).toBe("80px");
    expect(inset({ platform: "linux", overlayTitleBar: true })).toBe("80px");
    expect(inset({ platform: "macos", overlayTitleBar: false })).toBe("12px");
  });

  it("emits the whole answer set from the check screen", () => {
    const { onAction } = setup({
      screen: "check",
      watch: venu2,
      step: "answer",
      answers: { views: [] },
      phone: null,
      markPictures: [],
    });
    expect(screen.getByRole("button", { name: "Continue" }).hasAttribute("disabled")).toBe(true);
    fireEvent.click(screen.getByRole("radio", { name: /Boxes/ }));
    expect(onAction).toHaveBeenCalledWith("answer", { action: "answer", answers: { views: [], result: "boxes" } });
  });

  // Every code that can carry a rollback: the body names no file and never says "try again".
  it.each(["device_error", "verification_failed", "no_device"] as const)(
    "%s with an incomplete rollback offers Restore and keeps file names in Details",
    (code) => {
      for (const lang of ["en", "ka"] as const) {
        const { onAction } = setup(
          {
            screen: "error",
            op: "install",
            watch: venu2,
            code,
            rollback: { status: "incomplete", files: ["006-D9035-02.tmp"] },
            detail: "SendObject failed",
          },
          lang,
        );
        const alert = screen.getByRole("alert");
        expect(alert.textContent).not.toMatch(/try again|თავიდან/i);
        expect(alert.textContent).not.toContain(".tmp");
        expect(document.body.textContent).not.toContain("006-D9035-02.tmp");
        fireEvent.click(screen.getByRole("button", { name: lang === "en" ? "Details" : "დეტალები" }));
        expect(document.body.textContent).toContain("006-D9035-02.tmp");
        fireEvent.click(document.querySelector<HTMLButtonElement>("[data-main-action]")!);
        expect(onAction).toHaveBeenCalledWith("restore", undefined);
        expect(screen.getByRole("button", { name: lang === "en" ? "Back" : "უკან" })).toBeTruthy();
        cleanup();
      }
    },
  );

  it("shares without a watch naming no model", () => {
    setup({
      screen: "share",
      watch: null,
      what: "problem",
      summary: [{ label: "What happened", value: "device busy" }],
      reportText: "app 0.4.0",
      state: "opened",
      thanks: "we_look",
    });
    expect(document.body.textContent).not.toContain("Garmin owner");
    expect(screen.queryByText("Watch")).toBeNull();
    expect(screen.getByText("Thank you! We'll look into it and reply on GitHub.")).toBeTruthy();
  });
});
