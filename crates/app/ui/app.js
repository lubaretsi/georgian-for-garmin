// Screens: waiting → ready → working → staged → checking, or error; drive when the watch is
// mounted as a USB drive instead of MTP. While waiting, ready or on the drive screen the app
// polls the read-only `inspect` command (and `inspect_drive` when no MTP watch is found); the
// backend runs one watch operation at a time, and a result that arrives after the screen has
// moved on is ignored. When the waiting screen has found no watch for WAITING_HELP_MS, it shows
// tips for finding it; the wait starts over each time the screen comes up.
//
// Sharing goes through the core's `issue_link`: the GitHub issue form, filled in from the watch
// report, the last install or restore, and the answers on the check screen. Every screen that
// shares shows the exact report text first.
//
// Every user-facing string is in strings.json, one table per language with the same keys
// (`{name}` placeholders are filled in here). Georgian is the default; the language switch's
// choice is remembered by the backend.
"use strict";

const { invoke } = window.__TAURI__.core;

const POLL_MS = 2000;
const WAITING_HELP_MS = 10000;
const DEFAULT_LANGUAGE = "ka";
// Answer values, as the core's `issue_link` takes them.
const RESULTS = [
  "georgian",
  "boxes",
  "question_marks",
  "some_letters",
  "no_notification",
  "not_installed",
];
const VIEWS = ["popup", "list", "opened"];
const PHONES = ["android", "iphone"];
const NO_MARKER = "none";

let STRINGS = {};
let language = DEFAULT_LANGUAGE;
let T = {};
let demo = false;
/** The desktop, for platform-specific words: "macos", "windows" or "linux". */
let platform = "macos";
/** The beta build's test-mark pictures by part; empty in stable builds. */
let marks = new Map();

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

/** The view on screen, and the waiting screen's help: its timer, and whether it is due. */
let current = null;
let helpTimer = null;
let helpDue = false;

function show(view, draw) {
  if (view !== current) {
    // Leaving a screen cancels the help's timer and forgets a busy share; coming to the waiting
    // screen starts the timer.
    clearTimeout(helpTimer);
    helpDue = false;
    busyShared = null;
    if (view === "waiting") {
      helpTimer = setTimeout(() => {
        helpDue = true;
        redraw();
      }, WAITING_HELP_MS);
    }
    current = view;
  }
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

/**
 * What to do about a busy watch, from the core's `busy` (who holds it), in this platform's
 * words only. A holder kind without words of its own gets `other` when it has a name.
 */
function busyText(busy) {
  const words = T.busy[platform];
  const holder = busy ? busy.holder : { kind: "unknown", name: null };
  const text = words[holder.kind] || (holder.name ? words.other : words.unknown);
  return fill(text, { name: holder.name });
}

/**
 * A failed install or restore that couldn't undo all of its changes: which files may be left,
 * and the way back, after what went wrong.
 */
function rollbackText(rollback) {
  if (!rollback || rollback.status !== "incomplete") return "";
  const left = rollback.files.filter((f) => !f.done).map((f) => f.path.split("/").pop());
  return " " + fill(T.rollbackIncomplete, { files: [...new Set(left)].join(", ") });
}

const errorText = (f) =>
  (f.code === "device_busy" ? busyText(f.busy) : T.errors[f.code] || T.errors.internal) +
  rollbackText(f.rollback);

/** What sharing a busy watch did on this visit to the waiting screen: `{key, result, url}`. */
let busyShared = null;
const busyKey = (busy) => JSON.stringify(busy);

/** Shares what holds the watch: the report form with only the `app` and `busy` lines. */
async function shareBusy(busy) {
  const done = await openIssue({ busy });
  busyShared = { key: busyKey(busy), ...done };
  if (current === "waiting") redraw();
}

function showWaiting(error) {
  show("waiting", () => {
    const problem = $("waiting-problem");
    // `no_device` is what the waiting screen already says.
    problem.hidden = !error || error.code === "no_device";
    if (!problem.hidden) $("waiting-hint").textContent = errorText(error);
    const busy = !problem.hidden && error.code === "device_busy" ? error.busy : null;
    $("waiting-share-box").hidden = !busy;
    if (busy) {
      $("waiting-share").onclick = () => shareBusy(busy);
      const mine = busyShared && busyShared.key === busyKey(busy);
      $("waiting-share-why").hidden = mine;
      $("waiting-share-result").hidden = !mine;
      $("waiting-share-result").textContent = mine ? sharedText(busyShared) : "";
    }
    // A watch that answers with a problem has been found; its hint says what to do.
    $("waiting-help").hidden = !helpDue || !problem.hidden;
    $("waiting-help-express").textContent = T.waitingHelpExpress[platform];
  });
}

/**
 * What the main screen says about Georgian on the watch, from the core's `state` (the one rule
 * the CLI and the report use too), or null when nothing needs saying. Only for watches this
 * build installs on; `supportNote` covers the rest.
 */
function stateNote(report) {
  if (report.state === "other") {
    // A font the watch imported (a bitmap part's `.fnt`) that this build doesn't know: it is in
    // use, not waiting, and install would replace it at the next restart, so it refuses.
    const imported = report.fonts.some((f) => f.target && f.imported && f.file === "other");
    if (imported) return T.importedUnknownFont;
    // A TrueType file this build doesn't know isn't waiting for a restart; it blocks install.
    if (report.hosts.kind === "truetype") return T.unknownFont;
  }
  return report.state === "not_installed" ? null : T.state[report.state];
}

// Support status: the core's `support` (tested, expected, unsupported, unknown), from the
// support list in fonts/registry.json.

/** The status line: on another software version, which versions were confirmed. */
function supportHeadline(report) {
  const status = report.support.status;
  if (status === "expected" && report.tested.status === "tested_other_software") {
    return fill(T.supportOtherSoftware, { versions: report.tested.verified_software.join(", ") });
  }
  return T.support[status];
}

/** A sentence from the support list (`{en, ka}`), in the app's language. */
const sentence = (s) => s && (s[language] || s.en);

/** What the main screen says under the status, or null when nothing needs saying. */
function supportNote(report) {
  const support = report.support;
  if (support.status === "unsupported") {
    const reason = sentence(support.reason);
    return report.restorable ? `${reason} ${T.restoreHint}` : reason;
  }
  // Nothing installs, yet an earlier test version's fonts can be restored.
  if (!report.supported) return report.restorable ? T.restoreOnly : null;
  return stateNote(report);
}

/** The "Follow updates" link to an unsupported watch's GitHub issue. */
function drawFollow(id, support) {
  const button = $(id);
  const issue = support.issue;
  button.hidden = issue == null || support.status !== "unsupported";
  if (button.hidden) return;
  button.textContent = fill(T.followUpdates, { issue });
  // The demo build opens nothing.
  button.onclick = () => invoke("open_issue", { issue }).catch(() => {});
}

/** States in which Install Georgian would change nothing, so it isn't offered. */
const NOTHING_TO_INSTALL = ["installed", "waiting"];

// Sharing

/** What sharing last did on the ready or drive screen: `{watch, result, url}`. */
let shared = null;

const watchKey = (report) => `${report.model_part} ${report.software_version} ${report.connection}`;

/** The words for a share result (a strings key), with the link in the demo build. */
function sharedText(s) {
  return s.result === "shareDemo" ? fill(T.shareDemo, { url: s.url }) : T[s.result];
}

/** The core's `Answers` fields; anything else in `answers` (like `shared`) is the screen's own. */
const ANSWER_FIELDS = ["result", "views", "phone", "marker", "connect_version"];

/** `issue_link`'s input: only the core's answer fields, without empty ones. */
function linkInput(report, outcome, answers, busy) {
  const input = { report };
  if (outcome) input.outcome = outcome;
  if (busy) input.busy = busy;
  if (answers) {
    const a = {};
    for (const key of ANSWER_FIELDS) {
      const value = answers[key];
      if (value != null && !(Array.isArray(value) && value.length === 0)) a[key] = value;
    }
    input.answers = a;
  }
  return input;
}

/** Opens the filled-in issue form; returns `{result, url}` with `result` a strings key. */
async function openIssue(input) {
  try {
    const done = await invoke("share_report", { input });
    const result = demo ? "shareDemo" : done.complete ? "shareDone" : "sharePaste";
    return { result, url: done.url };
  } catch (e) {
    return { result: "shareFailed", url: "" };
  }
}

/** Shares `report` from the ready or drive screen (no outcome, no answers). */
async function shareWatch(report, screen) {
  const done = await openIssue(linkInput(report));
  shared = { watch: watchKey(report), ...done };
  if (!$(screen).hidden) redraw();
}

function drawShared(report, textId, resultId) {
  $(textId).textContent = report.text;
  const mine = shared && shared.watch === watchKey(report);
  $(resultId).hidden = !mine;
  $(resultId).textContent = mine ? sharedText(shared) : "";
  return mine;
}

/** The watch on the ready screen. */
let readyReport = null;

function showReady(report) {
  readyReport = report;
  show("ready", () => {
    const support = report.support;
    $("ready-model").textContent = report.device.model;
    $("ready-version").textContent = fill(T.softwareVersion, {
      version: report.device.software_version,
    });
    $("ready-support").hidden = false;
    $("ready-support").className = `tested ${support.status === "tested" ? "yes" : "no"}`;
    $("ready-support").textContent = supportHeadline(report);
    $("ready-untested").hidden = support.status !== "expected";
    const note = supportNote(report);
    $("ready-note").hidden = !note;
    $("ready-note").textContent = note || "";
    drawFollow("ready-follow", support);
    // A watch whose fonts only an earlier test version installed into can still restore.
    $("ready-actions").hidden = !report.supported && !report.restorable;
    // The support list's word that a model doesn't work beats a route this build has.
    $("install").hidden =
      !report.supported ||
      support.status === "unsupported" ||
      NOTHING_TO_INSTALL.includes(report.state);
    // Asked only when a report adds information (the core's rule).
    $("share").hidden = !support.report_wanted;
    // Once shared, the result takes the explanation's place.
    $("share-why").hidden = drawShared(report.report, "share-text", "share-result");
  });
}

/** A watch mounted as a USB drive: nothing installs, but its report can be shared. */
let driveReport = null;

function showDrive(output) {
  driveReport = output;
  show("drive", () => {
    $("drive-model").textContent = output.device.model;
    $("drive-version").textContent = fill(T.softwareVersion, {
      version: output.device.software_version,
    });
    $("drive-reason").hidden = output.support.status !== "unsupported";
    $("drive-reason").textContent = sentence(output.support.reason) || "";
    drawFollow("drive-follow", output.support);
    $("drive-share-box").hidden = !output.support.report_wanted;
    drawShared(output.report, "drive-text", "drive-result");
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
    const count = output.packs.reduce((n, pack) => n + pack.parts.length, 0);
    $("staged-sizes").textContent = fill(count > 1 ? T.stagedFiles : T.stagedOneFile, {
      count,
    });
    $("staged-packs").replaceChildren(
      ...output.packs.map((pack) => {
        const item = document.createElement("li");
        // Bitmap parts by pixel size; TrueType parts scale, so by part number.
        item.textContent =
          pack.kind === "truetype"
            ? fill(T.stagedTrueTypePack, {
                pack: pack.name,
                parts: pack.parts.map((p) => p.part).join(", "),
              })
            : fill(T.stagedPack, {
                pack: pack.name,
                sizes: pack.parts.map((p) => p.size_px).join(", "),
              });
        return item;
      }),
    );
    $("staged-after").textContent = T.afterRestart[output.variant];
    const georgian = output.variant === "georgian";
    $("staged-test").hidden = !georgian;
    $("check").hidden = !georgian;
  });
}

function showError(f) {
  show("error", () => {
    $("error-message").textContent = errorText(f);
    $("error-details").textContent = `${f.code}: ${f.message}`;
    // A failed install or restore can be reported with what led to it.
    $("error-report").hidden = !(lastRun && lastRun.outcome.error);
  });
}

// The check screen

/** The last install or restore: the report from before it and its outcome. */
let lastRun = null;
/** The check screen's answers, and the phone page by language. */
let answers = null;
const phonePages = new Map();

function choice(label, pressed, onClick, picture) {
  const button = document.createElement("button");
  button.className = "choice";
  button.setAttribute("aria-pressed", String(pressed));
  if (picture) {
    const img = document.createElement("img");
    img.src = picture;
    img.alt = "";
    button.append(img);
  }
  const text = document.createElement("span");
  text.textContent = label;
  button.append(text);
  button.addEventListener("click", () => {
    onClick();
    redraw();
    preview();
  });
  return button;
}

/** Target parts' test marks, for the marker question; empty in stable builds. */
function targetMarks() {
  return lastRun.report.parts
    .filter((p) => p.target && marks.has(p.part))
    .map((p) => marks.get(p.part));
}

const sawLetters = () => answers.result === "georgian" || answers.result === "some_letters";

async function drawPhone() {
  let page = phonePages.get(language);
  if (!page) {
    page = await invoke("phone_page", { language });
    phonePages.set(language, page);
  }
  $("check-qr").src = page.qr;
  $("check-qr").title = page.url;
}

function drawCheck() {
  drawPhone().catch(() => {
    $("check-qr").hidden = true;
  });
  const failed = Boolean(lastRun.outcome.error);
  $("check-phone").hidden = failed;
  // Already confirmed on this model and software: a report helps only if it didn't work.
  const confirmed = !failed && lastRun.report.support.status === "tested";
  $("check-confirmed").hidden = !confirmed;
  $("check-share").textContent = confirmed ? T.checkShareFailed : T.shareGithub;
  $("check-share").className = confirmed ? "secondary" : "primary";
  $("check-done").className = confirmed ? "primary" : "secondary";
  $("ask-result").replaceChildren(
    ...RESULTS.map((r) =>
      choice(T.results[r], answers.result === r, () => {
        answers.result = answers.result === r ? null : r;
        if (!sawLetters()) {
          answers.views = [];
          answers.marker = null;
        }
      }),
    ),
  );
  $("ask-views-box").hidden = !sawLetters();
  $("ask-views").replaceChildren(
    ...VIEWS.map((v) =>
      choice(T.views[v], answers.views.includes(v), () => {
        answers.views = answers.views.includes(v)
          ? answers.views.filter((x) => x !== v)
          : VIEWS.filter((x) => x === v || answers.views.includes(x));
      }),
    ),
  );
  const pictures = targetMarks();
  $("ask-marker-box").hidden = !sawLetters() || pictures.length === 0;
  $("ask-marker").replaceChildren(
    ...pictures.map((m) =>
      choice(
        m.variant,
        answers.marker === m.variant,
        () => {
          answers.marker = answers.marker === m.variant ? null : m.variant;
        },
        m.picture,
      ),
    ),
    choice(T.markerNone, answers.marker === NO_MARKER, () => {
      answers.marker = answers.marker === NO_MARKER ? null : NO_MARKER;
    }),
  );
  $("ask-phone").replaceChildren(
    ...PHONES.map((p) =>
      choice(T.phones[p], answers.phone === p, () => {
        answers.phone = answers.phone === p ? null : p;
      }),
    ),
  );
  $("check-result").hidden = !answers.shared;
  $("check-result").textContent = answers.shared ? sharedText(answers.shared) : "";
}

/** Shows exactly what will be shared, as the core builds it. */
async function preview() {
  if (!answers) return;
  const asked = answers;
  try {
    const link = await invoke("issue_link", {
      input: linkInput(lastRun.report, lastRun.outcome, asked, lastRun.busy),
    });
    if (asked !== answers) return;
    $("check-text").textContent = link.report_text;
    $("check-paste").hidden = link.report_in_url;
  } catch (e) {
    $("check-text").textContent = errorText(failure(e));
  }
}

function showCheck(result) {
  stopPolling();
  answers = { result, views: [], marker: null, phone: null, shared: null };
  show("checking", drawCheck);
  window.scrollTo(0, 0);
  preview();
}

async function shareCheck() {
  const asked = answers;
  asked.shared = await openIssue(
    linkInput(lastRun.report, lastRun.outcome, asked, lastRun.busy),
  );
  if (!$("checking").hidden && asked === answers) redraw();
}

// Polling and actions

let pollToken = 0;

function startPolling() {
  const token = ++pollToken;
  const tick = async () => {
    let result = null;
    let drive = null;
    let error = null;
    try {
      result = await invoke("inspect");
    } catch (e) {
      error = failure(e);
    }
    if (error && error.code === "no_device") {
      try {
        drive = await invoke("inspect_drive");
      } catch (e) {
        // No watch mounted as a drive either: the waiting screen says what to do.
      }
    }
    if (token !== pollToken) return;
    if (result) showReady(result);
    else if (drive) showDrive(drive);
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
  const report = readyReport && readyReport.report;
  showWorking(command);
  try {
    const output = await invoke(command);
    const parts = output.packs.flatMap((pack) => pack.parts.map((p) => p.part));
    lastRun = { report, outcome: { operation: command, status: output.status, parts } };
    showStaged(output);
  } catch (e) {
    const f = failure(e);
    lastRun = report
      ? { report, outcome: { operation: command, error: f.code }, busy: f.busy }
      : null;
    showError(f);
  }
}

function backToStart() {
  readyReport = null;
  driveReport = null;
  answers = null;
  showWaiting(null);
  startPolling();
}

$("install").addEventListener("click", () => stage("install"));
$("restore").addEventListener("click", () => stage("restore_stock"));
$("share-button").addEventListener("click", () => readyReport && shareWatch(readyReport.report, "ready"));
$("drive-share").addEventListener("click", () => driveReport && shareWatch(driveReport.report, "drive"));
$("check").addEventListener("click", () => showCheck(null));
$("error-report").addEventListener("click", () => showCheck("not_installed"));
$("check-share").addEventListener("click", shareCheck);
$("check-done").addEventListener("click", backToStart);
$("done").addEventListener("click", backToStart);
$("retry").addEventListener("click", backToStart);
$("waiting-retry").addEventListener("click", () => startPolling());

async function start() {
  STRINGS = await (await fetch("strings.json")).json();
  buildLanguageSwitch();
  const [saved, isDemo, desktop, pictures] = await Promise.all([
    invoke("saved_language").catch(() => null),
    invoke("demo_mode"),
    invoke("platform"),
    invoke("test_marks").catch(() => []),
  ]);
  demo = isDemo;
  platform = desktop;
  marks = new Map(pictures.map((m) => [m.part, m]));
  useLanguage(saved || DEFAULT_LANGUAGE);
  $("demo").hidden = !demo;
  backToStart();
}

start();
