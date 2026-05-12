import { HomeMediaItemDto, MusicTrackDto } from "@/types/contracts/library";

export type PlayerPlaylistTrack = {
  id: string;
  title: string;
  mediaPath: string;
  trackNumber: number | null;
  posterPath: string | null;
};

export type PlayerSession = {
  mediaId: string;
  mediaTitle: string;
  mediaPath: string;
  posterPath: string | null;
  subtitlePath: string | null;
  mediaType?: string;
  albumTitle?: string | null;
  artistTitle?: string | null;
  currentTrackId?: string | null;
  playlist?: PlayerPlaylistTrack[];
  resumePositionSec?: number | null;
  parentMediaId?: string | null;
  parentTitle?: string | null;
  episodeTitle?: string | null;
  seasonNumber?: number | null;
  episodeNumber?: number | null;
};

export const PLAYER_SESSION_STORAGE_KEY = "netcrico.player.session";

export function toPlayerSession(item: HomeMediaItemDto): PlayerSession {
  return {
    mediaId: item.id,
    mediaTitle: item.title,
    mediaPath: item.mediaPath,
    posterPath: item.posterPath ?? item.coverPath,
    subtitlePath: item.subtitlePath,
    mediaType: item.mediaType
  };
}

export function createMusicAlbumSession(
  album: HomeMediaItemDto,
  tracks: MusicTrackDto[],
  currentTrackId?: string,
  artistTitle?: string | null
): PlayerSession | null {
  const playlist = tracks.map((track) => ({
    id: track.id,
    title: track.title,
    mediaPath: track.mediaPath,
    trackNumber: track.trackNumber,
    posterPath: track.posterPath ?? album.posterPath
  }));
  const selectedTrack = playlist.find((track) => track.id === currentTrackId) ?? playlist[0];

  if (!selectedTrack) {
    return null;
  }

  return {
    mediaId: selectedTrack.id,
    mediaTitle: selectedTrack.title,
    mediaPath: selectedTrack.mediaPath,
    posterPath: selectedTrack.posterPath ?? album.posterPath,
    subtitlePath: null,
    mediaType: "music_track",
    albumTitle: album.title,
    artistTitle: artistTitle ?? null,
    currentTrackId: selectedTrack.id,
    playlist
  };
}

export function readStoredPlayerSession(): PlayerSession | null {
  if (typeof window === "undefined") {
    return null;
  }

  const rawValue = window.localStorage.getItem(PLAYER_SESSION_STORAGE_KEY);

  if (!rawValue) {
    return null;
  }

  try {
    return JSON.parse(rawValue) as PlayerSession;
  } catch {
    window.localStorage.removeItem(PLAYER_SESSION_STORAGE_KEY);
    return null;
  }
}

export function persistPlayerSession(session: PlayerSession | null) {
  if (typeof window === "undefined") {
    return;
  }

  if (!session) {
    window.localStorage.removeItem(PLAYER_SESSION_STORAGE_KEY);
    return;
  }

  window.localStorage.setItem(PLAYER_SESSION_STORAGE_KEY, JSON.stringify(session));
}