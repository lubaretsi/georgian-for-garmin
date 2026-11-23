// Direction A's copy: rewrites and additions on top of the shared catalogs (src/copy).
// Everything not listed here comes from the catalogs unchanged. Vocabulary: CONTRACT.md §7.
// Each entry notes the catalog key it replaces, or "new".

import type { Lang } from "@/core/backend";

const en = {
  connect: {
    /** replaces connect.searching.lead: shorter, letters-not-boxes up front */
    lead: "The app adds Georgian letters to your watch's font, so Georgian notifications show letters instead of boxes.",
    /** replaces connect.searching.previewTitle ("How it goes") */
    previewTitle: "Four short steps",
  },
  watch: {
    /** replaces watch.install.title for the not-confirmed verdicts */
    installTitle: "Ready to install Georgian",
  },
  restart: {
    /** replaces restart.restart.how + restart.restart.reconnect: one action per step */
    steps: [
      "Hold the watch's power button and choose Power Off.",
      "Turn the watch back on.",
      "Connect it to the computer again: the app checks the rest.",
    ],
    /** new: the footer while the app waits for the restarted watch */
    waiting: "Waiting for the watch…",
  },
  check: {
    /** new: legends lose "(optional)"; this badge says it instead */
    optional: "Optional",
    /** replaces check.answer.views.legend */
    viewsLegend: "Where did the letters show?",
    /** replaces check.answer.phone.legend */
    phoneLegend: "Your phone",
  },
  outcome: {
    /** replaces outcome.works_first.body; the GitHub account moves to its own line (share.account) */
    worksFirstBody: "Share the result and every {model} owner will know Georgian works.",
  },
} as const;

type Widen<T> = T extends string ? string : T extends readonly (infer U)[] ? readonly Widen<U>[] : { readonly [K in keyof T]: Widen<T[K]> };
export type DesignCopy = Widen<typeof en>;

const ka: DesignCopy = {
  connect: {
    lead: "პროგრამა საათის შრიფტს ქართულ ასოებს დაამატებს და ქართული შეტყობინებები კვადრატების ნაცვლად ასოებით გამოჩნდება.",
    previewTitle: "ოთხი მარტივი ნაბიჯი",
  },
  watch: {
    installTitle: "ყველაფერი მზადაა ქართულის დასაყენებლად",
  },
  restart: {
    steps: [
      "ხანგრძლივად დააჭირეთ საათის ჩართვის ღილაკს და აირჩიეთ Power Off.",
      "ისევ ჩართეთ საათი.",
      "ისევ შეაერთეთ საათი კომპიუტერთან: დანარჩენს პროგრამა შეამოწმებს.",
    ],
    waiting: "საათს ველოდებით…",
  },
  check: {
    optional: "არასავალდებულო",
    viewsLegend: "სად გამოჩნდა ასოები?",
    phoneLegend: "თქვენი ტელეფონი",
  },
  outcome: {
    worksFirstBody: "გააზიარეთ შედეგი და {model}-ის ყველა მფლობელი გაიგებს, რომ ქართული მუშაობს.",
  },
};

export const designCopy: Record<Lang, DesignCopy> = { en, ka };
