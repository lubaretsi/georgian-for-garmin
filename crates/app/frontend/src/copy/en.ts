// English copy. The shape of this object is the catalog type (`Copy`); ka.ts must match it
// key for key, with the same placeholders per string (src/copy/copy.test.ts).
// Vocabulary: CONTRACT.md §7. Main-flow keys may not use the §2.3 words (copy.test.ts).

export const en = {
  app: {
    name: "Georgian for Garmin",
    demo: "Demo: no real watch",
  },

  header: {
    language: "Language",
    more: "More",
  },

  steps: {
    label: "Steps",
    connect: "Connect",
    install: "Install",
    restart: "Restart",
    check: "Check",
    /** Read after a step's name by screen readers. */
    current: "current step",
    done: "done",
  },

  actions: {
    install: "Install Georgian",
    restore: "Restore original font",
    check: "Check on the watch",
    didntWorkCheck: "Didn't work? Check on the watch",
    sendTest: "Send a test notification",
    skipVerify: "I'll check on the watch",
    /** After a restore: back to the watch screen without waiting for the reconnect. */
    skip: "Skip",
    sentIt: "I sent it",
    continue: "Continue",
    back: "Back",
    cancel: "Cancel",
    done: "Done",
    notNow: "Not now",
    tryAgain: "Try again",
    tryAgainNow: "Try again now",
    restartAgain: "Restart once more",
    shareResult: "Share the result",
    shareFontList: "Share the font list",
    reportProblem: "Report this problem",
    stillStuck: "Still stuck? Share what the app found",
    openGithub: "Open GitHub",
    followUpdates: "Follow updates",
    whichWatches: "Which watches work",
    copy: "Copy",
    copyCommands: "Copy the commands",
    copied: "Copied",
    copyFailed: "Couldn't copy. Select the text and copy it yourself.",
    details: "Details",
    close: "Close",
    saveToFile: "Save to file",
    showInFolder: "Show in folder",
  },

  watchCard: {
    software: "Software {software}",
    /** The watch list's row labels (watch / install, outcome). */
    labels: {
      watch: "Watch",
      software: "Software",
      georgian: "Georgian",
    },
    /** The Georgian row's badge, from `confidence`. */
    status: {
      confirmed: "Confirmed",
      confirmed_other_software: "Confirmed on other software",
      confirmed_some_fonts: "Partly confirmed",
      unconfirmed: "Not confirmed yet",
    },
  },

  connect: {
    searching: {
      title: "Connect your watch with its USB cable",
      lead: "On Garmin watches, Georgian notifications show as boxes. The app adds Georgian letters to your watch's own font.",
      looking: "Looking for your watch…",
      previewTitle: "How it goes",
      preview: {
        connect: "Plug the watch into this computer.",
        install: "The app adds Georgian to the watch's font.",
        restart: "The watch loads the new font.",
        check: "Send a test notification and see what shows.",
      },
    },
    help: {
      title: "Can't find your watch?",
      /** Each tip: a short title and one line. */
      express: {
        macos: { title: "Quit Garmin Express", body: "Click its icon in the menu bar and choose Quit." },
        windows: { title: "Quit Garmin Express", body: "Right-click its icon next to the clock and choose Exit." },
        linux: { title: "Eject the watch in your file manager", body: "Only one program can use the watch at a time." },
      },
      replug: { title: "Connect the watch again", body: "If the watch asks how to connect, choose MTP: that's how it shares its files." },
      cable: { title: "Try another cable", body: "Some USB cables only charge." },
    },
    busy: {
      title: {
        confirmed: "{program} is using your watch",
        unconfirmed: "{program} may be using your watch",
      },
      program: {
        garminExpress: "Garmin Express",
        cameraService: "macOS's camera service",
        fileManager: "Your file manager",
        named: "“{program}”",
        unknown: "Another program",
        thisApp: "This app",
      },
      nameCut: "The system shows only the start of the program's name.",
      lead: "Only one program can use the watch at a time. Free it like this:",
      /** The watch held by this very app: an earlier step of it hasn't finished. */
      thisApp: {
        title: "This app is still using the watch",
        lead: "An earlier step hasn't finished yet.",
      },
      steps: {
        garmin_express: {
          macos: [
            "Click the Garmin Express icon in the menu bar and choose Quit.",
            "If it keeps running, open Activity Monitor, search for “Garmin” and quit what it finds.",
          ],
          windows: [
            "Right-click the Garmin Express icon in the notification area next to the clock and choose Exit.",
            "If it keeps running, open Task Manager, search for “Garmin” and end what it finds.",
          ],
          linux: ["Quit Garmin Express, then try again."],
        },
        macos_camera_service: {
          macos: [
            "Quit Photos, Image Capture, Preview and any other app that imports photos.",
            "Unplug the watch and connect it again.",
          ],
          windows: [
            "Quit any app that imports photos from cameras or phones.",
            "Unplug the watch and connect it again.",
          ],
          linux: [
            "Quit any app that imports photos from cameras or phones.",
            "Unplug the watch and connect it again.",
          ],
        },
        desktop_mtp: {
          macos: [
            "Quit that app: it opens phones and watches connected by USB.",
            "Unplug the watch and connect it again.",
          ],
          windows: [
            "Quit the app that opened the watch.",
            "Unplug the watch and connect it again.",
          ],
          linux: [
            "Eject the watch in your file manager's sidebar.",
            "On KDE, if that doesn't help, run kquitapp6 kiod6 in a terminal (kquitapp5 kiod5 on Plasma 5). KDE takes the watch again the next time it's connected.",
          ],
        },
        other: {
          macos: ["Quit it (in Activity Monitor if it has no window), then try again."],
          windows: ["Close it (in Task Manager if it has no window), then try again."],
          linux: ["Quit it, then try again."],
        },
        unknown: {
          macos: [
            "Quit Garmin Express and any app for phones, cameras or photos.",
            "If that doesn't help, restart the Mac and try again before opening other apps.",
          ],
          windows: [
            "Quit Garmin Express and any app for phones, cameras or photos.",
            "If that doesn't help, restart the computer and try again before opening other apps.",
          ],
          linux: [
            "Eject the watch in your file manager: it's the most common cause.",
            "If that doesn't help, restart the computer and try again before opening other apps.",
          ],
        },
        this_app: {
          macos: [
            "Wait for that step to finish: the app tries again on its own.",
            "If the watch stays busy, quit the app and open it again.",
          ],
          windows: [
            "Wait for that step to finish: the app tries again on its own.",
            "If the watch stays busy, close the app and open it again.",
          ],
          linux: [
            "Wait for that step to finish: the app tries again on its own.",
            "If the watch stays busy, close the app and open it again.",
          ],
        },
      },
      retrying: "The app tries again every few seconds.",
      /** Busy while the app follows a restart. */
      duringRestart: "Once the watch is free, the app goes on checking the restart.",
    },
    problem: {
      permission_denied: {
        title: "Linux doesn't let the app open the watch yet",
        body: "Linux lets the app use the watch only with a rule that comes with the app. Open a terminal in the folder you unpacked the app into and run these two commands. Then unplug the watch and connect it again.",
      },
      multiple_devices: {
        title: "More than one Garmin device is connected",
        body: "Unplug every Garmin device except the watch you want to change. The app goes on by itself.",
      },
      garmin_folder_not_found: {
        title: "The app couldn't read the watch's files",
        body: "Unplug the watch and connect it again. If this keeps happening, report it so we can look.",
      },
      extdata_not_found: {
        title: "The app couldn't find the watch's font folder",
        body: "Restart the watch and connect it again. If this keeps happening, report it so we can look.",
      },
      device_error: {
        title: "The watch stopped responding",
        body: "Unplug the watch, check the cable and connect it again.",
      },
      internal: {
        title: "Something went wrong in the app",
        body: "Reporting what happened helps us fix it.",
      },
    },
  },

  drive: {
    /** In drive mode while the app follows a restart. */
    duringRestart: "Once the watch connects over MTP, the app goes on checking the restart.",
    title: "Your {model} is connected as a USB drive",
    mtp: {
      body: "The app can add Georgian only when the watch connects in its other mode, MTP, which shares its files with the computer.",
      how: "If the watch asks how to connect, choose MTP. If it doesn't ask, look for USB mode in the watch's system settings and choose MTP there. Then unplug the watch and connect it again.",
      waiting: "The app is waiting for the watch.",
    },
    driveOnly: {
      body: "The app can't add Georgian to this watch yet: it connects only as a USB drive, and the app can't change its fonts that way.",
      follow: "Follow updates on GitHub to hear when that changes.",
    },
  },

  /** One line from `confidence` (§5.2). {versions} lists `confirmedSoftware`. */
  confidence: {
    confirmed: "Georgian is confirmed on {model} with software {software}.",
    confirmed_other_software: "Georgian is confirmed on {model} with software {versions}, not yet with {software}.",
    confirmed_other_software_unlisted: "Georgian is confirmed on {model} with other software, not yet with {software}.",
    confirmed_some_fonts: "On {model}, Georgian is confirmed in some of the fonts the app installs, not yet in all of them.",
    unconfirmed: "No one has confirmed Georgian on {model} yet.",
  },

  /** Why a check on the watch helps, when Georgian isn't confirmed for this watch. */
  checkWhy: {
    confirmed_other_software: "Checking takes a minute, and your answer tells us whether it works with software {software} too.",
    confirmed_some_fonts: "Checking takes a minute, and your answer tells every {model} owner whether it works.",
    unconfirmed: "Checking takes a minute, and your answer tells every {model} owner whether it works.",
  },

  watch: {
    install: {
      titleConfirmed: "Georgian works on this watch",
      title: "Your watch is ready for Georgian",
      unconfirmedFallback: "If Georgian doesn't show, one click puts Garmin's font back.",
      note: {
        partly: "Some of the watch's fonts already have Georgian. Installing adds the rest.",
        beta: "A test version of Georgian is on this watch. Installing replaces it with this one.",
        not_visible: "This watch doesn't show its font files to the computer, so the app can't tell whether Georgian is already on it. Installing again does no harm.",
      },
    },
    installed: {
      title: "Georgian is on your watch",
      unplug: "You can unplug the watch.",
    },
    restartPending: {
      title: "Restart the watch to finish",
      georgian: "Georgian is already on the watch and shows after a restart.",
      stock: "Garmin's font is already on the watch and comes back after a restart.",
    },
    blocked: {
      waiting_other_font: {
        title: "Another font is waiting on the watch",
        body: "Restart the watch, then connect it again.",
      },
      unknown_font: {
        title: "The app doesn't recognise a font on this watch",
        body: "So it changes nothing. Sharing the watch's font list helps us add it.",
      },
    },
    unsupported: {
      title: "Georgian doesn't work on {model} yet",
      restorable: "An earlier test version left fonts on this watch. Restore original font puts Garmin's fonts back.",
      follow: "Follow updates on GitHub to hear when that changes.",
    },
    noFont: {
      title: "There's no Georgian font for {model} yet",
      body: "Sharing the watch's font list helps us look for one.",
    },
    unreadable: {
      title: "The app couldn't read the watch's fonts",
      body: "Unplug the watch and connect it again, then try again.",
    },
    shareWhy: "Sharing the watch's font list helps us add it. It needs a free GitHub account.",
    sharedBefore: "You've already shared this watch's font list. Thank you!",
  },

  confirmRestore: {
    title: "Put Garmin's original font back?",
    body: "Georgian notifications will show as boxes again. You can install Georgian again at any time.",
  },

  working: {
    install: "Installing Georgian…",
    restore: "Restoring Garmin's font…",
    keepConnected: "Keep the watch connected.",
    progress: "{done} of {total} files",
  },

  restart: {
    attempt: "Attempt {attempt}",
    /** The footer's status while the app waits for the watch to come back. */
    waiting: "Waiting for the watch…",
    unplug: {
      title: "Unplug the watch",
      install: "Georgian is on the watch. It shows after a restart.",
      restore: "Garmin's font is on the watch. It comes back after a restart.",
      waiting: "The app goes on as soon as it sees the watch unplugged.",
    },
    restart: {
      title: "Restart the watch",
      /** The two numbered steps: a short title and one line each. */
      hold: { title: "Hold the power button", body: "Choose Power Off, then turn the watch on again." },
      reconnect: { title: "Connect it again", body: "The app checks that the watch loaded the font." },
      notVerifiable: "This watch doesn't show the change to the computer, so you check it on the watch.",
      notVerifiableRestore: "This watch doesn't show the change to the computer. Garmin's font is back once the watch has restarted.",
    },
    verifying: {
      title: "Checking the watch…",
      body: "This takes a few seconds.",
    },
    again: {
      title: "Restart the watch once more",
      body: "The watch hasn't loaded the font yet. This happens sometimes.",
      how: "Unplug the watch, restart it once more, then connect it again.",
    },
    otherWatch: {
      title: "This is a different watch",
      body: "Connect the {model} again.",
    },
  },

  result: {
    install: {
      title: "Georgian is on your watch",
      confirmed: "Georgian notifications should show as letters now.",
    },
    restore: {
      title: "Garmin's font is back",
      body: "Georgian notifications show as boxes again. You can unplug the watch.",
    },
  },

  check: {
    send: {
      title: "Send yourself a test notification",
      scan: "Scan this with your phone, then tap Send test notification.",
      qrLabel: "QR code for the phone test page",
      orText: "Or send yourself this text, for example in a messaging app:",
      textOnly: "Send yourself this text, for example in a messaging app:",
      expect: "The watch should show Georgian letters, capitals included.",
    },
    answer: {
      title: "What did the watch show?",
      results: {
        georgian: "Georgian letters",
        boxes: "Boxes",
        question_marks: "Question marks",
        some_letters: "Some letters, some boxes",
        no_notification: "Nothing arrived",
      },
      views: {
        legend: "Where did the letters show?",
        popup: "When it arrived",
        list: "In the notification list",
        opened: "When I opened it",
      },
      phone: {
        legend: "Your phone",
        android: "Android",
        iphone: "iPhone",
      },
      marker: {
        legend: "Which one matches the watch?",
        hint: "In this beta, one capital letter has a bar, in a different place for each font. Pick what the watch showed.",
        none: "None of these",
        picture: "Variant {variant}",
      },
      required: "Choose what the watch showed to continue.",
      /** Next to the legends of the questions that may be left out. */
      optional: "Optional",
    },
  },

  outcome: {
    works_confirmed: {
      title: "Georgian works. Thanks!",
      body: "You can unplug the watch.",
    },
    works_first: {
      title: "Georgian works!",
      body: "You're the first to confirm it on {model}. Share the result so every owner knows.",
      account: "Needs a free GitHub account. Nothing is sent until you press Submit.",
    },
    didnt_show: {
      title: "Georgian didn't show",
      tryRestart: "Sometimes the watch needs one more restart.",
      share: "Restarting didn't help. Sharing the result helps us find out why.",
      restoreHint: "To put Garmin's font back, choose Restore original font under More.",
    },
    no_notification: {
      title: "The notification didn't reach the watch",
      body: "The watch shows a phone notification only if the phone passes it on.",
    },
    sharedBefore: "You've already shared this result. Thank you!",
    /** The answer as a list: watch, software, what the watch showed. */
    result: {
      title: "Your result",
      showed: "The watch showed",
    },
  },

  phone: {
    android: "Android: in Garmin Connect's notification settings, add Chrome (or the browser you use) to the apps that notify the watch.",
    iphone: "iPhone: open the test page in Safari, add it to the Home Screen (Share › Add to Home Screen, iOS 16.4 or later) and open it from there.",
    either: "Or send the test text to yourself in a messaging app.",
  },

  share: {
    title: {
      result: "Share the result",
      font_list: "Share the watch's font list",
      problem: "Report this problem",
    },
    why: {
      result: "Your answer tells every {model} owner whether Georgian works.",
      font_list: "The font list helps us find a way to add Georgian to {model}.",
      problem: "Knowing what happened helps us fix it.",
    },
    whatTitle: "What's shared",
    /** Labels and values of the shared summary; the flow builds the list (toScreen). */
    summary: {
      watch: "Watch",
      software: "Software",
      installed: "What the app installed",
      nothing: "Nothing yet",
      fonts: "{count} fonts",
      result: "What the watch showed",
      views: "Where",
      phone: "Phone",
      fontList: "The watch's font list",
      problem: "What happened",
    },
    private: "No serial number or watch ID.",
    account: "Sharing needs a free GitHub account.",
    nothingSent: "Nothing is sent until you press Submit on GitHub.",
    exactText: "Show the exact text",
    opened: {
      title: "GitHub opened in your browser",
      body: "Sign in if asked, check the form, then press Submit.",
      pasted: "The text is copied: paste it into the empty “Watch report” box.",
    },
    failed: {
      title: "GitHub didn't open",
      body: "Try again. You can also copy the exact text below.",
    },
    thanks: {
      tested_next: "Thank you! Once it's posted, the next version of the app lists {model} as confirmed.",
      we_look: "Thank you! We'll look into it and reply on GitHub.",
    },
  },

  error: {
    conflict: {
      title: "A font on the watch isn't Garmin's or ours",
      body: "Nothing was changed. If you put that font on the watch yourself, it may be waiting for a restart: restart the watch and connect it again. Otherwise, share the watch's font list so we can look.",
    },
    restore_first: {
      title: "Put Garmin's font back first",
      body: "An earlier test version of this app left a font the watch won't let the app read, so nothing was changed. Restore the original font, restart the watch, then install Georgian again.",
    },
    verification_failed: {
      title: "The font didn't arrive intact",
      body: "What the app wrote didn't match when it read it back. Try again.",
    },
    device_error: {
      title: "The watch stopped responding",
      body: "Check the cable, keep the watch connected and try again.",
    },
    no_device: {
      title: "The watch was disconnected",
      body: "Connect it again: the app goes on when it sees the watch.",
    },
    unsupported_device: {
      title: "There's nothing to change on this watch",
      body: "The app found no font on this watch it can add Georgian to.",
    },
    internal: {
      title: "Something went wrong in the app",
      body: "Reporting what happened helps us fix it.",
    },
    rollback: {
      complete: "Everything the app wrote was undone, so the watch is as it was.",
      /** Replaces the code's body when the undo was incomplete: no "try again", no file names. */
      incomplete: "Some fonts on the watch may not be as they were.",
      /** Under Details. */
      files: "Files that may have changed: {files}",
      restoreHint: "Restore original font puts Garmin's font back.",
    },
  },

  more: {
    title: "More",
    language: "Language",
    restore: {
      title: "Original font",
      body: "Puts Garmin's own font back on the watch.",
    },
    diagnostics: {
      title: "Diagnostic info",
      body: "Save or copy it when we ask for it.",
    },
    seen: {
      title: "What the app sees",
      noWatch: "No watch is connected.",
      reportTitle: "The exact text shared on GitHub",
      /** Labels for the flow's "What the app sees" list. The only copy that may name internals. */
      labels: {
        model: "Model",
        modelPart: "Part number",
        software: "Software",
        writes: "What install writes",
        state: "Font state",
      },
    },
    help: {
      title: "Help",
      connecting: "Connect the watch with a data cable: some USB cables only charge.",
      mtp: "If the watch asks how to connect, choose MTP: that's how it shares its files with a computer.",
      express: {
        macos: "Garmin Express can keep the app from seeing the watch. Quit it: click its icon in the menu bar and choose Quit.",
        windows: "Garmin Express can keep the app from seeing the watch. Quit it: right-click its icon in the notification area next to the clock and choose Exit.",
        linux: "Your file manager can keep the app from seeing the watch. Eject the watch there.",
      },
      phoneTitle: "Test notifications from your phone",
    },
    about: {
      title: "About",
      version: "App version {version}",
      unofficial: "Unofficial; not affiliated with Garmin.",
      source: "Source and issues on GitHub:",
    },
  },

  /** More › Diagnostic info (§11): saved or copied by the user, never sent. */
  diagnostics: {
    title: "Diagnostic info",
    lead: "If we ask you for diagnostic info, save it here and send it to us.",
    contents: "It lists the app, your system, the Garmin devices connected and the app's recent steps. It has no serial number or watch ID.",
    summaryTitle: "Summary",
    /**
     * The summary list the flow builds (toScreen): labels, then the values that aren't data.
     * A failure reads as the connect or error screen's title; never its code (§2.3).
     */
    summary: {
      app: "App version",
      system: "System",
      watch: "Watch",
      connection: "Connection",
      lastProblem: "Last problem",
      noWatch: "Not found",
      connected: "Connected",
      connectedTo: "Connected ({model})",
      noWatchFound: "No watch found",
      usbDrive: "Connected as a USB drive",
      none: "None",
    },
    fullText: "Show the full text",
    collecting: "Collecting the info…",
    saved: "Saved in your Downloads folder.",
    copied: "Copied. Paste it into your message to us.",
    failed: "Couldn't save the file. Copy the text instead.",
  },
} as const;
