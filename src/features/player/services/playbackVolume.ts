export type PlaybackVolumeState = {
  level: number;
  isMuted: boolean;
};

const PLAYBACK_VOLUME_STORAGE_KEY = "netcrico.playback.volume";
const DEFAULT_PLAYBACK_VOLUME_STATE: PlaybackVolumeState = {
  level: 80,
  isMuted: false
};

export function clampPlaybackVolume(level: number) {
  if (!Number.isFinite(level)) {
    return DEFAULT_PLAYBACK_VOLUME_STATE.level;
  }

  return Math.max(0, Math.min(100, Math.round(level)));
}

export function readStoredPlaybackVolumeState(): PlaybackVolumeState {
  if (typeof window === "undefined") {
    return DEFAULT_PLAYBACK_VOLUME_STATE;
  }

  const rawValue = window.localStorage.getItem(PLAYBACK_VOLUME_STORAGE_KEY);

  if (!rawValue) {
    return DEFAULT_PLAYBACK_VOLUME_STATE;
  }

  try {
    const parsed = JSON.parse(rawValue) as Partial<PlaybackVolumeState>;

    return {
      level: clampPlaybackVolume(parsed.level ?? DEFAULT_PLAYBACK_VOLUME_STATE.level),
      isMuted: typeof parsed.isMuted === "boolean" ? parsed.isMuted : DEFAULT_PLAYBACK_VOLUME_STATE.isMuted
    };
  } catch {
    window.localStorage.removeItem(PLAYBACK_VOLUME_STORAGE_KEY);
    return DEFAULT_PLAYBACK_VOLUME_STATE;
  }
}

export function persistPlaybackVolumeState(state: PlaybackVolumeState) {
  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.setItem(
    PLAYBACK_VOLUME_STORAGE_KEY,
    JSON.stringify({
      level: clampPlaybackVolume(state.level),
      isMuted: state.isMuted
    })
  );
}