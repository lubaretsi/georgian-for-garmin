// Text that is the same in both languages. The phone test page, the issue forms and the
// docs cite these exact strings.

/** The text the watch should show (site/strings.js `TEST_TEXT`). */
export const TEST_TEXT = "Test მხედრული ᲛᲗᲐᲕᲠᲣᲚᲘ Test";

/** The Linux udev rule commands from docs/command-line.md, run in the unpacked app folder. */
export const LINUX_UDEV_COMMANDS = [
  "sudo install -m 644 70-georgian-for-garmin.rules /etc/udev/rules.d/",
  "sudo udevadm control --reload",
].join("\n");

export const REPO_URL = "https://github.com/lubaretsi/georgian-for-garmin";

/** The rendered answer samples (§5.2): Georgian, boxes, question marks, some letters. */
export const RESULT_SAMPLES = {
  georgian: "ქართ",
  boxes: "□□□□",
  question_marks: "????",
  some_letters: "ქა□□",
} as const;

/** The language names the switch shows, each in its own language. */
export const LANGUAGE_NAMES = { ka: "ქართული", en: "English" } as const;
