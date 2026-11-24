import type { Copy } from "../copy";
import { Button } from "../ui/Button";

/** What the demo build's watch does when a control is pressed (the app's `demo_watch` command). */
export type DemoWatchAction = "unplug" | "restart" | "plug" | "plug_drive" | "hold" | "release";

const ACTIONS: readonly DemoWatchAction[] = ["unplug", "restart", "plug", "plug_drive", "hold", "release"];

/**
 * The demo build's physical side, in a slim bar above the footer: the user unplugs, restarts and
 * plugs in the simulated watch, as with a real one, and lets Garmin Express hold it. The app
 * notices on its next poll. Only the demo build shows it (`demoMode()`).
 */
export function DemoWatchBar(props: { copy: Copy; inert?: boolean; onAction: (action: DemoWatchAction) => void }) {
  const d = props.copy.demoWatch;
  return (
    <section
      inert={props.inert}
      aria-label={d.label}
      className="relative z-20 flex shrink-0 flex-wrap items-center gap-x-2 gap-y-1.5 border-t border-dashed border-sep bg-canvas px-4 py-2"
    >
      <h2 className="mr-1 text-label font-semibold text-fg-2">{d.label}</h2>
      {ACTIONS.map((action) => (
        <Button key={action} variant="secondary" size="sm" onClick={() => props.onAction(action)}>
          {d[action]}
        </Button>
      ))}
    </section>
  );
}
