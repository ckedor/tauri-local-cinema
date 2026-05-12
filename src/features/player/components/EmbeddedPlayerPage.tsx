import { VolumeControl } from "@/features/player/components/VolumeControl";
import {
    createContinueWatchingEntry,
    getContinueWatchingEntryId,
    shouldRemoveContinueWatching,
    supportsContinueWatching
} from "@/features/player/services/continueWatching";
import {
    clampPlaybackVolume,
    persistPlaybackVolumeState,
    PlaybackVolumeState,
    readStoredPlaybackVolumeState
} from "@/features/player/services/playbackVolume";
import { PlayerPlaylistTrack, PlayerSession } from "@/features/player/services/playerSession";
import { useContinueWatchingStore } from "@/features/player/store/continueWatching.store";
import { usePlayerStore } from "@/features/player/store/player.store";
import { getShowDetail } from "@/services/tauri/commands/library";
import {
    playerLoad,
    playerSeek,
    playerSeekTo,
    playerSetMuted,
    playerSetRect,
    playerSetVolume,
    PlayerStatus,
    playerStatus,
    playerStop,
    playerTogglePause
} from "@/services/tauri/commands/player";
import { PLAYER_EVENTS } from "@/services/tauri/events/player";
import { ShowEpisodeDto } from "@/types/contracts/library";
import {
    Close as CloseIcon,
    FullscreenExit as FullscreenExitIcon,
    Fullscreen as FullscreenIcon,
    Pause as PauseIcon,
    PlayArrow as PlayArrowIcon,
    Stop as StopIcon
} from "@mui/icons-material";
import { Box, ButtonBase, IconButton, Slider, Stack, Typography } from "@mui/material";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";

let pendingPlayerCleanupTimer: number | null = null;
const FULLSCREEN_CHROME_HIDE_DELAY_MS = 2000;
const CHROME_TRANSITION = "220ms ease";

export function EmbeddedPlayerPage() {
  const navigate = useNavigate();
  const session = usePlayerStore((s) => s.session);
  const setSession = usePlayerStore((s) => s.setSession);
  const initialPlayerVolumeState = useRef<PlaybackVolumeState>(readStoredPlaybackVolumeState());
  const surfaceRef = useRef<HTMLDivElement | null>(null);
  const loadFrameRef = useRef<number | null>(null);
  const chromeHideTimerRef = useRef<number | null>(null);
  const pendingSurfaceClickTimerRef = useRef<number | null>(null);
  const ignoreSurfaceClickUntilRef = useRef(0);
  const lastPersistedPositionRef = useRef(0);
  const latestStatusRef = useRef<PlayerStatus | null>(null);
  const pendingResumePositionRef = useRef<number | null>(null);
  const isApplyingResumeRef = useRef(false);
  const skipEmptySessionRedirectRef = useRef(false);
  const [status, setStatus] = useState<PlayerStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Local immersive player mode; this is intentionally separate from the app window fullscreen state.
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [showFullscreenChrome, setShowFullscreenChrome] = useState(true);
  const [isScrubbing, setIsScrubbing] = useState(false);
  const [scrubValue, setScrubValue] = useState(0);
  const [playerVolumeState, setPlayerVolumeState] = useState<PlaybackVolumeState>(() => initialPlayerVolumeState.current);
  const [showEpisodeQueue, setShowEpisodeQueue] = useState<ShowEpisodeDto[]>([]);
  const autoAdvancedEpisodeIdRef = useRef<string | null>(null);

  const measureRect = useCallback(() => {
    const el = surfaceRef.current;
    if (!el) return null;
    const r = el.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    return {
      x: Math.round(r.left * dpr),
      y: Math.round(r.top * dpr),
      width: Math.max(1, Math.round(r.width * dpr)),
      height: Math.max(1, Math.round(r.height * dpr))
    };
  }, []);

  const syncRect = useCallback(() => {
    const r = measureRect();
    if (r) void playerSetRect(r).catch(() => undefined);
  }, [measureRect]);

  const clearChromeHideTimer = useCallback(() => {
    if (chromeHideTimerRef.current !== null) {
      window.clearTimeout(chromeHideTimerRef.current);
      chromeHideTimerRef.current = null;
    }
  }, []);

  const scheduleChromeHide = useCallback(() => {
    if (!isFullscreen) return;

    clearChromeHideTimer();
    chromeHideTimerRef.current = window.setTimeout(() => {
      setShowFullscreenChrome(false);
      chromeHideTimerRef.current = null;
    }, FULLSCREEN_CHROME_HIDE_DELAY_MS);
  }, [clearChromeHideTimer, isFullscreen]);

  const revealFullscreenChrome = useCallback(() => {
    if (!isFullscreen) return;

    setShowFullscreenChrome(true);

    if (!isScrubbing) {
      scheduleChromeHide();
    }
  }, [isFullscreen, isScrubbing, scheduleChromeHide]);

  const persistPlayerVolumePreferences = useCallback((nextState: PlaybackVolumeState) => {
    initialPlayerVolumeState.current = nextState;
    setPlayerVolumeState(nextState);
    persistPlaybackVolumeState(nextState);
  }, []);

  const applyStoredPlayerVolume = useCallback(async (nextState: PlaybackVolumeState) => {
    let nextStatus = await playerSetVolume(nextState.level);

    if (nextState.isMuted !== nextStatus.isMuted) {
      nextStatus = await playerSetMuted(nextState.isMuted);
    }

    return nextStatus;
  }, []);

  const syncContinueWatching = useCallback((nextStatus: PlayerStatus | null, force = false) => {
    if (!session || !supportsContinueWatching(session)) {
      return;
    }

    const positionSec = Math.max(0, nextStatus?.positionSec ?? session.resumePositionSec ?? 0);
    const durationSec = Math.max(0, nextStatus?.durationSec ?? 0);

    if (!force && Math.abs(positionSec - lastPersistedPositionRef.current) < 5) {
      return;
    }

    if (shouldRemoveContinueWatching(positionSec, durationSec)) {
      useContinueWatchingStore.getState().removeEntry(getContinueWatchingEntryId(session));
      lastPersistedPositionRef.current = positionSec;
      return;
    }

    const nextEntry = createContinueWatchingEntry(session, positionSec, durationSec);

    if (!nextEntry) {
      return;
    }

    useContinueWatchingStore.getState().upsertEntry(nextEntry);
    lastPersistedPositionRef.current = positionSec;
  }, [session]);

  // Mount: load mpv with current session.
  useEffect(() => {
    if (pendingPlayerCleanupTimer !== null) {
      window.clearTimeout(pendingPlayerCleanupTimer);
      pendingPlayerCleanupTimer = null;
    }

    if (!session) {
      if (skipEmptySessionRedirectRef.current) {
        skipEmptySessionRedirectRef.current = false;
        return;
      }

      navigate("/", { replace: true });
      return;
    }
    let cancelled = false;
    const bootPlayer = () => {
      const rect = measureRect();

      if (!rect || rect.width < 48 || rect.height < 48) {
        loadFrameRef.current = window.requestAnimationFrame(bootPlayer);
        return;
      }

      void playerLoad(
        {
          mediaId: session.mediaId,
          mediaPath: session.mediaPath,
          mediaTitle: session.mediaTitle,
          subtitlePath: session.subtitlePath
        },
        rect
      )
        .then(async (initialStatus) => {
          if (cancelled) return;

          setError(null);
          syncRect();

          let nextStatus = initialStatus;

          try {
            nextStatus = await applyStoredPlayerVolume(initialPlayerVolumeState.current);
          } catch {
            // Keep playback running even if the volume sync fails.
          }

          if ((session.resumePositionSec ?? 0) > 0) {
            try {
              await playerSeekTo(session.resumePositionSec ?? 0);
              nextStatus = await playerStatus();
            } catch {
              // Keep playback running even if seeking to the saved position fails.
            }
          }

          if (!cancelled) {
            setStatus(nextStatus);
          }
        })
        .catch((e: unknown) => {
          if (!cancelled) setError(asMessage(e));
        });
    };

    bootPlayer();

    return () => {
      cancelled = true;
      if (loadFrameRef.current !== null) {
        window.cancelAnimationFrame(loadFrameRef.current);
        loadFrameRef.current = null;
      }

      syncContinueWatching(latestStatusRef.current, true);

      pendingPlayerCleanupTimer = window.setTimeout(() => {
        pendingPlayerCleanupTimer = null;
        void playerStop().catch(() => undefined);
      }, 150);
    };
  }, [applyStoredPlayerVolume, measureRect, navigate, session, syncContinueWatching, syncRect]);

  useEffect(() => {
    latestStatusRef.current = status;
    syncContinueWatching(status);
  }, [status, syncContinueWatching]);

  useEffect(() => {
    lastPersistedPositionRef.current = 0;
    pendingResumePositionRef.current = (session?.resumePositionSec ?? 0) > 0 ? (session?.resumePositionSec ?? 0) : null;
    isApplyingResumeRef.current = false;
    autoAdvancedEpisodeIdRef.current = null;
  }, [session?.mediaId, session?.parentMediaId]);

  useEffect(() => {
    if (session?.mediaType !== "show_episode" || !session.parentMediaId) {
      setShowEpisodeQueue([]);
      return;
    }

    let isMounted = true;

    void getShowDetail(session.parentMediaId)
      .then((detail) => {
        if (!isMounted) {
          return;
        }

        setShowEpisodeQueue(detail.seasons.flatMap((season) => season.episodes));
      })
      .catch(() => {
        if (isMounted) {
          setShowEpisodeQueue([]);
        }
      });

    return () => {
      isMounted = false;
    };
  }, [session?.mediaType, session?.parentMediaId]);

  useEffect(() => {
    const resumePosition = pendingResumePositionRef.current;

    if (!session || !status || !resumePosition || status.durationSec <= 0) {
      return;
    }

    if (status.positionSec >= resumePosition - 2) {
      pendingResumePositionRef.current = null;
      isApplyingResumeRef.current = false;
      return;
    }

    if (isApplyingResumeRef.current) {
      return;
    }

    isApplyingResumeRef.current = true;

    void playerSeekTo(resumePosition)
      .then(() => playerStatus())
      .then((nextStatus) => {
        setStatus(nextStatus);
      })
      .catch(() => undefined)
      .finally(() => {
        isApplyingResumeRef.current = false;
      });
  }, [session, status]);

  useEffect(() => {
    if (session?.mediaType !== "show_episode" || !session.parentMediaId || !status || !showEpisodeQueue.length) {
      return;
    }

    if (autoAdvancedEpisodeIdRef.current === session.mediaId) {
      return;
    }

    const remainingSeconds = status.durationSec - status.positionSec;

    if (status.durationSec <= 0 || remainingSeconds > 0.75 || status.isPaused) {
      return;
    }

    const currentEpisodeIndex = showEpisodeQueue.findIndex((episode) => episode.id === session.mediaId);
    const nextEpisode = currentEpisodeIndex >= 0 ? showEpisodeQueue[currentEpisodeIndex + 1] : null;

    if (!nextEpisode) {
      autoAdvancedEpisodeIdRef.current = session.mediaId;
      return;
    }

    autoAdvancedEpisodeIdRef.current = session.mediaId;
    syncContinueWatching(status, true);
    setSession(createNextShowEpisodeSession(session, nextEpisode));
  }, [session, setSession, showEpisodeQueue, status, syncContinueWatching]);

  // Keep mpv child window aligned with the placeholder div.
  useEffect(() => {
    const el = surfaceRef.current;
    if (!el) return;
    const ro = new ResizeObserver(syncRect);
    ro.observe(el);
    window.addEventListener("resize", syncRect);
    window.addEventListener("scroll", syncRect, true);
    // Listen to Tauri window resize events so app-window fullscreen toggles re-sync the mpv surface immediately.
    const tauriWin = getCurrentWindow();
    const unlistenPromise = tauriWin.onResized(() => syncRect());
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", syncRect);
      window.removeEventListener("scroll", syncRect, true);
      void unlistenPromise.then((u) => u()).catch(() => undefined);
    };
  }, [syncRect]);

  // Re-sync rect after fullscreen toggle (the surface div changes).
  useEffect(() => {
    const id = window.setTimeout(syncRect, 0);
    const id2 = window.setTimeout(syncRect, 120);
    return () => {
      window.clearTimeout(id);
      window.clearTimeout(id2);
    };
  }, [isFullscreen, showFullscreenChrome, syncRect]);

  useEffect(() => {
    if (!isFullscreen) {
      setShowFullscreenChrome(true);
      clearChromeHideTimer();
      return;
    }

    setShowFullscreenChrome(true);

    if (!isScrubbing) {
      scheduleChromeHide();
    }

    return clearChromeHideTimer;
  }, [clearChromeHideTimer, isFullscreen, isScrubbing, scheduleChromeHide]);

  useEffect(() => {
    if (!isFullscreen) {
      return;
    }

    const reveal = () => revealFullscreenChrome();
    window.addEventListener("mousemove", reveal);
    window.addEventListener("mousedown", reveal);
    window.addEventListener("keydown", reveal);

    return () => {
      window.removeEventListener("mousemove", reveal);
      window.removeEventListener("mousedown", reveal);
      window.removeEventListener("keydown", reveal);
    };
  }, [isFullscreen, revealFullscreenChrome]);

  // Poll position/duration while playing.
  useEffect(() => {
    const id = window.setInterval(async () => {
      try {
        const s = await playerStatus();
        setStatus(s);
      } catch {
        /* ignore */
      }
    }, 500);
    return () => window.clearInterval(id);
  }, []);

  function toggleFullscreen() {
    const next = !isFullscreen;
    setIsFullscreen(next);

    if (next) {
      setShowFullscreenChrome(true);
    } else {
      clearChromeHideTimer();
    }
  }

  async function handleTogglePause() {
    try {
      setStatus(await playerTogglePause());
    } catch (e) {
      setError(asMessage(e));
    }
  }

  async function handleSeek(delta: number) {
    try {
      await playerSeek(delta);
    } catch (e) {
      setError(asMessage(e));
    }
  }

  async function handleStop() {
    syncContinueWatching(latestStatusRef.current, true);

    try {
      await playerStop();
    } catch {
      /* ignore */
    }

    const returnPath = getPlayerReturnPath(session) ?? "/";
    skipEmptySessionRedirectRef.current = true;
    setSession(null);
    navigate(returnPath, { replace: true });
  }

  async function handleSetPlayerVolume(nextVolume: number) {
    const clampedVolume = clampPlaybackVolume(nextVolume);
    const nextPreferences: PlaybackVolumeState = {
      level: clampedVolume,
      isMuted: clampedVolume > 0 ? false : initialPlayerVolumeState.current.isMuted
    };

    persistPlayerVolumePreferences(nextPreferences);

    if (!status?.isPlaying) {
      return;
    }

    try {
      let nextStatus = await playerSetVolume(clampedVolume);

      if (clampedVolume > 0 && nextStatus.isMuted) {
        nextStatus = await playerSetMuted(false);
      }

      setStatus(nextStatus);
      setError(null);
    } catch (e) {
      setError(asMessage(e));
    }
  }

  async function handleTogglePlayerMuted() {
    const nextMuted = !(status?.isMuted ?? initialPlayerVolumeState.current.isMuted);
    const nextPreferences: PlaybackVolumeState = {
      ...initialPlayerVolumeState.current,
      isMuted: nextMuted
    };

    persistPlayerVolumePreferences(nextPreferences);

    if (!status?.isPlaying) {
      return;
    }

    try {
      setStatus(await playerSetMuted(nextMuted));
      setError(null);
    } catch (e) {
      setError(asMessage(e));
    }
  }

  const clearPendingSurfaceClick = useCallback(() => {
    if (pendingSurfaceClickTimerRef.current !== null) {
      window.clearTimeout(pendingSurfaceClickTimerRef.current);
      pendingSurfaceClickTimerRef.current = null;
    }
  }, []);

  useEffect(() => {
    let active = true;
    const unlistenCallbacks: Array<() => void> = [];

    const registerListeners = async () => {
      try {
        const [unlistenClick, unlistenDoubleClick] = await Promise.all([
          listen(PLAYER_EVENTS.surfaceClick, () => {
            if (Date.now() < ignoreSurfaceClickUntilRef.current) {
              return;
            }

            clearPendingSurfaceClick();
            pendingSurfaceClickTimerRef.current = window.setTimeout(() => {
              pendingSurfaceClickTimerRef.current = null;
              revealFullscreenChrome();
              void handleTogglePause();
            }, 220);
          }),
          listen(PLAYER_EVENTS.surfaceDoubleClick, () => {
            ignoreSurfaceClickUntilRef.current = Date.now() + 300;
            clearPendingSurfaceClick();
            revealFullscreenChrome();
            void toggleFullscreen();
          })
        ]);

        if (!active) {
          unlistenClick();
          unlistenDoubleClick();
          return;
        }

        unlistenCallbacks.push(unlistenClick, unlistenDoubleClick);
      } catch (e) {
        if (active) {
          setError(asMessage(e));
        }
      }
    };

    void registerListeners();

    return () => {
      active = false;
      clearPendingSurfaceClick();
      unlistenCallbacks.forEach((unlisten) => unlisten());
    };
  }, [clearPendingSurfaceClick, revealFullscreenChrome]);

  // Keyboard shortcuts.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.code === "Space") {
        e.preventDefault();
        void handleTogglePause();
      } else if (e.key === "f" || e.key === "F") {
        void toggleFullscreen();
      } else if (e.key === "Escape") {
        if (isFullscreen) void toggleFullscreen();
      } else if (e.key === "ArrowLeft") {
        void handleSeek(-5);
      } else if (e.key === "ArrowRight") {
        void handleSeek(5);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isFullscreen]);

  const duration = status?.durationSec ?? 0;
  const position = isScrubbing ? scrubValue : status?.positionSec ?? 0;
  const sliderMax = Math.max(1, duration);
  const fullscreenChromeVisible = !isFullscreen || showFullscreenChrome;
  const isAudioSession = session?.mediaType === "music_track";
  const playlist = session?.playlist ?? [];
  const selectedTrackId = session?.currentTrackId ?? session?.mediaId;
  const selectedTrack = playlist.find((track) => track.id === selectedTrackId) ?? null;
  const albumContextLabel = [session?.artistTitle, session?.albumTitle].filter(Boolean).join(" • ");
  const posterUrl = session?.posterPath ? convertFileSrc(session.posterPath) : null;
  const playerVolume = status?.volumePercent ?? playerVolumeState.level;
  const isPlayerMuted = status?.isMuted ?? playerVolumeState.isMuted;
  const hiddenChromeSx = {
    opacity: 0,
    maxHeight: 0,
    minHeight: 0,
    px: 0,
    py: 0,
    borderWidth: 0,
    overflow: "hidden",
    pointerEvents: "none",
    transition: `opacity ${CHROME_TRANSITION}, max-height ${CHROME_TRANSITION}, padding ${CHROME_TRANSITION}, border-width ${CHROME_TRANSITION}`
  } as const;
  const visibleChromeSx = {
    opacity: 1,
    maxHeight: 160,
    overflow: "hidden",
    pointerEvents: "auto",
    transition: `opacity ${CHROME_TRANSITION}, max-height ${CHROME_TRANSITION}, padding ${CHROME_TRANSITION}, border-width ${CHROME_TRANSITION}`
  } as const;

  function handleSelectPlaylistTrack(track: PlayerPlaylistTrack) {
    if (!session || track.id === selectedTrackId) {
      return;
    }

    setSession({
      ...session,
      mediaId: track.id,
      mediaTitle: track.title,
      mediaPath: track.mediaPath,
      posterPath: track.posterPath ?? session.posterPath,
      currentTrackId: track.id
    });
  }

  return (
    <Box
      onMouseMove={() => revealFullscreenChrome()}
      onMouseDown={() => revealFullscreenChrome()}
      sx={
        isFullscreen
          ? {
              position: "fixed",
              inset: 0,
              zIndex: 9999,
              display: "flex",
              flexDirection: "column",
              backgroundColor: "#000",
              cursor: fullscreenChromeVisible ? "default" : "none"
            }
          : {
              position: "relative",
              mx: -3,
              my: -3,
              height: "calc(100vh - 88px)",
              display: "flex",
              flexDirection: "column",
              backgroundColor: "#000"
            }
      }
    >
      <Stack
        direction="row"
        alignItems="center"
        justifyContent="space-between"
        sx={{
          position: "relative",
          zIndex: 1,
          px: 2,
          py: 1.5,
          borderBottom: "1px solid rgba(255,255,255,0.06)",
          background: "linear-gradient(180deg, rgba(13,16,20,0.98) 0%, rgba(13,16,20,0.92) 100%)",
          ...(fullscreenChromeVisible ? visibleChromeSx : hiddenChromeSx)
        }}
      >
        <Stack spacing={0.25} sx={{ minWidth: 0 }}>
          <Typography variant="h6" sx={{ color: "rgba(255,255,255,0.9)" }} noWrap>
            {session?.mediaTitle ?? "Reprodutor"}
          </Typography>
          {albumContextLabel ? (
            <Typography variant="caption" sx={{ color: "rgba(255,255,255,0.56)" }} noWrap>
              {albumContextLabel}
            </Typography>
          ) : null}
        </Stack>
        <Stack direction="row" spacing={0.5} alignItems="center">
          {!isFullscreen && status?.isPaused ? (
            <IconButton
              aria-label="Fechar player"
              onClick={() => void handleStop()}
              size="small"
              sx={{
                color: "rgba(255,255,255,0.78)",
                border: "1px solid rgba(255,255,255,0.1)",
                backgroundColor: "rgba(255,255,255,0.03)",
                ":hover": {
                  backgroundColor: "rgba(255,255,255,0.08)"
                }
              }}
            >
              <CloseIcon fontSize="small" />
            </IconButton>
          ) : null}
        </Stack>
      </Stack>

      <Box
        sx={{
          position: "relative",
          flex: 1,
          minHeight: 0,
          backgroundColor: "#000"
        }}
      >
        <Box
          ref={surfaceRef}
          sx={{
            position: "absolute",
            inset: 0,
            backgroundColor: "#000"
          }}
        />

        {isAudioSession ? (
          <Box
            sx={{
              position: "absolute",
              inset: 0,
              zIndex: 1,
              pointerEvents: "none",
              background:
                "radial-gradient(circle at top, rgba(209, 73, 91, 0.18), transparent 34%), linear-gradient(180deg, rgba(11,14,18,0.74) 0%, rgba(11,14,18,0.92) 100%)"
            }}
          >
            <Stack direction={{ xs: "column", md: "row" }} sx={{ height: "100%" }}>
              <Stack
                spacing={2}
                alignItems="center"
                justifyContent="center"
                sx={{
                  flex: 1,
                  px: { xs: 3, md: 6 },
                  py: { xs: 3, md: 5 },
                  minHeight: 0,
                  textAlign: "center"
                }}
              >
                <Box
                  sx={{
                    width: { xs: "min(78vw, 340px)", md: "min(52vw, 520px)" },
                    aspectRatio: "1 / 1",
                    borderRadius: 4,
                    overflow: "hidden",
                    boxShadow: "0 24px 64px rgba(0,0,0,0.38)",
                    background:
                      posterUrl
                        ? "rgba(17,20,24,0.96)"
                        : "linear-gradient(180deg, rgba(237, 174, 73, 0.18) 0%, rgba(209, 73, 91, 0.34) 100%)",
                    border: "1px solid rgba(255,255,255,0.08)"
                  }}
                >
                  {posterUrl ? (
                    <Box
                      component="img"
                      src={posterUrl}
                      alt={session?.albumTitle ?? session?.mediaTitle ?? "Album"}
                      sx={{ width: "100%", height: "100%", objectFit: "cover" }}
                    />
                  ) : null}
                </Box>

                <Stack spacing={0.5} sx={{ maxWidth: 560 }}>
                  <Typography variant="h4" sx={{ color: "rgba(255,255,255,0.94)" }}>
                    {session?.albumTitle ?? session?.mediaTitle ?? "Album"}
                  </Typography>
                  {selectedTrack ? (
                    <Typography variant="h6" sx={{ color: "rgba(255,255,255,0.78)" }}>
                      {selectedTrack.title}
                    </Typography>
                  ) : null}
                  {session?.artistTitle ? (
                    <Typography variant="body2" sx={{ color: "rgba(255,255,255,0.58)" }}>
                      {session.artistTitle}
                    </Typography>
                  ) : null}
                </Stack>
              </Stack>

              {playlist.length ? (
                <Box
                  sx={{
                    pointerEvents: "auto",
                    width: { xs: "100%", md: 340 },
                    maxHeight: { xs: 320, md: "100%" },
                    borderTop: { xs: "1px solid rgba(255,255,255,0.08)", md: "none" },
                    borderLeft: { xs: "none", md: "1px solid rgba(255,255,255,0.08)" },
                    backgroundColor: "rgba(9, 12, 16, 0.74)",
                    backdropFilter: "blur(16px)",
                    display: "flex",
                    flexDirection: "column"
                  }}
                >
                  <Stack spacing={0.4} sx={{ p: 2, borderBottom: "1px solid rgba(255,255,255,0.08)" }}>
                    <Typography variant="overline" sx={{ color: "rgba(255,255,255,0.56)", letterSpacing: 1.6 }}>
                      Playlist
                    </Typography>
                    <Typography variant="body2" sx={{ color: "rgba(255,255,255,0.78)" }}>
                      {playlist.length} faixas
                    </Typography>
                  </Stack>

                  <Stack spacing={0.75} sx={{ overflowY: "auto", p: 2 }}>
                    {playlist.map((track) => {
                      const isSelected = track.id === selectedTrackId;

                      return (
                        <ButtonBase
                          key={track.id}
                          onClick={() => handleSelectPlaylistTrack(track)}
                          sx={{
                            width: "100%",
                            borderRadius: 2.5,
                            border: "1px solid rgba(255,255,255,0.08)",
                            backgroundColor: isSelected ? "rgba(209, 73, 91, 0.18)" : "rgba(255,255,255,0.03)",
                            px: 1.25,
                            py: 1,
                            justifyContent: "flex-start",
                            textAlign: "left"
                          }}
                        >
                          <Stack direction="row" spacing={1.25} alignItems="center" sx={{ width: "100%" }}>
                            <Typography variant="caption" sx={{ minWidth: 28, color: "rgba(255,255,255,0.56)" }}>
                              {formatPlaylistTrackNumber(track)}
                            </Typography>
                            <Stack spacing={0.15} sx={{ minWidth: 0, flex: 1 }}>
                              <Typography noWrap sx={{ color: "rgba(255,255,255,0.9)", fontWeight: isSelected ? 700 : 500 }}>
                                {track.title}
                              </Typography>
                              <Typography noWrap variant="caption" sx={{ color: "rgba(255,255,255,0.5)" }}>
                                {track.mediaPath.split(/[\\/]/).pop() ?? track.mediaPath}
                              </Typography>
                            </Stack>
                          </Stack>
                        </ButtonBase>
                      );
                    })}
                  </Stack>
                </Box>
              ) : null}
            </Stack>
          </Box>
        ) : null}
      </Box>

      <Box
        sx={{
          position: "relative",
          zIndex: 1,
          px: 2,
          py: 1.5,
          borderTop: "1px solid rgba(255,255,255,0.06)",
          background: "linear-gradient(0deg, rgba(13,16,20,0.98) 0%, rgba(13,16,20,0.94) 100%)",
          ...(fullscreenChromeVisible ? visibleChromeSx : hiddenChromeSx)
        }}
      >
        <Stack direction="row" spacing={2} alignItems="center" sx={{ px: 1 }}>
          <Slider
            value={Math.min(position, sliderMax)}
            min={0}
            max={sliderMax}
            step={0.1}
            onChange={(_, v) => {
              revealFullscreenChrome();
              setIsScrubbing(true);
              setScrubValue(typeof v === "number" ? v : v[0]);
            }}
            onChangeCommitted={(_, v) => {
              const target = typeof v === "number" ? v : v[0];
              setIsScrubbing(false);
              void playerSeekTo(target).catch(() => undefined);
              if (isFullscreen) {
                scheduleChromeHide();
              }
            }}
            sx={{
              color: "primary.main",
              "& .MuiSlider-thumb": { width: 14, height: 14 },
              "& .MuiSlider-rail": { opacity: 0.3 }
            }}
          />
          <Typography
            noWrap
            variant="caption"
            sx={{ color: "rgba(255,255,255,0.7)", minWidth: 108, textAlign: "right" }}
          >
            {formatTime(position)} / {formatTime(duration)}
          </Typography>
        </Stack>

        <Stack direction="row" spacing={1} alignItems="center" sx={{ px: 1, pt: 0.5 }}>
          <IconButton
            onClick={() => void handleTogglePause()}
            sx={{ color: "rgba(255,255,255,0.92)" }}
          >
            {status?.isPaused ? <PlayArrowIcon /> : <PauseIcon />}
          </IconButton>
          <IconButton
            onClick={() => void handleSeek(-10)}
            sx={{ color: "rgba(255,255,255,0.85)" }}
          >
            <Typography variant="caption" sx={{ fontWeight: 700 }}>-10s</Typography>
          </IconButton>
          <IconButton
            onClick={() => void handleSeek(10)}
            sx={{ color: "rgba(255,255,255,0.85)" }}
          >
            <Typography variant="caption" sx={{ fontWeight: 700 }}>+10s</Typography>
          </IconButton>

          <VolumeControl
            volume={playerVolume}
            isMuted={isPlayerMuted}
            onChange={(nextVolume) => void handleSetPlayerVolume(nextVolume)}
            onToggleMuted={() => void handleTogglePlayerMuted()}
            sliderWidth={120}
            sx={{ ml: 0.5 }}
          />

          <Box sx={{ flex: 1 }} />

          <IconButton
            onClick={() => void toggleFullscreen()}
            sx={{ color: "rgba(255,255,255,0.85)" }}
          >
            {isFullscreen ? <FullscreenExitIcon /> : <FullscreenIcon />}
          </IconButton>
          <IconButton
            onClick={() => void handleStop()}
            sx={{ color: "error.main" }}
          >
            <StopIcon />
          </IconButton>
        </Stack>
      </Box>

      {error && fullscreenChromeVisible ? (
        <Box sx={{ px: 3, pb: 1.5 }}>
          <Typography color="error" variant="body2">
            {error}
          </Typography>
        </Box>
      ) : null}
    </Box>
  );
}

function formatTime(seconds: number): string {
  if (!isFinite(seconds) || seconds < 0) return "0:00";
  const total = Math.floor(seconds);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

function formatPlaylistTrackNumber(track: PlayerPlaylistTrack): string {
  return track.trackNumber ? String(track.trackNumber).padStart(2, "0") : "--";
}

function createNextShowEpisodeSession(session: PlayerSession, episode: ShowEpisodeDto): PlayerSession {
  return {
    ...session,
    mediaId: episode.id,
    mediaTitle: formatShowEpisodeTitle(session.parentTitle ?? session.mediaTitle, episode),
    mediaPath: episode.mediaPath,
    subtitlePath: episode.subtitlePath,
    mediaType: "show_episode",
    episodeTitle: episode.title,
    seasonNumber: episode.seasonNumber,
    episodeNumber: episode.episodeNumber,
    resumePositionSec: null
  };
}

function formatShowEpisodeTitle(showTitle: string, episode: ShowEpisodeDto): string {
  return [showTitle, formatShowEpisodeLabel(episode), episode.title].filter(Boolean).join(" - ");
}

function formatShowEpisodeLabel(episode: ShowEpisodeDto): string {
  if (episode.seasonNumber === 0) {
    return episode.episodeNumber ? `Extra ${String(episode.episodeNumber).padStart(2, "0")}` : "Extra";
  }

  const episodePart = episode.episodeNumber ? `E${String(episode.episodeNumber).padStart(2, "0")}` : "EP";
  return `T${String(episode.seasonNumber).padStart(2, "0")} ${episodePart}`;
}

function getPlayerReturnPath(session: PlayerSession | null): string | null {
  if (!session) {
    return null;
  }

  if (session.mediaType === "show_episode" && session.parentMediaId) {
    return `/library/${encodeURIComponent(session.parentMediaId)}`;
  }

  if (session.mediaType && session.mediaType !== "music_track") {
    return `/library/${encodeURIComponent(session.mediaId)}`;
  }

  return null;
}

function asMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return JSON.stringify(e);
}
