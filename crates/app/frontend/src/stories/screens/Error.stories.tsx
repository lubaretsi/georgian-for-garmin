import type { Meta } from "@storybook/react-vite";
import { fn } from "storybook/test";

import type { ErrorCode } from "../../core/types.gen";
import { failure } from "../../mock/fixtures";
import type { RollbackView, WatchInfo } from "../../model/screens";
import { rollbackFrom, WATCHES } from "./data";
import { renderScreen, type ScreenStoryObj } from "./render";
import type { ScreenStoryArgs } from "./ScreenStory";

/** An error screen from a failure fixture: its code, message and rollback. */
const fromFixture = (scenario: string, step: string, op: "install" | "restore", watch: WatchInfo = WATCHES.venu2) => {
  const f = failure(scenario, step);
  return error(f.code, op, watch, f.rollback ? rollbackFrom(scenario, step) : null, f.message);
};

const error = (code: ErrorCode, op: "install" | "restore", watch: WatchInfo, rollback: RollbackView | null, detail: string) =>
  ({ screen: "error", op, watch, code, rollback, detail }) as const;

export default {
  title: "Screens/Error",
  render: renderScreen,
  args: { onAction: fn(), model: fromFixture("errors", "install-conflict", "install") },
} satisfies Meta<ScreenStoryArgs>;

export const Conflict: ScreenStoryObj = { args: { model: fromFixture("errors", "install-conflict", "install") } };
export const RestoreFirst: ScreenStoryObj = {
  args: { model: fromFixture("errors", "install-restore_first", "install", WATCHES.forerunner165), restorable: true },
};
export const VerificationFailed: ScreenStoryObj = {
  args: { model: fromFixture("errors", "install-verification_failed", "install"), restorable: true },
};
export const DeviceErrorRestore: ScreenStoryObj = {
  args: { model: fromFixture("errors", "restore_stock-device_error", "restore"), restorable: true },
};
export const NoDevice: ScreenStoryObj = { args: { model: fromFixture("errors", "install-no_device", "install") } };
export const UnsupportedDevice: ScreenStoryObj = {
  args: {
    model: error(
      "unsupported_device",
      "install",
      WATCHES.forerunner265,
      null,
      "The watch lists none of the bundled font packs.",
    ),
  },
};
export const Internal: ScreenStoryObj = { args: { model: fromFixture("errors", "install-internal", "install") } };
export const InvalidInput: ScreenStoryObj = { args: { model: fromFixture("errors", "install-invalid_input", "install") } };
export const FontUnavailable: ScreenStoryObj = {
  args: { model: fromFixture("errors", "install-font_unavailable", "install") },
};

/** Venu 2, install fails mid-write and everything is undone. */
export const RollbackComplete: ScreenStoryObj = {
  args: { model: fromFixture("venu2-write-fails", "install-rollback-complete", "install"), restorable: true },
};
/** Venu 2, install fails mid-write and two files can't be undone: restore comes first. */
export const RollbackIncomplete: ScreenStoryObj = {
  args: { model: fromFixture("venu2-write-fails", "install-rollback-incomplete", "install"), restorable: true },
};
