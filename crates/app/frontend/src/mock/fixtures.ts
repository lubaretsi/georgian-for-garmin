// The core's real results for each flow scenario, written by crates/core/tests/ui_fixtures.rs
// to fixtures/core/<scenario>/<step>.json (regenerate: `GFG_WRITE_UI_FIXTURES=1 cargo test -p
// georgian-for-garmin-core --test ui_fixtures`). Each file is one call: its operation, the input
// where it had one, and the output or the error as the app hands it to the frontend.

import type {
  Failure,
  InspectDriveOutput,
  InspectOutput,
  IssueLink,
  IssueLinkInput,
  StageOutput,
  SwitchOutput,
} from "../core/types.gen";

type Outputs = {
  inspect: InspectOutput;
  install: StageOutput;
  restore_stock: StageOutput;
  inspect_drive: InspectDriveOutput;
  issue_link: IssueLink;
  switch_to_mtp: SwitchOutput;
};
export type Operation = keyof Outputs;

export type FixtureFile = {
  [K in Operation]: {
    operation: K;
    input?: K extends "issue_link" ? IssueLinkInput : unknown;
  } & ({ output: Outputs[K]; error?: undefined } | { error: Failure; output?: undefined });
}[Operation];

export type Fixture = { scenario: string; step: string; file: FixtureFile };

const modules = import.meta.glob<FixtureFile>("../../fixtures/core/*/*.json", {
  eager: true,
  import: "default",
});

/** Every fixture, ordered by scenario and step. */
export const fixtures: Fixture[] = Object.entries(modules)
  .map(([path, file]) => {
    const match = /\/fixtures\/core\/([^/]+)\/([^/]+)\.json$/.exec(path);
    if (!match?.[1] || !match[2]) throw new Error(`unexpected fixture path ${path}`);
    return { scenario: match[1], step: match[2], file };
  })
  .sort((a, b) => (a.scenario + "/" + a.step).localeCompare(b.scenario + "/" + b.step));

export function fixtureFile(scenario: string, step: string): FixtureFile {
  const found = fixtures.find((f) => f.scenario === scenario && f.step === step);
  if (!found) throw new Error(`no fixture ${scenario}/${step}`);
  return found.file;
}

/** The output of `scenario/step`, which must be a successful `operation`. */
export function output<O extends Operation>(scenario: string, step: string, operation: O): Outputs[O] {
  const file = fixtureFile(scenario, step);
  if (file.operation !== operation || file.output === undefined) {
    throw new Error(`${scenario}/${step} is no successful ${operation}`);
  }
  return file.output as Outputs[O];
}

/** The error of `scenario/step`, which must have failed. */
export function failure(scenario: string, step: string): Failure {
  const file = fixtureFile(scenario, step);
  if (!file.error) throw new Error(`${scenario}/${step} didn't fail`);
  return file.error;
}

/** Every successful `inspect` fixture: what `verdictOf` is tested against. */
export function inspectFixtures(): { name: string; output: InspectOutput }[] {
  return fixtures.flatMap(({ scenario, step, file }) =>
    file.operation === "inspect" && file.output ? [{ name: `${scenario}/${step}`, output: file.output }] : [],
  );
}
