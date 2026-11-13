// Which playback session's picture is showing inside Vela's own window.
//
// The in-window player announces its own lifecycle with two events, each
// naming its session. Kept apart from the page so the one rule that matters
// can be tested on its own: when one play replaces another, the old play's
// "closed" arrives AFTER the new play's "opened", and must not dismiss the
// new play's view.

export type PlayerViewEvent = { kind: "opened" | "closed"; sessionId: string };

/** The session whose view shows after `event`, given the one showing before. */
export function nextPlayerSession(current: string | null, event: PlayerViewEvent): string | null {
  if (event.kind === "opened") return event.sessionId;
  return current === event.sessionId ? null : current;
}
