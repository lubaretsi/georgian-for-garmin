import { X } from "lucide-react";
import { Dialog } from "radix-ui";
import type { ReactNode } from "react";

import { Button } from "./button";

/**
 * A bottom sheet over the app window (Radix Dialog: focus trap, Escape, focus return).
 * It portals into `container`, the app's own root, so it stays inside the window.
 */
export function Sheet(props: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  closeLabel: string;
  container: HTMLElement | null;
  children: ReactNode;
}) {
  return (
    <Dialog.Root open={props.open} onOpenChange={props.onOpenChange}>
      <Dialog.Portal container={props.container}>
        <Dialog.Overlay className="wa:absolute wa:inset-0 wa:z-40 wa:bg-[rgb(30_18_12/0.38)] wa:backdrop-blur-[2px] wa:data-[state=open]:animate-[a-fade_200ms_ease-out]" />
        <Dialog.Content
          aria-describedby={undefined}
          className="wa:absolute wa:inset-x-0 wa:bottom-0 wa:z-50 wa:flex wa:max-h-[88%] wa:flex-col wa:rounded-t-[24px] wa:bg-paper wa:shadow-raised wa:data-[state=open]:animate-[a-sheet-in_280ms_cubic-bezier(0.22,1,0.36,1)]"
        >
          <div aria-hidden className="wa:mx-auto wa:mt-2.5 wa:h-1.5 wa:w-10 wa:rounded-full wa:bg-line" />
          <div className="wa:flex wa:items-center wa:gap-3 wa:px-5 wa:pt-2 wa:pb-3">
            <Dialog.Title className="wa:flex-1 wa:text-[19px] wa:font-semibold">{props.title}</Dialog.Title>
            <Dialog.Close asChild>
              <Button variant="quiet" size="icon" aria-label={props.closeLabel}>
                <X />
              </Button>
            </Dialog.Close>
          </div>
          <div className="wa:grid wa:gap-4 wa:overflow-y-auto wa:px-5 wa:pb-6">{props.children}</div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
