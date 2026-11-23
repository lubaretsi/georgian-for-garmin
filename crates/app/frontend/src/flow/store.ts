// The flow's state outside React: each event goes through the reducer, and the transition's
// effects run right away, so no effect is lost when React batches renders.

import { useSyncExternalStore } from "react";

import type { Backend, Lang } from "../core/backend";
import { EffectRunner } from "./effects";
import { type FlowEvent, type FlowState, initialState, reduce } from "./state";

export class FlowStore {
  private state: FlowState;
  private readonly listeners = new Set<() => void>();
  private readonly runner: EffectRunner;
  private started = false;

  /** `speed` divides every delay (the story tests run faster). */
  constructor(backend: Backend, lang: Lang, version: string, speed = 1) {
    this.state = initialState(lang, version);
    this.runner = new EffectRunner(backend, this.dispatch, speed);
  }

  getState = (): FlowState => this.state;

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  dispatch = (event: FlowEvent): void => {
    this.state = reduce(this.state, event);
    this.runner.run(this.state.effects);
    for (const listener of this.listeners) listener();
  };

  /** Starts looking for the watch; again after `stop`, with a fresh poll. */
  start(): void {
    if (this.started) return;
    this.started = true;
    this.runner.resume();
    this.dispatch({ type: "start" });
  }

  /** Stops every poll and timer. */
  stop(): void {
    this.started = false;
    this.runner.stop();
  }
}

export function useFlowState(store: FlowStore): FlowState {
  return useSyncExternalStore(store.subscribe, store.getState);
}
