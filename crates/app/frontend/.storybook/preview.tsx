import type { Decorator, Preview } from "@storybook/react-vite";

import "../src/ui/global.css";
import { WindowFrame } from "../src/ui/WindowFrame";
import { initialGlobals, storyGlobals, windowSizeTitles } from "./globals";

/**
 * Frames every story at the toolbar's window size, theme and language. A story with
 * `parameters.frame === false` (the flows) places its own `WindowFrame` beside the simulator.
 */
const frame: Decorator = (Story, context) => {
  const { lang, theme, width, height, ...g } = storyGlobals(context.globals);
  // A story that pins its platform in args (busy × platform, Linux permissions) gets that window.
  const pinned: unknown = context.args.platform;
  const platform = pinned === "macos" || pinned === "windows" || pinned === "linux" ? pinned : g.platform;
  document.documentElement.dataset.theme = theme;
  document.documentElement.lang = lang;
  if (context.parameters.frame === false) return <Story />;
  return (
    <WindowFrame width={width} height={height} theme={theme} lang={lang} platform={platform}>
      <Story />
    </WindowFrame>
  );
};

const preview: Preview = {
  decorators: [frame],
  initialGlobals,
  globalTypes: {
    lang: {
      description: "Language",
      toolbar: {
        title: "Language",
        icon: "globe",
        items: [
          { value: "ka", title: "ქართული" },
          { value: "en", title: "English" },
        ],
        dynamicTitle: true,
      },
    },
    platform: {
      description: "Platform",
      toolbar: {
        title: "Platform",
        icon: "browser",
        items: [
          { value: "macos", title: "macOS" },
          { value: "windows", title: "Windows" },
          { value: "linux", title: "Linux" },
        ],
        dynamicTitle: true,
      },
    },
    theme: {
      description: "Theme",
      toolbar: {
        title: "Theme",
        icon: "contrast",
        items: [
          { value: "light", title: "Light" },
          { value: "dark", title: "Dark" },
        ],
        dynamicTitle: true,
      },
    },
    size: {
      description: "Window size",
      toolbar: {
        title: "Window size",
        icon: "grow",
        items: Object.entries(windowSizeTitles).map(([value, title]) => ({ value, title })),
        dynamicTitle: true,
      },
    },
  },
  parameters: {
    layout: "padded",
    // Any axe violation fails the story's test.
    a11y: { test: "error" },
  },
};

export default preview;
