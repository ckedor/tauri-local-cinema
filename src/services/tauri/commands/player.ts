import { invokeCommand } from "@/services/tauri/client/invoke";
import * as htmlVideo from "@/services/player/htmlVideo";
import { getSubtitleSettings } from "@/services/tauri/commands/library";

export type PlayerSessionPayload = {
  mediaId: string;
  mediaPath: string;
  mediaTitle: string;
  subtitlePath: string | null;
};

export type PlayerRect = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type PlayerStatus = {
  session: PlayerSessionPayload | null;
  isPlaying: boolean;
  isPaused: boolean;
  positionSec: number;
  durationSec: number;
  volumePercent: number;
  isMuted: boolean;
  lastError: string | null;
};

// Which engine plays the current session. Linux plays in the page's <video>
// (see htmlVideo.ts); a file WebKit can't open falls back to mpv, which there
// opens its own window. Windows always uses the embedded mpv.
let engine: "html" | "mpv" = htmlVideo.isLinuxWebview ? "html" : "mpv";
const html = () => engine === "html";

export const playerLoad = async (session: PlayerSessionPayload, rect: PlayerRect) => {
  if (htmlVideo.isLinuxWebview) {
    try {
      engine = "html";
      return await htmlVideo.load(session);
    } catch (error) {
      engine = "mpv";
      void invokeCommand("log_frontend", {
        message: `<video> can't play ${session.mediaPath} (${String(error)}): falling back to mpv`
      }).catch(() => undefined);
    }
  }
  return invokeCommand<PlayerStatus>("player_load", { session, rect });
};

export const playerSetRect = (rect: PlayerRect) =>
  html() ? Promise.resolve() : invokeCommand<void>("player_set_rect", { rect });

export const playerTogglePause = () =>
  html() ? htmlVideo.togglePause() : invokeCommand<PlayerStatus>("player_toggle_pause");

export const playerSetPaused = (paused: boolean) =>
  html() ? htmlVideo.setPaused(paused) : invokeCommand<PlayerStatus>("player_set_paused", { paused });

export const playerSetVolume = (volumePercent: number) =>
  html()
    ? Promise.resolve(htmlVideo.setVolume(volumePercent))
    : invokeCommand<PlayerStatus>("player_set_volume", { volumePercent });

export const playerSetMuted = (muted: boolean) =>
  html() ? Promise.resolve(htmlVideo.setMuted(muted)) : invokeCommand<PlayerStatus>("player_set_muted", { muted });

export const playerSeek = (seconds: number) =>
  html() ? Promise.resolve(htmlVideo.seek(seconds)) : invokeCommand<void>("player_seek", { seconds });

export const playerSeekTo = (seconds: number) =>
  html() ? Promise.resolve(htmlVideo.seekTo(seconds)) : invokeCommand<void>("player_seek_to", { seconds });

export const playerStop = async () => {
  if (html()) return htmlVideo.stop();
  const status = await invokeCommand<PlayerStatus>("player_stop");
  engine = htmlVideo.isLinuxWebview ? "html" : "mpv";
  return status;
};

export const playerStatus = () =>
  html() ? Promise.resolve(htmlVideo.status()) : invokeCommand<PlayerStatus>("player_status");

export type SubtitleTrack = {
  id: string;
  label: string;
  language: string | null;
  selected: boolean;
  /** A file next to the video, as opposed to a track inside it. */
  external: boolean;
};

/** Subtitle tracks of the playing file in the languages set in Settings
 * (embedded ones and files next to it); a release often embeds dozens. */
export const playerSubtitleTracks = async (): Promise<SubtitleTrack[]> => {
  const preferred = await subtitleLanguages();
  let tracks: SubtitleTrack[];
  if (html()) {
    tracks = htmlVideo.subtitleTracks().map((track) => ({ ...track, external: true }));
  } else {
    const raw = await invokeCommand<{ id: string; language: string | null; title: string | null; external: boolean; selected: boolean }[]>("player_subtitle_tracks");
    tracks = raw.map((track) => ({
      id: track.id,
      label: track.title && track.title !== track.language ? htmlVideo.subtitleLabel(track.language, track.title) : htmlVideo.subtitleLabel(track.language),
      language: track.language,
      selected: track.selected,
      external: track.external
    }));
  }

  // The file next to the video is the one the library keeps (often extracted
  // from the track inside it), so it stands in for the embedded ones.
  const byLanguage = preferred.flatMap((language) => {
    const matching = tracks.filter((track) => matchesLanguage(language, track.language, track.label));
    return matching.some((track) => track.external) ? matching.filter((track) => track.external) : matching;
  });
  // A file with no language tag can't be matched, but it is still the user's.
  return [...byLanguage, ...tracks.filter((track) => track.external && !track.language)];
};

let preferredLanguages: Promise<string[]> | null = null;

function subtitleLanguages() {
  preferredLanguages ??= getSubtitleSettings()
    .then((settings) => settings.languages.split(",").map((language) => language.trim()).filter(Boolean))
    .catch(() => ["pt-BR", "en"]);
  return preferredLanguages;
}

// Releases tag tracks with ISO 639-2 codes ("eng", "por") and name the
// variant in the title ("Brazil", "Latin America").
const THREE_LETTER_CODES: Record<string, string> = { eng: "en", por: "pt", pob: "pt-br", spa: "es", fre: "fr", fra: "fr", ger: "de", deu: "de", ita: "it", jpn: "ja" };

/** "pt-BR" matches pt-br, pob, or por/pt titled "Brazil"/"Brasil"; "en" matches en, eng, en-US. */
export function matchesLanguage(preferred: string, language: string | null, title: string | null) {
  const [base, region] = preferred.toLowerCase().split("-");
  const code = (language ?? "").toLowerCase().split(/[._]/)[0];
  const [trackBase, trackRegion] = (THREE_LETTER_CODES[code] ?? code).split("-");
  if (trackBase !== base) return false;
  if (!region || trackRegion === region) return true;
  if (trackRegion) return false;
  const regionNames = [region, regionName("en", region), regionName("pt-BR", region)].filter(Boolean) as string[];
  return regionNames.some((name) => new RegExp(`\\b${name}\\b`, "i").test(title ?? ""));
}

function regionName(locale: string, region: string) {
  try {
    return new Intl.DisplayNames([locale], { type: "region" }).of(region.toUpperCase()) ?? null;
  } catch {
    return null;
  }
}

/** `null` turns subtitles off. */
export const playerSetSubtitle = (id: string | null) =>
  html() ? Promise.resolve(htmlVideo.setSubtitle(id)) : invokeCommand<void>("player_set_subtitle", { id });
