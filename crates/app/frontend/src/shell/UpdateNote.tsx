import { CircleArrowUp } from "lucide-react";
import { useId } from "react";

import { type Copy, fmt } from "../copy";
import { Button } from "../ui/Button";

/**
 * The update note (§12): "Version {version} is available", a quiet borderless button that
 * opens the release page in the browser. Pinned on every screen between the scrolling content
 * and the footer, and in More › About; never among the footer's actions, never modal.
 */
export function UpdateNote(props: { version: string; copy: Copy; onPress: () => void; className?: string }) {
  const id = useId();
  return (
    <div className={props.className}>
      <Button
        variant="toolbar"
        size="sm"
        aria-describedby={`${id}-detail`}
        onClick={props.onPress}
        className="text-fg-2 hover:text-fg"
      >
        <CircleArrowUp size={14} strokeWidth={1.9} aria-hidden="true" />
        {fmt(props.copy.update.available, { version: props.version })}
      </Button>
      <span id={`${id}-detail`} className="sr-only">
        {props.copy.update.download}
      </span>
    </div>
  );
}
