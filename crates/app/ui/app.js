// Screens: waiting → ready → working → staged, or error. While waiting or ready the app polls
// the read-only `inspect` command; the backend runs one watch operation at a time, and a
// result that arrives after the screen has moved on is ignored.
//
// Every user-facing string is in strings.json, one table per language with the same keys
// (`{name}` placeholders are filled in here). Georgian is the default; the language switch's
// choice is remembered by the backend.
"use strict";

const { invoke } = window.__TAURI__.core;

const POLL_MS = 2000;
const DEFAULT_LANGUAGE = "ka";

let STRINGS = {};
let language = DEFAULT_LANGUAGE;
let T = {};

const $ = (id) => document.getElementById(id);
const fill = (text, values) => text.replace(/\{(\w+)\}/g, (_, key) => values[key] ?? "");

// Language

/** Redraws the current screen in the current language. */
let redraw = () => {};

function useLanguage(code) {
  language = STRINGS[code] ? code : DEFAULT_LANGUAGE;
  T = STRINGS[language];
  document.documentElement.lang = language;
  for (const el of document.querySelectorAll("[data-t]")) {
    el.textContent = T[el.dataset.t];
  }
  for (const button of $("languages").children) {
    button.setAttribute("aria-pressed", String(button.dataset.language === language));
  }
  redraw();
}

function buildLanguageSwitch() {
  const nav = $("languages");
  for (const [code, table] of Object.entries(STRINGS)) {
    const button = document.createElement("button");
    button.dataset.language = code;
    button.lang = code;
    button.textContent = table.languageName;
    button.addEventListener("click", () => {
      useLanguage(code);
      // Not saved means only that the next launch starts in the default language.
      invoke("save_language", { language: code }).catch(() => {});
    });
    nav.append(button);
  }
  nav.hidden = false;
}

// Screens

function show(view, draw) {
  redraw = draw;
  draw();
  for (const el of document.querySelectorAll(".view")) {
    el.hidden = el.id !== view;
  }
}

/** Errors arrive as `{code, message}` from the backend, or as a string from Tauri itself. */
function failure(e) {
  if (e && typeof e === "object" && "code" in e) return e;
  return { code: "internal", message: String(e) };
}

const errorText = (f) => T.errors[f.code] || T.errors.internal;

function showWaiting(error) {
  show("waiting", () => {
    const problem = $("waiting-problem");
    // `no_device` is what the waiting screen already says.
    problem.hidden = !error || error.code === "no_device";
    if (!problem.hidden) $("waiting-hint").textContent = errorText(error);
  });
}

function showReady(report) {
  show("ready", () => {
    const font = report.fonts.find((f) => f.default) || report.fonts[0];
    const supported = Boolean(font && font.supported);
    $("ready-model").textContent = report.device.model;
    $("ready-version").textContent = fill(T.softwareVersion, {
      version: report.device.software_version,
    });
    const note = supported ? T.stagedNow[font.staged] : T.unsupportedWatch;
    $("ready-note").hidden = !note;
    $("ready-note").textContent = note || "";
    $("ready-actions").hidden = !supported;
  });
}

function showWorking(command) {
  show("working", () => {
    $("working-title").textContent =
      command === "install" ? T.workingInstall : T.workingRestore;
  });
}

function showStaged(output) {
  show("staged", () => {
    $("staged-after").textContent = T.afterRestart[output.variant];
    $("staged-test").hidden = output.variant !== "georgian";
  });
}

function showError(f) {
  show("error", () => {
    $("error-message").textContent = errorText(f);
    $("error-details").textContent = `${f.code}: ${f.message}`;
  });
}

// Polling and actions

let pollToken = 0;

function startPolling() {
  const token = ++pollToken;
  const tick = async () => {
    let result = null;
    let error = null;
    try {
      result = await invoke("inspect");
    } catch (e) {
      error = failure(e);
    }
    if (token !== pollToken) return;
    if (result) showReady(result);
    else showWaiting(error);
    setTimeout(() => token === pollToken && tick(), POLL_MS);
  };
  tick();
}

function stopPolling() {
  pollToken++;
}

async function stage(command) {
  stopPolling();
  showWorking(command);
  try {
    showStaged(await invoke(command));
  } catch (e) {
    showError(failure(e));
  }
}

function backToStart() {
  showWaiting(null);
  startPolling();
}

$("install").addEventListener("click", () => stage("install"));
$("restore").addEventListener("click", () => stage("restore_stock"));
$("done").addEventListener("click", backToStart);
$("retry").addEventListener("click", backToStart);
$("waiting-retry").addEventListener("click", () => startPolling());

async function start() {
  STRINGS = await (await fetch("strings.json")).json();
  buildLanguageSwitch();
  const [saved, demo] = await Promise.all([
    invoke("saved_language").catch(() => null),
    invoke("demo_mode"),
  ]);
  useLanguage(saved || DEFAULT_LANGUAGE);
  $("demo").hidden = !demo;
  backToStart();
}

start();
