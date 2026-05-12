import { PlayerSession } from "@/features/player/services/playerSession";

export type ContinueWatchingMediaType = "movie" | "show_episode" | "standup" | "documentary";

export type ContinueWatchingEntry = {
  entryId: string;
  mediaId: string;
  mediaTitle: string;
  mediaPath: string;
  posterPath: string | null;
  subtitlePath: string | null;
  mediaType: ContinueWatchingMediaType;
  progressSec: number;
  durationSec: number;
  updatedAt: number;
  parentMediaId: string | null;
  parentTitle: string | null;
  episodeTitle: string | null;
  seasonNumber: number | null;
  episodeNumber: number | null;
};

type SupportedPlayerSession = PlayerSession & {
  mediaType: ContinueWatchingMediaType;
};

export const CONTINUE_WATCHING_STORAGE_KEY = "netcrico.player.continue-watching";

const MIN_PROGRESS_SECONDS = 5;
const COMPLETION_THRESHOLD = 0.96;
const CONTINUE_WATCHING_MEDIA_TYPES: ContinueWatchingMediaType[] = ["movie", "show_episode", "standup", "documentary"];

export function supportsContinueWatching(session: PlayerSession | null | undefined): session is SupportedPlayerSession {
  return Boolean(session?.mediaType && CONTINUE_WATCHING_MEDIA_TYPES.includes(session.mediaType as ContinueWatchingMediaType));
}

export function getContinueWatchingEntryId(session: Pick<PlayerSession, "mediaId" | "mediaType" | "parentMediaId">) {
  if (session.mediaType === "show_episode" && session.parentMediaId) {
    return session.parentMediaId;
  }

  return session.mediaId;
}

export function shouldRemoveContinueWatching(progressSec: number, durationSec: number) {
  if (durationSec <= 0) {
    return false;
  }

  return progressSec / durationSec >= COMPLETION_THRESHOLD;
}

export function createContinueWatchingEntry(
  session: PlayerSession,
  progressSec: number,
  durationSec: number
): ContinueWatchingEntry | null {
  if (!supportsContinueWatching(session) || progressSec < MIN_PROGRESS_SECONDS) {
    return null;
  }

  return {
    entryId: getContinueWatchingEntryId(session),
    mediaId: session.mediaId,
    mediaTitle: session.mediaTitle,
    mediaPath: session.mediaPath,
    posterPath: session.posterPath,
    subtitlePath: session.subtitlePath,
    mediaType: session.mediaType,
    progressSec,
    durationSec,
    updatedAt: Date.now(),
    parentMediaId: session.parentMediaId ?? null,
    parentTitle: session.parentTitle ?? null,
    episodeTitle: session.episodeTitle ?? null,
    seasonNumber: session.seasonNumber ?? null,
    episodeNumber: session.episodeNumber ?? null
  };
}

export function toPlayerSessionFromContinueWatching(entry: ContinueWatchingEntry): PlayerSession {
  return {
    mediaId: entry.mediaId,
    mediaTitle: entry.mediaTitle,
    mediaPath: entry.mediaPath,
    posterPath: entry.posterPath,
    subtitlePath: entry.subtitlePath,
    mediaType: entry.mediaType,
    resumePositionSec: entry.progressSec,
    parentMediaId: entry.parentMediaId,
    parentTitle: entry.parentTitle,
    episodeTitle: entry.episodeTitle,
    seasonNumber: entry.seasonNumber,
    episodeNumber: entry.episodeNumber
  };
}

export function readStoredContinueWatching(): ContinueWatchingEntry[] {
  if (typeof window === "undefined") {
    return [];
  }

  const rawValue = window.localStorage.getItem(CONTINUE_WATCHING_STORAGE_KEY);

  if (!rawValue) {
    return [];
  }

  try {
    const parsedValue = JSON.parse(rawValue);

    if (!Array.isArray(parsedValue)) {
      window.localStorage.removeItem(CONTINUE_WATCHING_STORAGE_KEY);
      return [];
    }

    return parsedValue
      .filter((entry): entry is ContinueWatchingEntry => Boolean(entry && typeof entry === "object" && typeof entry.entryId === "string"))
      .sort(compareContinueWatchingEntries);
  } catch {
    window.localStorage.removeItem(CONTINUE_WATCHING_STORAGE_KEY);
    return [];
  }
}

export function persistContinueWatching(entries: ContinueWatchingEntry[]) {
  if (typeof window === "undefined") {
    return;
  }

  if (!entries.length) {
    window.localStorage.removeItem(CONTINUE_WATCHING_STORAGE_KEY);
    return;
  }

  window.localStorage.setItem(CONTINUE_WATCHING_STORAGE_KEY, JSON.stringify(entries.sort(compareContinueWatchingEntries)));
}

function compareContinueWatchingEntries(left: ContinueWatchingEntry, right: ContinueWatchingEntry) {
  return right.updatedAt - left.updatedAt;
}