import { ChevronRight } from "lucide-react";
import { Collapsible } from "radix-ui";
import type { ReactNode } from "react";

/**
 * A collapsed section, opened by its summary line ("Details", "Show the exact text"). The
 * summary is a real button with its expanded state (inside an h2 when it names a section);
 * the content mounts only when open.
 */
export function Disclosure(props: { summary: string; heading?: boolean; defaultOpen?: boolean; children: ReactNode }) {
  const trigger = (
    <Collapsible.Trigger className="flex items-center gap-1 rounded-[6px] px-3.5 text-label font-semibold text-fg-2 hover:text-fg">
      {props.summary}
      <ChevronRight
        size={14}
        strokeWidth={2.4}
        aria-hidden="true"
        className="transition-transform duration-200 group-data-[state=open]/disclosure:rotate-90"
      />
    </Collapsible.Trigger>
  );
  return (
    <Collapsible.Root defaultOpen={props.defaultOpen} className="group/disclosure">
      {props.heading ? <h2>{trigger}</h2> : trigger}
      <Collapsible.Content className="mt-2">{props.children}</Collapsible.Content>
    </Collapsible.Root>
  );
}
