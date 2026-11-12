import type { ServerPlaylist, ServerPlaylistGroup } from "./types";

export type ServerPlaylistEntry = {
  playlist: ServerPlaylist;
  sourceId: string;
  sourceName: string;
  sourceKind: string;
};

// The sidebar lists server playlists flat, in server order then each server's
// own order. Owner ruling 2026-11-12: "if there are no available server
// playlists, show nothing for that server" — so a server that is unavailable,
// or has none, contributes no entry, and an empty result means no section.
export function flattenServerPlaylists(groups: ServerPlaylistGroup[]): ServerPlaylistEntry[] {
  return groups
    .filter((group) => group.available)
    .flatMap((group) =>
      group.playlists.map((playlist) => ({
        playlist,
        sourceId: group.sourceId,
        sourceName: group.sourceName,
        sourceKind: group.sourceKind,
      })),
    );
}

// The one place a server kind is turned into words.
export const KIND_LABELS: Record<string, string> = {
  plex: "Plex",
  jellyfin: "Jellyfin",
  emby: "Emby",
};

// An unknown kind is returned as stored, so a future backend degrades to its
// raw name.
export function kindLabel(kind: string): string {
  return KIND_LABELS[kind] ?? kind;
}

// A server's name for plain strings, where no mark can be drawn: "altiera
// (Jellyfin)". An unknown or missing kind leaves the bare name.
export function serverLabel(name: string, kind: string | null | undefined): string {
  const label = kind ? KIND_LABELS[kind] : undefined;
  return label ? `${name} (${label})` : name;
}
