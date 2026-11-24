// The check screen's answers are the core's `Seen` values, in the issue form's order.

import { readFileSync } from "node:fs";

import { expect, test } from "vitest";

import { OFFERED } from "./Check";

test("the answers follow the core's order, without the flow's own not_installed", () => {
  // types.gen.ts keeps the schema's order, which is `Seen::ALL`'s, the form's.
  const generated = readFileSync(new URL("../core/types.gen.ts", import.meta.url), "utf8");
  const union = /^export type Seen = (.*);$/m.exec(generated)?.[1];
  expect(union, "types.gen.ts declares Seen").toBeDefined();
  const seen = [...(union ?? "").matchAll(/"(\w+)"/g)].map((m) => m[1]);
  expect(OFFERED).toEqual(seen.filter((s) => s !== "not_installed"));
});
