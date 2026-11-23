// Direction B's own copy: the strings it rewrites or adds for its screens, in both
// languages. Everything else comes from the shared catalogs (src/copy). Vocabulary and the
// words never used in the main flow: CONTRACT.md §7 and §2.3.

import type { Lang } from "../../core/backend";

const en = {
  toolbar: {
    steps: "Step {n} of 4",
  },
  connect: {
    lead: "On Garmin watches, Georgian notifications show as boxes. The app adds Georgian letters to your watch's own font.",
    help: {
      express: {
        macos: { title: "Quit Garmin Express", body: "Click its icon in the menu bar and choose Quit." },
        windows: { title: "Quit Garmin Express", body: "Right-click its icon next to the clock and choose Exit." },
        linux: { title: "Eject the watch in your file manager", body: "Only one program can use the watch at a time." },
      },
      replug: { title: "Connect the watch again", body: "If the watch asks how to connect, choose MTP." },
      cable: { title: "Try another cable", body: "Some USB cables only charge." },
    },
    preview: {
      connect: "Plug the watch into this computer.",
      install: "The app adds Georgian to the watch's font.",
      restart: "The watch loads the new font.",
      check: "Send a test notification and see what shows.",
    },
  },
  watch: {
    title: "Your watch is ready for Georgian",
    rows: { watch: "Watch", software: "Software", georgian: "Georgian" },
    status: {
      confirmed: "Confirmed",
      confirmed_other_software: "Confirmed on other software",
      confirmed_some_fonts: "Partly confirmed",
      unconfirmed: "Not confirmed yet",
    },
    safe: "If Georgian doesn't show, one click puts Garmin's font back.",
  },
  restart: {
    lead: "Georgian is on the watch. It shows after a restart.",
    hold: { title: "Hold the power button", body: "Choose Power Off, then turn the watch on again." },
    reconnect: { title: "Connect it again", body: "The app checks that the watch loaded the font." },
    waiting: "Waiting for the watch…",
    skip: "I'll check on the watch",
  },
  check: {
    optional: "Optional",
    viewsLegend: "Where did the letters show?",
    views: { popup: "When it arrived", list: "In the notification list", opened: "When I opened it" },
    phoneLegend: "Your phone",
  },
  outcome: {
    title: "Georgian works!",
    lead: "You're the first to confirm it on {model}. Share the result so every owner knows.",
    resultTitle: "Your result",
    showed: "The watch showed",
    account: "Needs a free GitHub account. Nothing is sent until you press Submit.",
  },
} as const;

type Widen<T> = T extends string ? string : { readonly [K in keyof T]: Widen<T[K]> };
export type CopyB = Widen<typeof en>;

const ka: CopyB = {
  toolbar: {
    steps: "ნაბიჯი {n} 4-დან",
  },
  connect: {
    lead: "Garmin-ის საათზე ქართული შეტყობინებები კვადრატებად ჩანს. პროგრამა საათის საკუთარ შრიფტს ქართულ ასოებს დაამატებს.",
    help: {
      express: {
        macos: { title: "დახურეთ Garmin Express", body: "მენიუს ზოლში დააჭირეთ მის ხატულას და აირჩიეთ Quit." },
        windows: { title: "დახურეთ Garmin Express", body: "საათის გვერდით მის ხატულაზე დააჭირეთ მარჯვენა ღილაკით და აირჩიეთ Exit." },
        linux: { title: "ამოიღეთ საათი ფაილების მენეჯერში", body: "საათს ერთდროულად მხოლოდ ერთი პროგრამა იყენებს." },
      },
      replug: { title: "ხელახლა შეაერთეთ საათი", body: "თუ საათი შეერთების ხერხს გკითხავთ, აირჩიეთ MTP." },
      cable: { title: "სცადეთ სხვა კაბელი", body: "ზოგი USB კაბელი მხოლოდ მუხტავს." },
    },
    preview: {
      connect: "შეაერთეთ საათი ამ კომპიუტერთან.",
      install: "პროგრამა საათის შრიფტს ქართულს დაამატებს.",
      restart: "საათი ახალ შრიფტს ჩატვირთავს.",
      check: "გაიგზავნეთ სატესტო შეტყობინება და ნახეთ შედეგი.",
    },
  },
  watch: {
    title: "საათი ქართულისთვის მზადაა",
    rows: { watch: "საათი", software: "სისტემის ვერსია", georgian: "ქართული" },
    status: {
      confirmed: "დადასტურებულია",
      confirmed_other_software: "დადასტურებულია სხვა ვერსიაზე",
      confirmed_some_fonts: "ნაწილობრივ დადასტურებულია",
      unconfirmed: "ჯერ დაუდასტურებელია",
    },
    safe: "თუ ქართული არ გამოჩნდა, ერთი დაჭერით დააბრუნებთ Garmin-ის შრიფტს.",
  },
  restart: {
    lead: "ქართული უკვე საათზეა და გადატვირთვის შემდეგ გამოჩნდება.",
    hold: { title: "ხანგრძლივად დააჭირეთ ჩართვის ღილაკს", body: "აირჩიეთ Power Off და შემდეგ ისევ ჩართეთ საათი." },
    reconnect: { title: "ისევ შეაერთეთ კომპიუტერთან", body: "პროგრამა შეამოწმებს, ჩატვირთა თუ არა საათმა შრიფტი." },
    waiting: "საათს ველოდებით…",
    skip: "საათზე თავად შევამოწმებ",
  },
  check: {
    optional: "არასავალდებულო",
    viewsLegend: "სად გამოჩნდა ასოები?",
    views: { popup: "როცა მოვიდა", list: "შეტყობინებების სიაში", opened: "როცა გავხსენი" },
    phoneLegend: "თქვენი ტელეფონი",
  },
  outcome: {
    title: "ქართული მუშაობს!",
    lead: "{model}-ზე ამას პირველი თქვენ ადასტურებთ. გააზიარეთ შედეგი, რომ ყველა მფლობელმა გაიგოს.",
    resultTitle: "თქვენი შედეგი",
    showed: "საათმა აჩვენა",
    account: "საჭიროა უფასო GitHub ანგარიში. სანამ Submit-ს არ დააჭერთ, არაფერი გაიგზავნება.",
  },
};

export const copyB: Record<Lang, CopyB> = { en, ka };
