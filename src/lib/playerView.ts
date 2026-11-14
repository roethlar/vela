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

// A play's end, as `playback-ended` reports it. A play's own report of its
// end names its session and says whether the run is over: the play ended
// other than cleanly (an error, a quit that was not the page's, mpv dying)
// and nothing replaced it. The backend sends the same kind of report, saying
// the run is over, when a launch fails or a source choice ends without a play
// and nothing plays or is coming by then. A clean end is reported again by
// the completion dispatcher once it has dealt with any successor of its own;
// only that report carries `successor`, always as true or false: true when
// another play is active or one is being chosen. It also says the run is over
// when the successor could not be started or no play is active at all.
export type PlaybackEnd = { sessionId?: unknown; runOver?: unknown; successor?: unknown };

export type PlayEndContext = {
  /** The session whose view shows now. */
  showing: string | null;
  /** Whether a report has named the session whose view shows: its play has
   * ended, and its view is only waiting to close (`showingPlayEnded`). */
  showingEnded: boolean;
  /** Whether the page itself is deciding or starting a successor. */
  continuationPending: boolean;
};

/**
 * Whether the play whose view shows has ended, once `showing` is the view
 * showing: true from a report that names it until another view shows or none
 * does. Every report of an end comes after its play's player has ended, so a
 * view it names belongs to nothing that still plays. `previous` is the view
 * that showed before, and `ended` whether its play had ended.
 */
export function showingPlayEnded(
  previous: string | null,
  ended: boolean,
  showing: string | null,
  end?: PlaybackEnd,
): boolean {
  if (showing === null) return false;
  if (end && end.sessionId === showing) return true;
  return showing === previous && ended;
}

/** Whether this end leaves the fullscreen Vela asked for on the player's behalf. */
export function endLeavesPlayerFullscreen(end: PlaybackEnd, ctx: PlayEndContext): boolean {
  // No view of a play that still plays shows.
  const noLiveView = ctx.showing === null || ctx.showingEnded;
  if (end.runOver === true && typeof end.sessionId === "string") {
    // Unless another play's view shows by now.
    return noLiveView || ctx.showing === end.sessionId;
  }
  // The dispatcher's report that the run is neither over nor going on in a
  // successor of its own: the page decides what follows (Continue Playing).
  // With no live view and nothing pending here, nothing does.
  if (end.successor === false) return noLiveView && !ctx.continuationPending;
  // Otherwise the run goes on: a successor is coming (its view may not have
  // opened yet, or it plays in mpv's own window), the dispatcher will report
  // a clean end again, or another play replaced this one. Their own reports
  // end the run.
  return false;
}
