// Minimal reactive store — the lightweight state pattern for views, no
// framework. Views today keep state in closures inside `renderX`; this store
// formalizes the same model for NEW or refactored state so cross-component
// state (session, active rubro, open cash session) stops being threaded by
// hand through every render call.
//
// Usage:
//
//   interface SessionState { userId: string; roles: string[] }
//   export const session = createStore<SessionState | null>(null);
//
//   session.set({ userId: "user:abc", roles: ["owner"] });
//   const unsub = session.subscribe((s) => renderBadge(s));
//   session.get()?.userId;   // read without subscribing

export interface Store<T> {
  /** Current value (same reference until `set` replaces it). */
  get(): T;
  /** Replace the value and notify subscribers. No-op when `Object.is` equal. */
  set(next: T): void;
  /** Merge-style update from the current value. */
  update(fn: (prev: T) => T): void;
  /** Listen for changes. Returns an unsubscribe function. */
  subscribe(fn: (value: T) => void): () => void;
}

export function createStore<T>(initial: T): Store<T> {
  let value = initial;
  const listeners = new Set<(value: T) => void>();
  return {
    get: () => value,
    set(next) {
      if (Object.is(next, value)) return;
      value = next;
      for (const fn of listeners) fn(value);
    },
    update(fn) {
      this.set(fn(value));
    },
    subscribe(fn) {
      listeners.add(fn);
      return () => {
        listeners.delete(fn);
      };
    },
  };
}
