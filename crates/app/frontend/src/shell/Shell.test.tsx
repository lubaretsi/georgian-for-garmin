import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
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
    expect(document.querySelector(".action-bar")).toBeNull();
  });

  it("opens More as a modal dialog, closes it on Esc and returns focus", () => {
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
    expect(document.activeElement).toBe(button);
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
});
