import { STRINGS, TEST_TEXT } from "./strings.js";

const LANG_KEY = "lang";
const $ = (id) => document.getElementById(id);
const isLang = (value) => value !== null && Object.prototype.hasOwnProperty.call(STRINGS, value);

// ?lang= wins for this visit; the switch's choice is remembered in localStorage.
function initialLang() {
  const param = new URLSearchParams(location.search).get("lang");
  if (isLang(param)) return param;
  try {
    const saved = localStorage.getItem(LANG_KEY);
    if (isLang(saved)) return saved;
  } catch {
    // Storage can be off (private mode, blocked cookies): fall back to Georgian.
  }
  return "ka";
}

let lang = initialLang();
// The message currently shown in each status line, as a key, so a language switch re-renders it.
const shown = { status: null, "copy-status": null };

function t(key) {
  return STRINGS[lang][key];
}

function render() {
  document.documentElement.lang = lang;
  document.title = t("pageTitle");
  for (const el of document.querySelectorAll("[data-i18n]")) el.textContent = t(el.dataset.i18n);
  for (const [id, key] of Object.entries(shown)) if (key) $(id).textContent = t(key);
}

function say(id, key) {
  shown[id] = key;
  $(id).textContent = t(key);
  $(id).hidden = false;
}

$("lang").addEventListener("click", () => {
  lang = lang === "ka" ? "en" : "ka";
  try {
    localStorage.setItem(LANG_KEY, lang);
  } catch {
    // Not remembered; the switch still applies to this visit.
  }
  // Drop ?lang= so it doesn't override the choice on reload.
  const url = new URL(location.href);
  if (url.searchParams.has("lang")) {
    url.searchParams.delete("lang");
    history.replaceState(null, "", url);
  }
  render();
});

// --- Notification support -------------------------------------------------------------------

// iPadOS reports itself as a Mac; a touch screen tells them apart.
const isIos =
  /iPhone|iPad|iPod/.test(navigator.userAgent) ||
  (navigator.platform === "MacIntel" && navigator.maxTouchPoints > 1);
const isStandalone =
  navigator.standalone === true || matchMedia("(display-mode: standalone)").matches;

// iPhone Safari has no Notification API outside a Home Screen web app, and every browser needs a
// secure context (HTTPS or localhost).
function notificationsAvailable() {
  return window.isSecureContext && "Notification" in window;
}

// Puts the copy fallback first and makes it the main button.
function preferFallback(sendAvailable) {
  document.body.classList.add("fallback-first");
  $("send-section").hidden = !sendAvailable;
  $("send").classList.replace("primary", "secondary");
  $("copy").classList.replace("secondary", "primary");
}

function applySupport() {
  if (isIos) {
    $("android-note").hidden = true;
    $("ios-note").hidden = isStandalone && notificationsAvailable();
  }
  if (!notificationsAvailable()) {
    preferFallback(false);
    say("status", "unsupported");
  } else if (Notification.permission === "denied") {
    preferFallback(true);
    say("status", "denied");
  }
}

// Android Chrome only shows notifications through a service worker; `new Notification()` throws
// there. The worker's scope is the page's own directory (the site's subpath on GitHub Pages).
let workerReady = null;
if ("serviceWorker" in navigator && window.isSecureContext) {
  workerReady = navigator.serviceWorker
    .register("sw.js")
    .then(() => navigator.serviceWorker.ready)
    .catch(() => null);
}

function withTimeout(promise, ms) {
  return Promise.race([promise, new Promise((resolve) => setTimeout(() => resolve(null), ms))]);
}

async function showTestNotification() {
  const options = { body: TEST_TEXT, lang: "ka", tag: "georgian-test", renotify: true, icon: "icon-192.png" };
  const registration = workerReady && (await withTimeout(workerReady, 4000));
  if (registration) {
    try {
      await registration.showNotification(TEST_TEXT, options);
      return true;
    } catch {
      // Fall through to the page-level API (desktop browsers).
    }
  }
  try {
    new Notification(TEST_TEXT, options);
    return true;
  } catch {
    return false;
  }
}

$("send").addEventListener("click", async () => {
  if (!notificationsAvailable()) {
    preferFallback(false);
    say("status", "unsupported");
    return;
  }
  const button = $("send");
  button.disabled = true;
  say("status", "sending");
  try {
    // Asked from the tap itself: browsers ignore permission requests without a user gesture.
    let permission = Notification.permission;
    if (permission === "default") permission = await Notification.requestPermission();
    if (permission === "granted") {
      if (await showTestNotification()) {
        say("status", "sent");
      } else {
        preferFallback(true);
        say("status", "failed");
      }
    } else if (permission === "denied") {
      preferFallback(true);
      say("status", "denied");
    } else {
      say("status", "dismissed");
    }
  } catch {
    preferFallback(true);
    say("status", "failed");
  } finally {
    button.disabled = false;
  }
});

// --- Copy fallback --------------------------------------------------------------------------

function selectTestText() {
  const range = document.createRange();
  range.selectNodeContents($("test-text"));
  const selection = getSelection();
  selection.removeAllRanges();
  selection.addRange(range);
}

async function copyTestText() {
  if (navigator.clipboard && window.isSecureContext) {
    try {
      await navigator.clipboard.writeText(TEST_TEXT);
      return true;
    } catch {
      // Older browsers or a denied clipboard permission: try the selection route.
    }
  }
  selectTestText();
  try {
    return document.execCommand("copy");
  } catch {
    return false;
  }
}

$("copy").addEventListener("click", async () => {
  if (await copyTestText()) {
    say("copy-status", "copied");
  } else {
    selectTestText();
    say("copy-status", "copyFailed");
  }
});

$("test-text").textContent = TEST_TEXT;
render();
applySupport();
