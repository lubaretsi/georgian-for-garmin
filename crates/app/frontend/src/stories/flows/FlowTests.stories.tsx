// The flows of contract §8 as tests: the whole app over the simulated watch, each walked end to
// end by its play function, in Chromium and WebKit. Hidden from Storybook's sidebar (`!dev`);
// the clickable flows, starting at the beginning, are Flows.stories.tsx.

import type { Meta, StoryObj } from "@storybook/react-vite";
import axe from "axe-core";
import { expect, userEvent, waitFor } from "storybook/test";

import type { MockOptions } from "../../mock/MockBackend";
import { FlowStory, type FlowParts as Parts, parts, STEP_MS } from "./FlowStory";

type Args = { options: MockOptions };

export default {
  title: "Flows/Tests",
  tags: ["!dev"],
  render: (args, { globals }) => <FlowStory options={args.options} globals={globals} />,
  parameters: { frame: false, layout: "padded" },
  // The a11y addon checks the story as it first renders; a flow ends on another screen, so axe
  // checks that one too, and fails on "needs review" items as well as violations.
  afterEach: async ({ canvasElement }) => {
    const results = await axe.run(canvasElement, { resultTypes: ["violations", "incomplete"] });
    const found = [...results.violations, ...results.incomplete].map(
      (r) => `${r.id}: ${r.nodes.map((n) => n.target.join(" ")).join("; ")}`,
    );
    expect(found).toEqual([]);
  },
} satisfies Meta<Args>;

type Story = StoryObj<Args>;

/** A regular expression matching `text` literally. */
const literally = (text: string) => new RegExp(text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));

async function simulate(p: Parts, name: string) {
  await userEvent.click(await p.sim.findByRole("button", { name }));
}

async function press(p: Parts, name: string) {
  await userEvent.click(await p.button(name));
}

/** Install from the watch screen, then unplug, restart and reconnect the watch. */
async function installAndRestart(p: Parts) {
  await press(p, p.copy.actions.install);
  await p.screen(p.copy.restart.unplug.title);
  await simulate(p, "Unplug");
  await p.screen(p.copy.restart.restart.title);
  await simulate(p, "Restart watch");
  await simulate(p, "Plug in (MTP)");
}

/** From the result screen: check, send the test, and answer what the watch showed. */
async function checkAndAnswer(p: Parts, result: keyof Parts["copy"]["check"]["answer"]["results"]) {
  await press(p, p.copy.actions.check);
  await p.screen(p.copy.check.send.title);
  await press(p, p.copy.actions.sentIt);
  await p.screen(p.copy.check.answer.title);
  await userEvent.click(await p.app.findByRole("radio", { name: literally(p.copy.check.answer.results[result]) }));
  await press(p, p.copy.actions.continue);
}

/** The step indicator marks `label` as the current step. */
async function currentStep(p: Parts, label: string) {
  const steps = await p.app.findByRole("navigation", { name: p.copy.steps.label }, { timeout: STEP_MS });
  await waitFor(() => expect(steps.querySelector('[aria-current="step"]')).toHaveTextContent(label), {
    timeout: STEP_MS,
  });
}

async function shareOnGitHub(p: Parts) {
  await press(p, p.copy.actions.openGithub);
  await p.screen(p.copy.share.opened.title);
  await waitFor(() => expect(p.sim.getByTestId("sim-opened")).toHaveTextContent("github.com"), { timeout: STEP_MS });
}

export const Venu2Confirmed: Story = {
  args: { options: { watch: "venu2", connection: "unplugged" } },
  play: async ({ canvasElement, globals }) => {
    const p = parts(canvasElement, globals);
    await p.screen(p.copy.connect.searching.title);
    await simulate(p, "Plug in (MTP)");
    await p.screen(p.copy.watch.install.titleConfirmed);
    await installAndRestart(p);
    await p.screen(p.copy.result.install.title);
    // Confirmed: the check is optional and nothing asks to share.
    await p.button(p.copy.actions.sendTest);
    await expect(p.app.queryByRole("button", { name: p.copy.actions.shareResult })).toBeNull();
  },
};

export const Lily2ActiveFirstConfirmation: Story = {
  args: { options: { watch: "lily2active", connection: "unplugged" } },
  play: async ({ canvasElement, globals }) => {
    const p = parts(canvasElement, globals);
    await simulate(p, "Plug in (MTP)");
    await p.screen(p.copy.watch.install.title);
    await installAndRestart(p);
    await p.screen(p.copy.result.install.title);
    await checkAndAnswer(p, "georgian");
    await p.screen(p.fmt(p.copy.outcome.works_first.title, { model: "Lily 2 Active" }));
    await press(p, p.copy.actions.shareResult);
    await p.screen(p.copy.share.title.result);
    await shareOnGitHub(p);
  },
};

export const Instinct3Boxes: Story = {
  args: { options: { watch: "instinct3" } },
  play: async ({ canvasElement, globals }) => {
    const p = parts(canvasElement, globals);
    await p.screen(p.copy.watch.install.title);
    await installAndRestart(p);
    await p.screen(p.copy.result.install.title);
    await checkAndAnswer(p, "boxes");
    await p.screen(p.copy.outcome.didnt_show.title);
    await press(p, p.copy.actions.restartAgain);
    await p.screen(p.copy.restart.unplug.title);
    await simulate(p, "Unplug");
    await p.screen(p.copy.restart.restart.title);
    await p.app.findByText(p.fmt(p.copy.restart.attempt, { attempt: 2 }));
    await simulate(p, "Restart watch");
    await simulate(p, "Plug in (MTP)");
    await p.screen(p.copy.result.install.title);
    await checkAndAnswer(p, "boxes");
    await p.screen(p.copy.outcome.didnt_show.title);
    // Two restarts didn't help: sharing comes first now.
    await press(p, p.copy.actions.shareResult);
    await p.screen(p.copy.share.title.result);
    await shareOnGitHub(p);
  },
};

export const Venu3AnotherFontWaiting: Story = {
  args: { options: { watch: "venu3" } },
  play: async ({ canvasElement, globals }) => {
    const p = parts(canvasElement, globals);
    await p.screen(p.copy.watch.blocked.waiting_other_font.title);
    // The watch restarts while plugged in: it drops off USB and comes back.
    await simulate(p, "Restart watch");
    await p.screen(p.copy.watch.install.title);
    await press(p, p.copy.actions.install);
    await p.screen(p.copy.restart.unplug.title);
    await simulate(p, "Unplug");
    await p.screen(p.copy.restart.restart.title);
    // Its fonts don't show the change to the computer: the user checks on the watch.
    await p.app.findByText(p.copy.restart.restart.notVerifiable);
    await press(p, p.copy.actions.check);
    await p.screen(p.copy.check.send.title);
  },
};

export const Fenix6Unsupported: Story = {
  args: { options: { watch: "fenix6" } },
  play: async ({ canvasElement, globals }) => {
    const p = parts(canvasElement, globals);
    await p.screen(p.fmt(p.copy.watch.unsupported.title, { model: "fenix 6S Pro" }));
    await press(p, p.copy.actions.followUpdates);
    await waitFor(() => expect(p.sim.getByTestId("sim-opened")).toHaveTextContent("/issues/6"), { timeout: STEP_MS });
  },
};

export const Fenix6EarlierTestVersionLeft: Story = {
  args: { options: { watch: "fenix6BetaLeft" } },
  play: async ({ canvasElement, globals }) => {
    const p = parts(canvasElement, globals);
    const title = p.fmt(p.copy.watch.unsupported.title, { model: "fenix 6S Pro" });
    await p.screen(title);
    await press(p, p.copy.actions.restore);
    await p.screen(p.copy.confirmRestore.title);
    await press(p, p.copy.actions.restore);
    await p.screen(p.copy.restart.unplug.title);
    await simulate(p, "Unplug");
    await p.screen(p.copy.restart.restart.title);
    // TrueType fonts: the restart can't be checked from the computer.
    await press(p, p.copy.actions.continue);
    await simulate(p, "Restart watch");
    await simulate(p, "Plug in (MTP)");
    await p.screen(title);
  },
};

export const Forerunner245DriveOnly: Story = {
  args: { options: { watch: "forerunner245", connection: "drive" } },
  play: async ({ canvasElement, globals }) => {
    const p = parts(canvasElement, globals);
    await p.screen(p.fmt(p.copy.drive.title, { model: "Forerunner 245" }));
    await press(p, p.copy.actions.followUpdates);
    await waitFor(() => expect(p.sim.getByTestId("sim-opened")).toHaveTextContent("/issues/16"), { timeout: STEP_MS });
    // The support list knows this watch (report_wanted false): no font list is asked for.
    await expect(p.app.queryByRole("button", { name: p.copy.actions.shareFontList })).toBeNull();
  },
};

export const Forerunner265NoFont: Story = {
  args: { options: { watch: "forerunner265" } },
  play: async ({ canvasElement, globals }) => {
    const p = parts(canvasElement, globals);
    await p.screen(p.fmt(p.copy.watch.noFont.title, { model: "Forerunner 265" }));
    await press(p, p.copy.actions.shareFontList);
    await p.screen(p.copy.share.title.font_list);
    await shareOnGitHub(p);
  },
};

export const GarminExpressBusy: Story = {
  args: { options: { watch: "venu2", holder: "garmin_express" } },
  play: async ({ canvasElement, globals }) => {
    const p = parts(canvasElement, globals);
    await p.screen(p.fmt(p.copy.connect.busy.title.confirmed, { program: p.copy.connect.busy.program.garminExpress }));
    await simulate(p, "Release");
    await p.screen(p.copy.watch.install.titleConfirmed);
  },
};

export const InstallFailsRollbackIncomplete: Story = {
  args: { options: { watch: "venu2" } },
  play: async ({ canvasElement, globals }) => {
    const p = parts(canvasElement, globals);
    await p.screen(p.copy.watch.install.titleConfirmed);
    await simulate(p, "Make the next write fail");
    await press(p, p.copy.actions.install);
    await p.screen(p.copy.error.device_error.title);
    // Files may have been left behind: restoring is the main action.
    await press(p, p.copy.actions.restore);
    await p.screen(p.copy.confirmRestore.title);
    await press(p, p.copy.actions.restore);
    await p.screen(p.copy.restart.unplug.title);
    await simulate(p, "Unplug");
    await p.screen(p.copy.restart.restart.title);
    await simulate(p, "Restart watch");
    await simulate(p, "Plug in (MTP)");
    await p.screen(p.copy.result.restore.title);
  },
};

export const LinuxPermissionDenied: Story = {
  args: { options: { watch: "venu2", platform: "linux", problem: "connect-problems/inspect-permission_denied" } },
  globals: { platform: "linux" },
  play: async ({ canvasElement, globals }) => {
    const p = parts(canvasElement, globals);
    await p.screen(p.copy.connect.problem.permission_denied.title);
    await p.button(p.copy.actions.copyCommands);
  },
};

export const SecondRestartNeeded: Story = {
  args: { options: { watch: "venu2" } },
  play: async ({ canvasElement, globals }) => {
    const p = parts(canvasElement, globals);
    await p.screen(p.copy.watch.install.titleConfirmed);
    await press(p, p.copy.actions.install);
    await p.screen(p.copy.restart.unplug.title);
    await simulate(p, "Next restart doesn't import");
    await simulate(p, "Unplug");
    await p.screen(p.copy.restart.restart.title);
    await simulate(p, "Restart watch");
    await simulate(p, "Plug in (MTP)");
    await p.screen(p.copy.restart.again.title);
    await simulate(p, "Unplug");
    await p.screen(p.copy.restart.restart.title);
    await p.app.findByText(p.fmt(p.copy.restart.attempt, { attempt: 2 }));
    await simulate(p, "Restart watch");
    await simulate(p, "Plug in (MTP)");
    await p.screen(p.copy.result.install.title);
  },
};

export const GarminExpressGrabsItAfterTheRestart: Story = {
  args: { options: { watch: "venu2" } },
  play: async ({ canvasElement, globals }) => {
    const p = parts(canvasElement, globals);
    await p.screen(p.copy.watch.install.titleConfirmed);
    await press(p, p.copy.actions.install);
    await p.screen(p.copy.restart.unplug.title);
    await simulate(p, "Unplug");
    await p.screen(p.copy.restart.restart.title);
    await simulate(p, "Restart watch");
    await simulate(p, "Garmin Express holds it");
    await simulate(p, "Plug in (MTP)");
    await p.screen(p.fmt(p.copy.connect.busy.title.confirmed, { program: p.copy.connect.busy.program.garminExpress }));
    await currentStep(p, p.copy.steps.restart);
    // Released: the restart's check goes on.
    await simulate(p, "Release");
    await p.screen(p.copy.result.install.title);
  },
};

export const BackAsADriveAfterTheRestart: Story = {
  args: { options: { watch: "venu2" } },
  play: async ({ canvasElement, globals }) => {
    const p = parts(canvasElement, globals);
    await p.screen(p.copy.watch.install.titleConfirmed);
    await press(p, p.copy.actions.install);
    await p.screen(p.copy.restart.unplug.title);
    await simulate(p, "Unplug");
    await p.screen(p.copy.restart.restart.title);
    await simulate(p, "Restart watch");
    await simulate(p, "Plug in (drive)");
    await p.screen(p.fmt(p.copy.drive.title, { model: "Venu 2" }));
    await currentStep(p, p.copy.steps.restart);
    // Switched to MTP: the restart's check goes on.
    await simulate(p, "Plug in (MTP)");
    await p.screen(p.copy.result.install.title);
  },
};
