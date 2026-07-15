// Convert backend failures into user-safe text. Request URLs can carry Plex
// tokens and server-local identifiers, so every user-visible error surface
// shares this single redaction funnel.
export function friendlyError(error: string): string {
  return error
    .replaceAll(
      "RECONNECT_REQUIRED",
      "A server needs reconnecting — open Settings (⚙) and reconnect it.",
    )
    .replace(
      /error sending request for url \([^)]*\)/gi,
      "the server could not be reached",
    )
    .replace(/(https?:\/\/[^\s)?]+)\?[^\s)]*/gi, "$1");
}
