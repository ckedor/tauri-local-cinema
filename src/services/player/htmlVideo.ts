import { invoke } from "@tauri-apps/api/core";
import type { PlayerSessionPayload, PlayerStatus } from "@/services/tauri/commands/player";

// The player on Linux: an HTML <video> inside the player page, decoded by
// WebKitGTK through GStreamer, so the app's own controls sit on top of it.
// (On Windows, mpv draws into a native child window instead; Wayland has no
// way to embed another process's window.) It mirrors the player commands.
// Files come from the app's local HTTP server (media_server.rs): GStreamer
// can't read Tauri's asset:// scheme.

/** WebKitGTK's user agent says Linux; WebView2 on Windows does not. */
export const isLinuxWebview = /Linux/.test(navigator.userAgent) && !/Android/.test(navigator.userAgent);

let video: HTMLVideoElement | null = null;
let session: PlayerSessionPayload | null = null;
let subtitleUrls: string[] = [];

let serverUrl: Promise<string | null> | null = null;

/** The file's URL on the app's media server. */
async function mediaUrl(path: string): Promise<string> {
  serverUrl ??= invoke<string | null>("media_server_url");
  const base = await serverUrl;
  if (!base) throw new Error("no media server");
  return base + path.split("/").filter(Boolean).map(encodeURIComponent).join("/");
}

/** The page hands over its <video> element (null when it unmounts). */
export function attachVideoElement(element: HTMLVideoElement | null) {
  video = element;
}

function status(): PlayerStatus {
  if (!video || !session) {
    return {
      session: null,
      isPlaying: false,
      isPaused: false,
      positionSec: 0,
      durationSec: 0,
      volumePercent: 100,
      isMuted: false,
      lastError: null
    };
  }
  return {
    session,
    isPlaying: true,
    isPaused: video.paused,
    positionSec: video.currentTime || 0,
    durationSec: Number.isFinite(video.duration) ? video.duration : 0,
    volumePercent: Math.round(video.volume * 100),
    isMuted: video.muted,
    lastError: null
  };
}

/** <track> only reads WebVTT: SubRip differs in the header and the decimal comma. */
async function subtitleTrackUrl(path: string): Promise<string> {
  const text = await (await fetch(await mediaUrl(path))).text();
  if (/\.vtt$/i.test(path)) return URL.createObjectURL(new Blob([text], { type: "text/vtt" }));
  const vtt = "WEBVTT\n\n" + text.replace(/\r/g, "").replace(/(\d\d:\d\d:\d\d),(\d\d\d)/g, "$1.$2");
  return URL.createObjectURL(new Blob([vtt], { type: "text/vtt" }));
}

function clearSubtitles() {
  video?.querySelectorAll("track").forEach((track) => track.remove());
  subtitleUrls.forEach((url) => URL.revokeObjectURL(url));
  subtitleUrls = [];
}

/** Every subtitle file next to the video (library contract tags), plus the
 * session's own one when it is named differently. */
async function addSubtitleTracks(element: HTMLVideoElement, next: PlayerSessionPayload) {
  type SubtitleFile = { path: string; tag: string };
  const files = await invoke<SubtitleFile[]>("list_video_subtitles", { mediaPath: next.mediaPath }).catch((): SubtitleFile[] => []);
  if (next.subtitlePath?.trim() && !files.some((file) => file.path === next.subtitlePath)) {
    files.push({ path: next.subtitlePath, tag: "" });
  }

  for (const file of files) {
    try {
      const url = await subtitleTrackUrl(file.path);
      subtitleUrls.push(url);
      const language = file.tag.split(".")[0];
      const track = document.createElement("track");
      Object.assign(track, { kind: "subtitles", src: url, srclang: language, label: subtitleLabel(language, file.tag) });
      element.append(track);
    } catch {
      // Skip a file that can't be read; play with the others.
    }
  }
}

/** "pt-br" -> "Português (Brasil)"; the qualifier ("sdh") is kept. */
export function subtitleLabel(language: string | null | undefined, tag?: string) {
  const qualifier = tag?.split(".").slice(1).join(" ").toUpperCase();
  let name = language || "Legenda";
  try {
    if (language) name = new Intl.DisplayNames(["pt-BR"], { type: "language" }).of(language) ?? language;
  } catch {
    // Unknown code: show it as is.
  }
  name = name.charAt(0).toUpperCase() + name.slice(1);
  return qualifier ? `${name} (${qualifier})` : name;
}

/** Shows English when there is one (embedded or file), else the first track. */
function selectDefaultSubtitle(element: HTMLVideoElement) {
  const tracks = Array.from(element.textTracks).filter((track) => track.kind === "subtitles" || track.kind === "captions");
  const preferred = tracks.find((track) => /^en/i.test(track.language)) ?? tracks[0];
  for (const track of tracks) track.mode = track === preferred ? "showing" : "disabled";
}

export function subtitleTracks() {
  if (!video) return [];
  return Array.from(video.textTracks)
    .map((track, index) => ({ track, index }))
    .filter(({ track }) => track.kind === "subtitles" || track.kind === "captions")
    .map(({ track, index }) => ({
      id: String(index),
      label: track.label || subtitleLabel(track.language),
      language: track.language || null,
      selected: track.mode === "showing"
    }));
}

/** `null` turns subtitles off. */
export function setSubtitle(id: string | null) {
  if (!video) return;
  Array.from(video.textTracks).forEach((track, index) => {
    track.mode = String(index) === id ? "showing" : "disabled";
  });
}

/** Starts playback; rejects when WebKit can't play the file, so the caller
 * can fall back to mpv. */
export async function load(next: PlayerSessionPayload): Promise<PlayerStatus> {
  const element = video;
  if (!element) throw new Error("video element not mounted");
  stop();
  session = next;

  const ready = new Promise<void>((resolve, reject) => {
    const cleanup = () => {
      element.removeEventListener("loadedmetadata", onReady);
      element.removeEventListener("error", onError);
    };
    const onReady = () => {
      cleanup();
      resolve();
    };
    const onError = () => {
      cleanup();
      reject(new Error(element.error?.message || `can't play ${next.mediaPath}`));
    };
    element.addEventListener("loadedmetadata", onReady);
    element.addEventListener("error", onError);
  });

  element.src = await mediaUrl(next.mediaPath);
  await addSubtitleTracks(element, next);

  try {
    await ready;
    // Tracks embedded in the file are known once the metadata is in.
    selectDefaultSubtitle(element);
  } catch (error) {
    stop();
    throw error;
  }
  await element.play().catch(() => undefined);
  return status();
}

export function stop(): PlayerStatus {
  if (video) {
    video.pause();
    video.removeAttribute("src");
    clearSubtitles();
    video.load();
  }
  session = null;
  return status();
}

export async function setPaused(paused: boolean): Promise<PlayerStatus> {
  if (video) {
    if (paused) video.pause();
    else await video.play().catch(() => undefined);
  }
  return status();
}

export const togglePause = () => setPaused(!(video?.paused ?? true));

export function setVolume(volumePercent: number): PlayerStatus {
  if (video) video.volume = Math.min(100, Math.max(0, volumePercent)) / 100;
  return status();
}

export function setMuted(muted: boolean): PlayerStatus {
  if (video) video.muted = muted;
  return status();
}

export function seekTo(seconds: number) {
  if (video) video.currentTime = Math.max(0, seconds);
}

export function seek(deltaSeconds: number) {
  if (video) seekTo(video.currentTime + deltaSeconds);
}

export { status };
