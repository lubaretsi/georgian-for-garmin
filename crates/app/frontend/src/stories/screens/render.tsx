import type { StoryObj } from "@storybook/react-vite";
import { useGlobals } from "storybook/preview-api";

import { storyGlobals } from "../../../.storybook/globals";
import { ScreenStory, type ScreenStoryArgs } from "./ScreenStory";

/** Every Screens/… story's render: the toolbar's language and platform, unless pinned. */
export function renderScreen(args: ScreenStoryArgs) {
  const [globals, updateGlobals] = useGlobals();
  const g = storyGlobals(globals);
  return (
    <ScreenStory
      {...args}
      lang={g.lang}
      platform={args.platform ?? g.platform}
      onLanguage={(lang) => updateGlobals({ lang })}
    />
  );
}

export type ScreenStoryObj = StoryObj<ScreenStoryArgs>;
