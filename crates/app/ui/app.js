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

/** What the watch's target sizes hold now, as one note, or null when nothing needs saying. */
function stagedNote(report) {
  const staged = report.fonts.filter((f) => f.target).map((f) => f.staged);
  const all = (state) => staged.length > 0 && staged.every((s) => s === state);
  if (all("georgian")) return T.stagedNow.georgian;
  if (all("stock")) return T.stagedNow.stock;
  const mixed = staged.includes("georgian") && staged.includes("stock");
  return staged.includes("other") || mixed ? T.stagedNow.other : null;
}

/** The watch on the ready screen, and what sharing its report last did (a strings key). */
let readyReport = null;
let shared = null;

const watchKey = (report) => `${report.device.part_number} ${report.device.software_version}`;

function showReady(report) {
  if (shared && shared.watch !== watchKey(report)) shared = null;
  readyReport = report;
  show("ready", () => {
    const tested = report.pack && report.pack.tested;
    $("ready-model").textContent = report.device.model;
    $("ready-version").textContent = fill(T.softwareVersion, {
      version: report.device.software_version,
    });
    $("ready-tested").hidden = !tested;
    if (tested) {
      $("ready-tested").className = `tested ${tested.status === "tested" ? "yes" : "no"}`;
      $("ready-tested").textContent = fill(T.tested[tested.status], {
        versions: tested.verified_software.join(", "),
      });
    }
    $("ready-untested").hidden = !tested || tested.status === "tested";
    const note = report.supported ? stagedNote(report) : T.unsupportedWatch;
    $("ready-note").hidden = !note;
    $("ready-note").textContent = note || "";
    $("ready-actions").hidden = !report.supported;
    $("share").hidden = Boolean(tested && tested.status === "tested");
    // Once shared, the result takes the explanation's place.
    $("share-why").hidden = Boolean(shared);
    $("share-result").hidden = !shared;
    $("share-result").textContent = shared ? T[shared.result] : "";
  });
}

async function share() {
  const report = readyReport;
  if (!report) return;
  let result;
  try {
    const done = await invoke("share_report", {
      title: report.report.title,
      text: report.report.text,
    });
    result = done.complete ? "shareDone" : "sharePaste";
  } catch (e) {
    result = "shareFailed";
  }
  shared = { watch: watchKey(report), result };
  if (!$("ready").hidden) redraw();
}

function showWorking(command) {
  show("working", () => {
    $("working-title").textContent =
      command === "install" ? T.workingInstall : T.workingRestore;
  });
}

function showStaged(output) {
  show("staged", () => {
    const sizes = output.parts.map((p) => p.size_px).join(", ");
    $("staged-sizes").textContent = fill(
      output.parts.length > 1 ? T.stagedSizes : T.stagedOneSize,
      { sizes },
    );
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
  readyReport = null;
  showWaiting(null);
  startPolling();
}

$("install").addEventListener("click", () => stage("install"));
$("restore").addEventListener("click", () => stage("restore_stock"));
$("share-button").addEventListener("click", share);
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
