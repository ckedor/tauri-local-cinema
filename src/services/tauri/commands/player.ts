import { invokeCommand } from "@/services/tauri/client/invoke";

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

export const playerLoad = (session: PlayerSessionPayload, rect: PlayerRect) =>
  invokeCommand<PlayerStatus>("player_load", { session, rect });

export const playerSetRect = (rect: PlayerRect) =>
  invokeCommand<void>("player_set_rect", { rect });

export const playerTogglePause = () =>
  invokeCommand<PlayerStatus>("player_toggle_pause");

export const playerSetPaused = (paused: boolean) =>
  invokeCommand<PlayerStatus>("player_set_paused", { paused });

export const playerSetVolume = (volumePercent: number) =>
  invokeCommand<PlayerStatus>("player_set_volume", { volumePercent });

export const playerSetMuted = (muted: boolean) =>
  invokeCommand<PlayerStatus>("player_set_muted", { muted });

export const playerSeek = (seconds: number) =>
  invokeCommand<void>("player_seek", { seconds });

export const playerSeekTo = (seconds: number) =>
  invokeCommand<void>("player_seek_to", { seconds });

export const playerStop = () =>
  invokeCommand<PlayerStatus>("player_stop");

export const playerStatus = () =>
  invokeCommand<PlayerStatus>("player_status");
