// Shared frontend mirrors of the backend DTOs (camelCase over IPC).
// `Item` mirrors `ItemDto` (listing card); `Detail` mirrors `DetailDto`
// (the on-demand info surface, a superset of Item — every rich field is
// optional so a sparse backend degrades to a clean minimal page).

export type Item = {
  ratingKey: string;
  title: string;
  year?: number;
  summary?: string;
  poster?: string;
  seriesPoster?: string;
  backdrop?: string;
  mediaType?: string;
  durationMs?: number;
  viewOffsetMs?: number;
  grandparentTitle?: string;
  parentTitle?: string;
  // Namespaced container keys, when the backend exposes them: an episode's
  // season / a season's show (parent), an episode's show (grandparent).
  // Drive the info surface's season/show navigation.
  parentRatingKey?: string;
  grandparentRatingKey?: string;
  index?: number;
  parentIndex?: number;
  played?: boolean | null;
  lastWatchedAtMs?: number;
  sourceId?: string;
  backing?: { sourceId: string; ratingKey: string }[];
  canonicalId?: string;
  watchKey?: string;
  // Merged cards: where the detail surface / children drill should route
  // (the metadata-richest backing). Callers fall back to `ratingKey`.
  detailKey?: string;
};

export type CastMember = { name: string; role?: string; thumb?: string; personKey?: string };

// A person credit (director/writer); `personKey` (namespaced, when the
// backend identifies the person) is the person-browse query target.
export type PersonRef = { name: string; personKey?: string };

export type MediaStream = {
  streamType?: number; // 1 = video, 2 = audio, 3 = subtitle
  codec?: string;
  language?: string;
  channels?: number;
  displayTitle?: string;
};

export type MediaVersion = {
  videoResolution?: string;
  width?: number;
  height?: number;
  videoCodec?: string;
  audioCodec?: string;
  container?: string;
  hdr: boolean;
  streams?: MediaStream[];
};

export type Detail = {
  ratingKey: string;
  title: string;
  year?: number;
  summary?: string;
  tagline?: string;
  durationMs?: number;
  mediaType?: string;
  poster?: string;
  backdrop?: string;
  contentRating?: string;
  rating?: number;
  audienceRating?: number;
  studio?: string;
  originallyAvailableAt?: string;
  genres?: string[];
  directors?: PersonRef[];
  writers?: PersonRef[];
  countries?: string[];
  cast?: CastMember[];
  index?: number;
  parentIndex?: number;
  grandparentTitle?: string;
  parentTitle?: string;
  played?: boolean | null;
  viewOffsetMs?: number;
  media?: MediaVersion[];
  sourceId: string;
};

// The key the detail surface fetches/drills with: the metadata-richest
// backing of a merged card, else the item's own key.
export function detailKeyOf(item: Item): string {
  return item.detailKey ?? item.ratingKey;
}
