import type { Meta } from "@storybook/react-vite";
import { fn } from "storybook/test";

import type { Lang } from "../../core/backend";
import type { Answers, ScreenModel } from "../../model/screens";
import { markPictures, phone, WATCHES } from "./data";
import { renderScreen, type ScreenStoryObj } from "./render";
import type { ScreenStoryArgs } from "./ScreenStory";

const check =
  (step: "send" | "answer", answers: Answers = { views: [] }, extra: { phone?: boolean; marks?: boolean } = {}) =>
  (lang: Lang): ScreenModel => ({
    screen: "check",
    watch: WATCHES.lily2Active,
    step,
    answers,
    phone: extra.phone === false ? null : phone(lang),
    markPictures: extra.marks ? markPictures() : [],
  });

export default {
  title: "Screens/Check",
  render: renderScreen,
  args: { onAction: fn(), model: check("send") },
} satisfies Meta<ScreenStoryArgs>;

export const Send: ScreenStoryObj = { args: { model: check("send"), restorable: true } };

/** No phone page (it couldn't be made): the test text alone. */
export const SendTextOnly: ScreenStoryObj = { args: { model: check("send", { views: [] }, { phone: false }) } };

export const AnswerEmpty: ScreenStoryObj = { args: { model: check("answer") } };

export const AnswerAnswered: ScreenStoryObj = {
  args: { model: check("answer", { result: "georgian", views: ["popup", "list"], phone: "android" }) },
};

export const AnswerBoxes: ScreenStoryObj = {
  args: { model: check("answer", { result: "boxes", views: [], phone: "iphone" }) },
};

/**
 * A beta build: which marked capital the watch showed. The pictures are SVG stand-ins; the
 * core draws them as PNGs from the marked fonts.
 */
export const AnswerBetaMarker: ScreenStoryObj = {
  name: "Answer, beta marker (stand-in pictures)",
  args: { model: check("answer", { result: "georgian", views: ["popup"] }, { marks: true }) },
};
