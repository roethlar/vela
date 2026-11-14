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

// A play's end, as `playback-ended` reports it. The first report of an end
// names its session and says whether the run is over: the play ended other
// than cleanly (an error, a quit that was not the page's, mpv dying) and
// nothing replaced it. A clean end is reported again once the backend has
// started any successor of its own; that second report names its session
// too, and says the run is over when the backend could not start the
// successor.
export type PlaybackEnd = { sessionId?: unknown; runOver?: unknown };

export type PlayEndContext = {
  /** The session whose view shows now. */
  showing: string | null;
  /** The ends reported since a view last opened or closed, this one included. */
  endsSinceViewChanged: number;
  /** Whether the page itself is deciding or starting a successor. */
  continuationPending: boolean;
};

/** Whether this end leaves the fullscreen Vela asked for on the player's behalf. */
export function endLeavesPlayerFullscreen(end: PlaybackEnd, ctx: PlayEndContext): boolean {
  if (end.runOver === true && typeof end.sessionId === "string") {
    // Unless another play's view shows by now.
    return ctx.showing === null || ctx.showing === end.sessionId;
  }
  // Otherwise the second report of a clean end comes after the backend's
  // successor (if any) has started, so one with no view showing and nothing
  // pending here means the run is over.
  return ctx.showing === null && ctx.endsSinceViewChanged >= 2 && !ctx.continuationPending;
}
