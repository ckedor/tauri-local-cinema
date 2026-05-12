import { MusicArtistDetailView } from "@/features/library/components/MusicArtistDetailView";
import {
  formatMediaType,
  getMediaFormat,
  getPlaceholderDescription
} from "@/features/library/utils/media";
import { VolumeControl } from "@/features/player/components/VolumeControl";
import {
  ContinueWatchingEntry,
  toPlayerSessionFromContinueWatching
} from "@/features/player/services/continueWatching";
import { launchPlayer, launchPlayerSession } from "@/features/player/services/launchPlayer";
import {
  clampPlaybackVolume,
  persistPlaybackVolumeState,
  PlaybackVolumeState,
  readStoredPlaybackVolumeState
} from "@/features/player/services/playbackVolume";
import { useContinueWatchingStore } from "@/features/player/store/continueWatching.store";
import {
  getMediaItem,
  getMusicAlbumDetail,
  getMusicArtistDetail,
  getShowDetail
} from "@/services/tauri/commands/library";
import {
  HomeMediaItemDto,
  MusicAlbumDetailDto,
  MusicArtistDetailDto,
  MusicTrackDto,
  ShowDetailDto,
  ShowEpisodeDto
} from "@/types/contracts/library";
import PauseRoundedIcon from "@mui/icons-material/PauseRounded";
import PlayArrowRoundedIcon from "@mui/icons-material/PlayArrowRounded";
import {
  Alert,
  Box,
  Button,
  Chip,
  CircularProgress,
  Divider,
  IconButton,
  LinearProgress,
  Slider,
  Stack,
  Tab,
  Tabs,
  Typography
} from "@mui/material";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useMemo, useRef, useState, type SyntheticEvent } from "react";
import { useLocation, useNavigate, useParams } from "react-router-dom";

type LibraryDetailLocationState = {
  item?: HomeMediaItemDto;
  backToPath?: string;
};

export function LibraryDetailPage() {
  const navigate = useNavigate();
  const { mediaId } = useParams();
  const location = useLocation();
  const navigationState = location.state as LibraryDetailLocationState | null;
  const decodedMediaId = mediaId ? decodeURIComponent(mediaId) : null;
  const [item, setItem] = useState<HomeMediaItemDto | null>(navigationState?.item ?? null);
  const [isLoading, setIsLoading] = useState(!navigationState?.item);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [showDetail, setShowDetail] = useState<ShowDetailDto | null>(null);
  const [showDetailError, setShowDetailError] = useState<string | null>(null);
  const [isLoadingShowDetail, setIsLoadingShowDetail] = useState(false);
  const [selectedSeasonNumber, setSelectedSeasonNumber] = useState<number | null>(null);
  const [musicArtistDetail, setMusicArtistDetail] = useState<MusicArtistDetailDto | null>(null);
  const [musicArtistDetailError, setMusicArtistDetailError] = useState<string | null>(null);
  const [isLoadingMusicArtistDetail, setIsLoadingMusicArtistDetail] = useState(false);
  const [musicAlbumDetail, setMusicAlbumDetail] = useState<MusicAlbumDetailDto | null>(null);
  const [musicAlbumDetailError, setMusicAlbumDetailError] = useState<string | null>(null);
  const [isLoadingMusicAlbumDetail, setIsLoadingMusicAlbumDetail] = useState(false);
  const [hasPosterError, setHasPosterError] = useState(false);
  const [posterAspectRatio, setPosterAspectRatio] = useState<number | null>(null);
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const [activeAlbumTrackId, setActiveAlbumTrackId] = useState<string | null>(null);
  const [isInlineAlbumPlaying, setIsInlineAlbumPlaying] = useState(false);
  const [inlineAlbumPlaybackTime, setInlineAlbumPlaybackTime] = useState(0);
  const [inlineAlbumDuration, setInlineAlbumDuration] = useState(0);
  const [inlineAlbumPlaybackError, setInlineAlbumPlaybackError] = useState<string | null>(null);
  const [inlineAlbumVolumeState, setInlineAlbumVolumeState] = useState<PlaybackVolumeState>(() => readStoredPlaybackVolumeState());
  const continueWatchingEntries = useContinueWatchingStore((state) => state.entries);

  useEffect(() => {
    if (!decodedMediaId || !navigationState?.item) {
      return;
    }

    if (navigationState.item.id !== decodedMediaId) {
      return;
    }

    setItem(navigationState.item);
    setIsLoading(false);
    setErrorMessage(null);
  }, [decodedMediaId, navigationState?.item]);

  useEffect(() => {
    if (navigationState?.item || !decodedMediaId) {
      return;
    }

    let isMounted = true;

    void (async () => {
      setIsLoading(true);

      try {
        const matchedItem = await getMediaItem(decodedMediaId);

        if (!isMounted) {
          return;
        }

        setItem(matchedItem);
        setErrorMessage(matchedItem ? null : "Titulo nao encontrado na biblioteca.");
      } catch (error) {
        if (isMounted) {
          setErrorMessage(asMessage(error, "Nao foi possivel carregar os detalhes do titulo."));
        }
      } finally {
        if (isMounted) {
          setIsLoading(false);
        }
      }
    })();

    return () => {
      isMounted = false;
    };
  }, [decodedMediaId, navigationState?.item]);

  useEffect(() => {
    setHasPosterError(false);
    setPosterAspectRatio(null);
  }, [item?.coverPath, item?.posterPath]);

  useEffect(() => {
    if (item?.mediaType !== "show") {
      setShowDetail(null);
      setShowDetailError(null);
      setSelectedSeasonNumber(null);
      setIsLoadingShowDetail(false);
      return;
    }

    let isMounted = true;
    setIsLoadingShowDetail(true);

    void getShowDetail(item.id)
      .then((detail) => {
        if (!isMounted) {
          return;
        }

        const orderedSeasons = [...detail.seasons].sort(compareShowSeasons);
        const nextDetail = { ...detail, seasons: orderedSeasons };

        setShowDetail(nextDetail);
        setShowDetailError(null);
        setSelectedSeasonNumber((currentSeason) => {
          if (currentSeason !== null && nextDetail.seasons.some((season) => season.seasonNumber === currentSeason)) {
            return currentSeason;
          }

          if (
            currentShowContinueEntry?.seasonNumber !== null
            && currentShowContinueEntry?.seasonNumber !== undefined
            && nextDetail.seasons.some((season) => season.seasonNumber === currentShowContinueEntry.seasonNumber)
          ) {
            return currentShowContinueEntry.seasonNumber;
          }

          return nextDetail.seasons.find((season) => season.seasonNumber > 0)?.seasonNumber
            ?? nextDetail.seasons[0]?.seasonNumber
            ?? null;
        });
      })
      .catch((error) => {
        if (isMounted) {
          setShowDetail(null);
          setShowDetailError(asMessage(error, "Nao foi possivel carregar temporadas e episodios."));
          setSelectedSeasonNumber(null);
        }
      })
      .finally(() => {
        if (isMounted) {
          setIsLoadingShowDetail(false);
        }
      });

    return () => {
      isMounted = false;
    };
  }, [continueWatchingEntries, item?.id, item?.mediaType]);

  useEffect(() => {
    if (item?.mediaType !== "music_artist") {
      setMusicArtistDetail(null);
      setMusicArtistDetailError(null);
      setIsLoadingMusicArtistDetail(false);
      return;
    }

    let isMounted = true;
    setIsLoadingMusicArtistDetail(true);

    void getMusicArtistDetail(item.id)
      .then((detail) => {
        if (!isMounted) {
          return;
        }

        setMusicArtistDetail({
          ...detail,
          albums: [...detail.albums].sort(compareAlbumsByYear)
        });
        setMusicArtistDetailError(null);
      })
      .catch((error) => {
        if (isMounted) {
          setMusicArtistDetail(null);
          setMusicArtistDetailError(asMessage(error, "Nao foi possivel carregar os albuns do artista."));
        }
      })
      .finally(() => {
        if (isMounted) {
          setIsLoadingMusicArtistDetail(false);
        }
      });

    return () => {
      isMounted = false;
    };
  }, [item?.id, item?.mediaType]);

  useEffect(() => {
    if (item?.mediaType !== "music_album") {
      setMusicAlbumDetail(null);
      setMusicAlbumDetailError(null);
      setIsLoadingMusicAlbumDetail(false);
      return;
    }

    let isMounted = true;
    setIsLoadingMusicAlbumDetail(true);

    void getMusicAlbumDetail(item.id)
      .then((detail) => {
        if (!isMounted) {
          return;
        }

        setMusicAlbumDetail(detail);
        setMusicAlbumDetailError(null);
      })
      .catch((error) => {
        if (isMounted) {
          setMusicAlbumDetail(null);
          setMusicAlbumDetailError(asMessage(error, "Nao foi possivel carregar a playlist do album."));
        }
      })
      .finally(() => {
        if (isMounted) {
          setIsLoadingMusicAlbumDetail(false);
        }
      });

    return () => {
      isMounted = false;
    };
  }, [item?.id, item?.mediaType]);

  useEffect(() => {
    const audio = audioRef.current;

    setActiveAlbumTrackId(null);
    setIsInlineAlbumPlaying(false);
    setInlineAlbumPlaybackTime(0);
    setInlineAlbumDuration(0);
    setInlineAlbumPlaybackError(null);

    if (!audio) {
      return;
    }

    audio.pause();
    audio.removeAttribute("src");
    audio.load();
  }, [item?.id, item?.mediaType]);

  useEffect(() => {
    return () => {
      const audio = audioRef.current;

      if (!audio) {
        return;
      }

      audio.pause();
      audio.removeAttribute("src");
      audio.load();
    };
  }, []);

  useEffect(() => {
    const audio = audioRef.current;

    if (audio) {
      audio.volume = inlineAlbumVolumeState.level / 100;
      audio.muted = inlineAlbumVolumeState.isMuted;
    }

    persistPlaybackVolumeState(inlineAlbumVolumeState);
  }, [inlineAlbumVolumeState]);

  const posterUrl = useMemo(() => {
    const detailImagePath = item?.coverPath ?? item?.posterPath;

    if (!detailImagePath || hasPosterError) {
      return null;
    }

    return convertFileSrc(detailImagePath);
  }, [hasPosterError, item?.coverPath, item?.posterPath]);

  const isNarrowPoster = posterAspectRatio !== null && posterAspectRatio < 1.15;
  const posterFadeMask = isNarrowPoster
    ? "linear-gradient(to right, transparent 0%, rgba(0, 0, 0, 0.28) 10%, rgba(0, 0, 0, 0.58) 16%, rgba(0, 0, 0, 0.84) 22%, rgba(0, 0, 0, 0.96) 27%, rgba(0, 0, 0, 1) 31%, rgba(0, 0, 0, 1) 100%)"
    : "linear-gradient(to right, transparent 0%, rgba(0, 0, 0, 0.24) 12%, rgba(0, 0, 0, 0.56) 20%, rgba(0, 0, 0, 0.82) 28%, rgba(0, 0, 0, 0.95) 34%, rgba(0, 0, 0, 1) 40%, rgba(0, 0, 0, 1) 100%)";

  const handlePosterLoad = (event: SyntheticEvent<HTMLImageElement>) => {
    const { naturalWidth, naturalHeight } = event.currentTarget;

    if (naturalWidth > 0 && naturalHeight > 0) {
      setPosterAspectRatio(naturalWidth / naturalHeight);
    }
  };

  const backToPath = useMemo(() => {
    if (navigationState?.backToPath) {
      return navigationState.backToPath;
    }

    return getDefaultBackToPath(item);
  }, [item, navigationState?.backToPath]);

  const selectedSeason = useMemo(() => {
    if (!showDetail) {
      return null;
    }

    return showDetail.seasons.find((season) => season.seasonNumber === selectedSeasonNumber)
      ?? showDetail.seasons[0]
      ?? null;
  }, [selectedSeasonNumber, showDetail]);

  const totalEpisodeCount = useMemo(() => {
    if (!showDetail) {
      return 0;
    }

    return showDetail.seasons.reduce((total, season) => total + season.episodes.length, 0);
  }, [showDetail]);

  const seasonSummaryLabel = useMemo(() => getSeasonSummaryLabel(showDetail), [showDetail]);
  const artistAlbums = musicArtistDetail?.albums ?? [];
  const albumTracks = musicAlbumDetail?.tracks ?? [];
  const currentShowContinueEntry = useMemo(() => {
    if (item?.mediaType !== "show") {
      return null;
    }

    return continueWatchingEntries.find(
      (entry) => entry.mediaType === "show_episode" && entry.entryId === item.id
    ) ?? null;
  }, [continueWatchingEntries, item?.id, item?.mediaType]);
  const detailDescription = useMemo(() => (item ? getPlaceholderDescription(item) : ""), [item]);
  const activeAlbumTrack = useMemo(
    () => albumTracks.find((track) => track.id === activeAlbumTrackId) ?? null,
    [activeAlbumTrackId, albumTracks]
  );
  const inlineAlbumProgressPercent = useMemo(() => {
    if (inlineAlbumDuration <= 0) {
      return 0;
    }

    return Math.min(100, (inlineAlbumPlaybackTime / inlineAlbumDuration) * 100);
  }, [inlineAlbumDuration, inlineAlbumPlaybackTime]);

  async function handlePlayEpisode(episode: ShowEpisodeDto) {
    if (!item) {
      return;
    }

    if (currentShowContinueEntry?.mediaId === episode.id) {
      await launchPlayerSession(toPlayerSessionFromContinueWatching(currentShowContinueEntry));
      return;
    }

    await launchPlayerSession({
      mediaId: episode.id,
      mediaTitle: buildEpisodePlaybackTitle(item.title, episode),
      mediaPath: episode.mediaPath,
      posterPath: item.posterPath ?? item.coverPath,
      subtitlePath: episode.subtitlePath,
      mediaType: "show_episode",
      parentMediaId: item.id,
      parentTitle: item.title,
      episodeTitle: episode.title,
      seasonNumber: episode.seasonNumber,
      episodeNumber: episode.episodeNumber
    });
  }

  async function playAlbumTrack(track: MusicTrackDto) {
    const audio = audioRef.current;

    if (!audio) {
      return;
    }

    const isSameTrack = activeAlbumTrackId === track.id;

    if (!isSameTrack) {
      audio.src = convertFileSrc(track.mediaPath);
      audio.load();
      setActiveAlbumTrackId(track.id);
      setInlineAlbumPlaybackTime(0);
      setInlineAlbumDuration(0);
    }

    try {
      await audio.play();
      setInlineAlbumPlaybackError(null);
    } catch (error) {
      setIsInlineAlbumPlaying(false);
      setInlineAlbumPlaybackError(asMessage(error, "Nao foi possivel reproduzir a faixa."));
    }
  }

  async function handleToggleAlbumTrack(track: MusicTrackDto) {
    const audio = audioRef.current;

    if (!audio) {
      return;
    }

    if (activeAlbumTrackId === track.id) {
      if (audio.paused) {
        await playAlbumTrack(track);
      } else {
        audio.pause();
      }

      return;
    }

    await playAlbumTrack(track);
  }

  async function handleToggleAlbumPlayback() {
    const track = activeAlbumTrack ?? albumTracks[0];

    if (!track) {
      return;
    }

    await handleToggleAlbumTrack(track);
  }

  async function handleAdvanceToNextAlbumTrack() {
    if (!activeAlbumTrackId) {
      setIsInlineAlbumPlaying(false);
      return;
    }

    const currentTrackIndex = albumTracks.findIndex((track) => track.id === activeAlbumTrackId);
    const nextTrack = currentTrackIndex >= 0 ? albumTracks[currentTrackIndex + 1] : null;

    if (!nextTrack) {
      setIsInlineAlbumPlaying(false);
      setInlineAlbumPlaybackTime(inlineAlbumDuration);
      return;
    }

    await playAlbumTrack(nextTrack);
  }

  function handleSeekAlbumTrack(nextTime: number) {
    const audio = audioRef.current;

    if (!audio || !Number.isFinite(nextTime)) {
      return;
    }

    const boundedTime = Math.max(0, Math.min(nextTime, inlineAlbumDuration || nextTime));
    audio.currentTime = boundedTime;
    setInlineAlbumPlaybackTime(boundedTime);
    setInlineAlbumPlaybackError(null);
  }

  function handleInlineAlbumVolumeChange(nextVolume: number) {
    const clampedVolume = clampPlaybackVolume(nextVolume);

    setInlineAlbumVolumeState((currentState) => ({
      level: clampedVolume,
      isMuted: clampedVolume > 0 ? false : currentState.isMuted
    }));
    setInlineAlbumPlaybackError(null);
  }

  function handleToggleInlineAlbumMuted() {
    setInlineAlbumVolumeState((currentState) => ({
      ...currentState,
      isMuted: !currentState.isMuted
    }));
  }

  if (isLoading) {
    return (
      <Stack alignItems="center" justifyContent="center" sx={{ minHeight: "60vh" }}>
        <CircularProgress color="secondary" />
      </Stack>
    );
  }

  if (!item) {
    return (
      <Stack spacing={3}>
        <Button sx={{ width: "fit-content" }} variant="text" onClick={() => navigate(-1)}>
          Voltar
        </Button>
        <Alert severity="error">{errorMessage ?? "Titulo nao encontrado."}</Alert>
      </Stack>
    );
  }

  const isShow = item.mediaType === "show";
  const isMusicArtist = item.mediaType === "music_artist";
  const isMusicAlbum = item.mediaType === "music_album";

  if (isMusicArtist) {
    return (
      <MusicArtistDetailView
        item={item}
        posterUrl={posterUrl}
        albums={artistAlbums}
        isLoading={isLoadingMusicArtistDetail}
        errorMessage={musicArtistDetailError ?? errorMessage}
        backToPath={backToPath}
        onBack={() => navigate(backToPath)}
        onOpenAlbum={(album) =>
          navigate(`/library/${encodeURIComponent(album.id)}`, {
            state: { item: album, backToPath: location.pathname }
          })
        }
      />
    );
  }

  return (
    <Box
      sx={{
        position: "relative",
        minHeight: "calc(100vh - 88px)",
        mx: { xs: -3, md: -3 },
        my: -3,
        overflow: "hidden",
        background: "linear-gradient(180deg, rgba(var(--app-bg-rgb), 0.995) 0%, rgba(var(--app-bg-deep-rgb), 0.995) 100%)"
      }}
    >
      {!isMusicAlbum ? (
        <Box
          sx={{
            display: { xs: "none", md: "block" },
            position: "absolute",
            inset: 0,
            zIndex: 0,
            pointerEvents: "none",
            overflow: "hidden",
            background:
              "linear-gradient(135deg, rgba(237, 174, 73, 0.08), rgba(var(--app-accent-rgb), 0.14) 34%, rgba(var(--app-bg-rgb), 0.9) 72%, rgba(var(--app-bg-deep-rgb), 0.98) 100%)"
          }}
        >
          {posterUrl ? (
            <Box
              sx={{
                position: "absolute",
                inset: 0,
                overflow: "hidden"
              }}
            >
              <Box
                sx={{
                  position: "absolute",
                  inset: 0,
                  display: "flex",
                  alignItems: "stretch",
                  justifyContent: "flex-end",
                  overflow: "hidden"
                }}
              >
                <Box
                  component="img"
                  src={posterUrl}
                  alt={item.title}
                  onLoad={handlePosterLoad}
                  onError={() => setHasPosterError(true)}
                  sx={{
                    width: "auto",
                    maxWidth: "none",
                    height: "100%",
                    maxHeight: "100%",
                    flexShrink: 0,
                    display: "block",
                    objectFit: "contain",
                    objectPosition: "right top",
                    filter: "saturate(0.98) contrast(1.02)",
                    transform: "translateX(1.2%)",
                    WebkitMaskImage: posterFadeMask,
                    maskImage: posterFadeMask,
                    WebkitMaskRepeat: "no-repeat",
                    maskRepeat: "no-repeat",
                    WebkitMaskSize: "100% 100%",
                    maskSize: "100% 100%"
                  }}
                />
              </Box>

              <Box
                sx={{
                  width: "50%",
                  position: "absolute",
                  inset: "0 0 0 auto",
                  background: "linear-gradient(to left, rgba(var(--app-bg-rgb), 0.18) 0%, rgba(var(--app-bg-rgb), 0.08) 12%, rgba(var(--app-bg-rgb), 0) 28%)"
                }}
              />
            </Box>
          ) : null}

          <Box
            sx={{
              position: "absolute",
              inset: 0,
              background:
                "linear-gradient(to right, rgba(var(--app-bg-rgb), 1) 0%, rgba(var(--app-bg-rgb), 0.994) 16%, rgba(var(--app-bg-rgb), 0.972) 26%, rgba(var(--app-bg-rgb), 0.9) 36%, rgba(var(--app-bg-rgb), 0.72) 46%, rgba(var(--app-bg-rgb), 0.46) 56%, rgba(var(--app-bg-rgb), 0.2) 68%, rgba(var(--app-bg-rgb), 0.06) 80%, rgba(var(--app-bg-rgb), 0) 100%), linear-gradient(to bottom, rgba(var(--app-bg-rgb), 0.04) 0%, rgba(var(--app-bg-rgb), 0.02) 22%, rgba(var(--app-bg-rgb), 0.08) 58%, rgba(var(--app-bg-deep-rgb), 0.38) 100%)"
            }}
          />
        </Box>
      ) : null}

      <Stack
        direction={{ xs: "column-reverse", md: "row" }}
        sx={{
          position: "relative",
          zIndex: 1,
          minHeight: "calc(100vh - 88px)"
        }}
      >
        <Stack
          spacing={isShow ? 2.25 : 3}
          sx={{
            position: "relative",
            zIndex: 2,
            width: { xs: "100%", md: "46%" },
            px: { xs: 2, md: 4 },
            pb: { xs: 2, md: 4 },
            pt: { xs: 1.1, md: 2.5 },
            justifyContent: "flex-start",
            background: {
              xs: "rgba(var(--app-bg-rgb), 0.96)",
              md: "transparent"
            }
          }}
        >
          <Stack
            direction="row"
            spacing={1.25}
            alignItems="center"
            justifyContent="space-between"
            useFlexGap
            sx={{ flexWrap: "wrap", rowGap: 0.75 }}
          >
            <Button
              sx={{
                width: "fit-content",
                minWidth: 0,
                color: "text.secondary",
                px: 0,
                py: 0.35
              }}
              variant="text"
              onClick={() => navigate(backToPath)}
            >
              ← Voltar
            </Button>

            <Stack
              direction="row"
              spacing={1}
              flexWrap="wrap"
              useFlexGap
              sx={{ flex: 1, justifyContent: { xs: "flex-start", sm: "flex-end" } }}
            >
              <Chip label={formatMediaType(item.mediaType)} color="secondary" size="small" />
              {!isMusicArtist && !isMusicAlbum && item.imdbRating ? (
                <Chip label={`IMDb ${item.imdbRating}`} variant="outlined" size="small" />
              ) : null}
              {isShow ? <Chip label={seasonSummaryLabel} variant="outlined" size="small" /> : null}
              {isMusicArtist ? <Chip label={`${artistAlbums.length} albuns`} variant="outlined" size="small" /> : null}
              {isMusicAlbum ? <Chip label={`${albumTracks.length} faixas`} variant="outlined" size="small" /> : null}
              {!isShow && !isMusicArtist && !isMusicAlbum ? (
                <Chip label={getMediaFormat(item.mediaPath)} variant="outlined" size="small" />
              ) : null}
            </Stack>
          </Stack>

          {errorMessage ? <Alert severity="warning">{errorMessage}</Alert> : null}

          <Stack spacing={1.5}>
            <Typography
              variant="h2"
              sx={{
                maxWidth: 520,
                fontSize: isShow ? { xs: "2.55rem", md: "3.15rem" } : undefined,
                lineHeight: isShow ? 1.03 : undefined
              }}
            >
              {item.title}
            </Typography>

            {isShow ? (
              <Typography
                color="text.secondary"
                sx={{
                  fontSize: { xs: "0.94rem", md: "1rem" },
                  letterSpacing: 0.4
                }}
              >
                {formatShowHeadlineMeta(item.year, totalEpisodeCount)}
              </Typography>
            ) : item.year !== null ? (
              <Typography
                color="text.secondary"
                sx={{
                  fontSize: { xs: "1rem", md: "1.08rem" },
                  letterSpacing: 0.4
                }}
              >
                {item.year}
              </Typography>
            ) : null}
          </Stack>

          {detailDescription ? (
            <Typography
              color="text.secondary"
              sx={{
                fontSize: isShow ? "0.93rem" : "1.02rem",
                lineHeight: isShow ? 1.6 : 1.8
              }}
            >
              {detailDescription}
            </Typography>
          ) : null}

          <Divider flexItem sx={{ borderColor: "rgba(255, 255, 255, 0.08)" }} />

          <Stack spacing={isShow ? 1.1 : 1.5}>
            <MetadataRow label={getPrimaryPathLabel(item.mediaType)} value={item.mediaPath} />
            {!isShow ? (
              <MetadataRow label={getSecondaryMetaLabel(item.mediaType)} value={getSecondaryMetaValue(item, totalEpisodeCount, artistAlbums.length, albumTracks.length)} />
            ) : null}
            {!isMusicArtist && !isMusicAlbum && item.genre ? <MetadataRow label="Genero" value={item.genre} /> : null}
            {!isMusicArtist && !isMusicAlbum && item.director ? <MetadataRow label="Direcao" value={item.director} /> : null}
            {!isMusicArtist && !isMusicAlbum && item.actors ? <MetadataRow label="Atores" value={item.actors} /> : null}
          </Stack>

          {isShow ? (
            <Stack spacing={2.5}>
              <Stack spacing={1}>
                {isLoadingShowDetail ? (
                  <Stack direction="row" alignItems="center" spacing={1.5}>
                    <CircularProgress color="secondary" size={20} />
                    <Typography color="text.secondary">Carregando episodios...</Typography>
                  </Stack>
                ) : null}

                {showDetailError ? <Alert severity="warning">{showDetailError}</Alert> : null}

                {!isLoadingShowDetail && showDetail?.seasons.length ? (
                  <>
                    <Tabs
                      value={selectedSeasonNumber ?? false}
                      onChange={(_, value: number) => setSelectedSeasonNumber(value)}
                      variant="scrollable"
                      scrollButtons="auto"
                      allowScrollButtonsMobile
                      sx={{
                        minHeight: 40,
                        ".MuiTabs-indicator": { backgroundColor: "primary.main" },
                        ".MuiTab-root": {
                          minHeight: 40,
                          color: "text.secondary"
                        }
                      }}
                    >
                      {showDetail.seasons.map((season) => (
                        <Tab
                          key={season.seasonNumber}
                          label={formatSeasonLabel(season.seasonNumber)}
                          value={season.seasonNumber}
                        />
                      ))}
                    </Tabs>

                    <Box
                      sx={{
                        borderRadius: 3,
                        border: "1px solid rgba(255,255,255,0.08)",
                        backgroundColor: "rgba(255,255,255,0.03)",
                        overflow: "hidden",
                        maxHeight: {
                          xs: 360,
                          sm: 420,
                          md: "calc(100vh - 430px)"
                        }
                      }}
                    >
                      <Stack
                        divider={<Divider flexItem sx={{ borderColor: "rgba(255,255,255,0.06)" }} />}
                        sx={{
                          overflowY: "auto",
                          maxHeight: "inherit",
                          "&::-webkit-scrollbar": {
                            width: 8
                          },
                          "&::-webkit-scrollbar-thumb": {
                            backgroundColor: "rgba(255,255,255,0.12)",
                            borderRadius: 999
                          }
                        }}
                      >
                        {selectedSeason?.episodes.map((episode) => (
                          <EpisodeRow
                            key={episode.id}
                            episode={episode}
                            continueEntry={currentShowContinueEntry?.mediaId === episode.id ? currentShowContinueEntry : null}
                            onPlay={() => void handlePlayEpisode(episode)}
                          />
                        ))}
                      </Stack>
                    </Box>
                  </>
                ) : null}

                {!isLoadingShowDetail && !showDetailError && !showDetail?.seasons.length ? (
                  <Typography color="text.secondary">Nenhum episodio encontrado para esta serie.</Typography>
                ) : null}
              </Stack>
            </Stack>
          ) : null}

          {isMusicAlbum ? (
            <Stack spacing={2.5}>
              <Stack
                direction={{ xs: "column", md: "row" }}
                spacing={2}
                alignItems={{ xs: "stretch", md: "center" }}
                useFlexGap
              >
                <Stack direction={{ xs: "column", sm: "row" }} spacing={2}>
                  <Button
                    color="primary"
                    disabled={!albumTracks.length}
                    size="large"
                    variant="contained"
                    onClick={() => void handleToggleAlbumPlayback()}
                  >
                    {isInlineAlbumPlaying ? "Pausar album" : activeAlbumTrack ? "Continuar album" : "Tocar album"}
                  </Button>

                  <Button size="large" variant="outlined" onClick={() => navigate(backToPath)}>
                    Voltar para a lista
                  </Button>
                </Stack>

                <VolumeControl
                  volume={inlineAlbumVolumeState.level}
                  isMuted={inlineAlbumVolumeState.isMuted}
                  onChange={handleInlineAlbumVolumeChange}
                  onToggleMuted={handleToggleInlineAlbumMuted}
                  disabled={!albumTracks.length}
                  sliderWidth={144}
                  sx={{
                    width: { xs: "100%", md: "auto" },
                    ml: { md: "auto" },
                    justifyContent: { xs: "space-between", md: "flex-start" }
                  }}
                />
              </Stack>

              {activeAlbumTrack ? (
                <Typography color="text.secondary" variant="body2">
                  Tocando agora: {activeAlbumTrack.title}
                </Typography>
              ) : null}

              <Typography variant="overline" sx={{ color: "text.secondary", letterSpacing: 1.6 }}>
                Playlist
              </Typography>

              {isLoadingMusicAlbumDetail ? (
                <Stack direction="row" alignItems="center" spacing={1.5}>
                  <CircularProgress color="secondary" size={20} />
                  <Typography color="text.secondary">Carregando faixas...</Typography>
                </Stack>
              ) : null}

              {musicAlbumDetailError ? <Alert severity="warning">{musicAlbumDetailError}</Alert> : null}

              {inlineAlbumPlaybackError ? <Alert severity="warning">{inlineAlbumPlaybackError}</Alert> : null}

              {!isLoadingMusicAlbumDetail && albumTracks.length ? (
                <Stack spacing={1} sx={{ maxHeight: 380, overflowY: "auto", pr: 1 }}>
                  {albumTracks.map((track) => (
                    <MusicTrackRow
                      key={track.id}
                      track={track}
                      isCurrentTrack={track.id === activeAlbumTrackId}
                      isPlaying={track.id === activeAlbumTrackId && isInlineAlbumPlaying}
                      progressPercent={track.id === activeAlbumTrackId ? inlineAlbumProgressPercent : 0}
                      progressValue={track.id === activeAlbumTrackId ? inlineAlbumPlaybackTime : 0}
                      durationValue={track.id === activeAlbumTrackId ? inlineAlbumDuration : 0}
                      elapsedLabel={track.id === activeAlbumTrackId ? formatPlaybackTime(inlineAlbumPlaybackTime) : null}
                      durationLabel={track.id === activeAlbumTrackId ? formatPlaybackTime(inlineAlbumDuration) : null}
                      onTogglePlayback={() => void handleToggleAlbumTrack(track)}
                      onSeek={handleSeekAlbumTrack}
                    />
                  ))}
                </Stack>
              ) : null}

              {!isLoadingMusicAlbumDetail && !musicAlbumDetailError && !albumTracks.length ? (
                <Typography color="text.secondary">Nenhuma faixa encontrada neste album.</Typography>
              ) : null}

              <Box component="audio"
                ref={audioRef}
                preload="metadata"
                sx={{ display: "none" }}
                onPlay={() => setIsInlineAlbumPlaying(true)}
                onPause={() => setIsInlineAlbumPlaying(false)}
                onTimeUpdate={(event) => setInlineAlbumPlaybackTime(event.currentTarget.currentTime)}
                onLoadedMetadata={(event) => setInlineAlbumDuration(Number.isFinite(event.currentTarget.duration) ? event.currentTarget.duration : 0)}
                onDurationChange={(event) => setInlineAlbumDuration(Number.isFinite(event.currentTarget.duration) ? event.currentTarget.duration : 0)}
                onEnded={() => void handleAdvanceToNextAlbumTrack()}
                onError={() => {
                  setIsInlineAlbumPlaying(false);
                  setInlineAlbumPlaybackError("Nao foi possivel reproduzir a faixa.");
                }}
              />
            </Stack>
          ) : null}

          {!isShow && !isMusicArtist && !isMusicAlbum ? (
            <Stack direction={{ xs: "column", sm: "row" }} spacing={2}>
              <Button
                color="primary"
                size="large"
                variant="contained"
                onClick={() => void launchPlayer(item)}
              >
                Reproduzir
              </Button>

              <Button size="large" variant="outlined" onClick={() => navigate(backToPath)}>
                Voltar para a lista
              </Button>
            </Stack>
          ) : null}
        </Stack>

        <Box
          sx={{
            position: "relative",
            width: { xs: "100%", md: "54%" },
            minHeight: { xs: 360, md: "auto" },
            background:
              isMusicAlbum
                ? posterUrl ?? "linear-gradient(135deg, rgba(237, 174, 73, 0.18), rgba(var(--app-accent-rgb), 0.3) 42%, rgba(var(--app-bg-rgb), 0.94) 100%)"
                : "transparent",
            backgroundSize: "cover",
            backgroundPosition: "center"
          }}
        >
          {posterUrl ? (
            <Box
              component="img"
              src={posterUrl}
              alt={item.title}
              onError={() => setHasPosterError(true)}
              sx={{
                display: { xs: "block", md: isMusicAlbum ? "block" : "none" },
                position: "absolute",
                inset: 0,
                width: "100%",
                height: "100%",
                objectFit: "cover"
              }}
            />
          ) : null}

          {!isMusicAlbum ? (
            <Box
              sx={{
                display: { xs: "block", md: "none" },
                position: "absolute",
                inset: 0,
                background: {
                  xs: "linear-gradient(to bottom, rgba(var(--app-bg-rgb), 0.04) 0%, rgba(var(--app-bg-rgb), 0.14) 38%, rgba(var(--app-bg-rgb), 0.58) 68%, rgba(var(--app-bg-rgb), 0.96) 100%)",
                  md: "none"
                }
              }}
            />
          ) : null}
        </Box>
      </Stack>
    </Box>
  );
}

type MetadataRowProps = {
  label: string;
  value: string;
};

function MetadataRow({ label, value }: MetadataRowProps) {
  return (
    <Stack spacing={0.4}>
      <Typography variant="caption" sx={{ color: "text.secondary", textTransform: "uppercase", letterSpacing: 1.4 }}>
        {label}
      </Typography>
      <Typography sx={{ wordBreak: "break-word", fontSize: "0.95rem", lineHeight: 1.45 }}>{value}</Typography>
    </Stack>
  );
}

type EpisodeCardProps = {
  episode: ShowEpisodeDto;
  continueEntry: ContinueWatchingEntry | null;
  onPlay: () => void;
};

function EpisodeRow({ episode, continueEntry, onPlay }: EpisodeCardProps) {
  const isCurrentEpisode = Boolean(continueEntry);
  const progressPercent = continueEntry?.durationSec
    ? Math.min(100, (continueEntry.progressSec / continueEntry.durationSec) * 100)
    : 0;

  return (
    <Stack
      direction={{ xs: "column", sm: "row" }}
      spacing={1.15}
      alignItems={{ xs: "flex-start", sm: "center" }}
      sx={{
        px: 1.2,
        py: 0.85,
        backgroundColor: isCurrentEpisode ? "rgba(237, 174, 73, 0.08)" : "transparent"
      }}
    >
      <Chip
        color="secondary"
        label={formatEpisodeLabel(episode)}
        size="small"
        sx={{
          flexShrink: 0,
          height: 24,
          ".MuiChip-label": {
            px: 0.85,
            fontSize: "0.7rem",
            fontWeight: 700
          }
        }}
      />

      <Stack spacing={isCurrentEpisode ? 0.55 : 0.35} sx={{ minWidth: 0, flex: 1 }}>
        <Typography
          variant="subtitle1"
          sx={{
            fontWeight: 700,
            fontSize: "0.94rem",
            lineHeight: 1.25,
            color: isCurrentEpisode ? "rgba(255,255,255,0.98)" : undefined
          }}
        >
          {episode.title}
        </Typography>

        {isCurrentEpisode ? (
          <LinearProgress
            color="secondary"
            value={progressPercent}
            variant="determinate"
            sx={{
              maxWidth: 220,
              height: 6,
              borderRadius: 999,
              backgroundColor: "rgba(255,255,255,0.08)"
            }}
          />
        ) : null}
      </Stack>

      <IconButton
        aria-label={`${isCurrentEpisode ? "Continuar" : "Assistir"} ${episode.title}`}
        onClick={onPlay}
        sx={{
          alignSelf: { xs: "stretch", sm: "center" },
          flexShrink: 0,
          width: 34,
          height: 34,
          color: "common.white",
          backgroundColor: isCurrentEpisode ? "secondary.main" : "primary.main",
          borderRadius: 999,
          ":hover": {
            backgroundColor: isCurrentEpisode ? "secondary.dark" : "primary.dark"
          }
        }}
      >
        <PlayArrowRoundedIcon fontSize="small" />
      </IconButton>
    </Stack>
  );
}

type MusicTrackRowProps = {
  track: MusicTrackDto;
  isCurrentTrack: boolean;
  isPlaying: boolean;
  progressPercent: number;
  progressValue: number;
  durationValue: number;
  elapsedLabel: string | null;
  durationLabel: string | null;
  onTogglePlayback: () => void;
  onSeek: (nextTime: number) => void;
};

function MusicTrackRow({
  track,
  isCurrentTrack,
  isPlaying,
  progressPercent,
  progressValue,
  durationValue,
  elapsedLabel,
  durationLabel,
  onTogglePlayback,
  onSeek
}: MusicTrackRowProps) {
  return (
    <Stack
      direction="row"
      spacing={1}
      alignItems="center"
      sx={{
        px: 1.25,
        py: 0.9,
        borderRadius: 2.5,
        border: "1px solid rgba(255,255,255,0.08)",
        backgroundColor: isCurrentTrack ? "rgba(255,255,255,0.06)" : "rgba(255,255,255,0.03)",
        transition: "background-color 160ms ease, border-color 160ms ease",
        ":hover": {
          backgroundColor: "rgba(255,255,255,0.05)",
          borderColor: "rgba(255,255,255,0.14)"
        }
      }}
    >
      <Typography variant="caption" sx={{ minWidth: 34, color: "rgba(255,255,255,0.46)" }}>
        {formatTrackLabel(track)}
      </Typography>

      <Stack spacing={0.65} sx={{ minWidth: 0, flex: 1 }}>
        <Typography sx={{ fontWeight: 600 }} noWrap>
          {track.title}
        </Typography>

        {isCurrentTrack ? (
          <Stack direction="row" spacing={1} alignItems="center" sx={{ minWidth: 0 }}>
            <Slider
              aria-label={`Progresso de ${track.title}`}
              color="secondary"
              min={0}
              max={Math.max(durationValue, 1)}
              step={1}
              value={Math.min(progressValue, Math.max(durationValue, 1))}
              onChangeCommitted={(_, value) => onSeek(Array.isArray(value) ? value[0] : value)}
              sx={{
                flex: 1,
                py: 0,
                color: "secondary.main",
                '& .MuiSlider-rail': {
                  opacity: 1,
                  backgroundColor: "rgba(255,255,255,0.08)",
                  height: 5,
                  borderRadius: 999
                },
                '& .MuiSlider-track': {
                  height: 5,
                  border: "none",
                  borderRadius: 999,
                  opacity: durationValue > 0 ? 1 : 0.5
                },
                '& .MuiSlider-thumb': {
                  width: 10,
                  height: 10,
                  transition: "transform 120ms ease",
                  '&:hover, &.Mui-focusVisible, &.Mui-active': {
                    boxShadow: "0 0 0 6px rgba(237, 174, 73, 0.16)"
                  }
                },
                '& .MuiSlider-mark': {
                  display: "none"
                }
              }}
            />

            <Typography variant="caption" sx={{ color: "rgba(255,255,255,0.6)", minWidth: 40, textAlign: "right" }}>
              {elapsedLabel ?? "0:00"}
            </Typography>

            <Typography variant="caption" sx={{ color: "rgba(255,255,255,0.36)", minWidth: 40, textAlign: "right" }}>
              {durationLabel ?? "0:00"}
            </Typography>
          </Stack>
        ) : null}
      </Stack>

      <IconButton
        aria-label={`${isCurrentTrack && isPlaying ? "Pausar" : "Tocar"} ${track.title}`}
        onClick={onTogglePlayback}
        sx={{
          width: 36,
          height: 36,
          color: "common.white",
          backgroundColor: isCurrentTrack && isPlaying ? "secondary.main" : "primary.main",
          ":hover": {
            backgroundColor: isCurrentTrack && isPlaying ? "secondary.dark" : "primary.dark"
          }
        }}
      >
        {isCurrentTrack && isPlaying ? <PauseRoundedIcon fontSize="small" /> : <PlayArrowRoundedIcon fontSize="small" />}
      </IconButton>
    </Stack>
  );
}

function formatEpisodeLabel(episode: ShowEpisodeDto) {
  if (episode.seasonNumber === 0) {
    return episode.episodeNumber ? `Extra ${String(episode.episodeNumber).padStart(2, "0")}` : "Extra";
  }

  const episodePart = episode.episodeNumber ? `E${String(episode.episodeNumber).padStart(2, "0")}` : "EP";
  return `T${String(episode.seasonNumber).padStart(2, "0")} ${episodePart}`;
}

function buildEpisodePlaybackTitle(showTitle: string, episode: ShowEpisodeDto) {
  const parts = [showTitle, formatEpisodeLabel(episode), episode.title].filter(Boolean);

  return parts.join(" - ");
}

function formatSeasonLabel(seasonNumber: number) {
  return seasonNumber === 0 ? "Extras" : `Temporada ${seasonNumber}`;
}

function formatTrackLabel(track: MusicTrackDto) {
  return track.trackNumber ? `Faixa ${String(track.trackNumber).padStart(2, "0")}` : "Faixa";
}

function formatShowHeadlineMeta(year: number | null, totalEpisodeCount: number) {
  const parts: string[] = [];

  if (year !== null) {
    parts.push(String(year));
  }

  if (totalEpisodeCount > 0) {
    parts.push(`${totalEpisodeCount} episódio${totalEpisodeCount === 1 ? "" : "s"}`);
  }

  return parts.join(" - ");
}

function formatPlaybackTime(value: number) {
  if (!Number.isFinite(value) || value <= 0) {
    return "0:00";
  }

  const totalSeconds = Math.floor(value);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

function getSeasonSummaryLabel(showDetail: ShowDetailDto | null) {
  if (!showDetail) {
    return "0 temporadas";
  }

  const regularSeasonCount = showDetail.seasons.filter((season) => season.seasonNumber > 0).length;
  const hasExtras = showDetail.seasons.some((season) => season.seasonNumber === 0);

  if (regularSeasonCount === 0 && hasExtras) {
    return "Extras";
  }

  if (hasExtras) {
    return `${regularSeasonCount} temporadas + extras`;
  }

  return `${regularSeasonCount} temporadas`;
}

function compareShowSeasons(left: ShowDetailDto["seasons"][number], right: ShowDetailDto["seasons"][number]) {
  if (left.seasonNumber === 0 && right.seasonNumber !== 0) {
    return 1;
  }

  if (left.seasonNumber !== 0 && right.seasonNumber === 0) {
    return -1;
  }

  return left.seasonNumber - right.seasonNumber;
}

function compareAlbumsByYear(left: HomeMediaItemDto, right: HomeMediaItemDto) {
  const leftYear = left.year ?? Number.POSITIVE_INFINITY;
  const rightYear = right.year ?? Number.POSITIVE_INFINITY;

  if (leftYear !== rightYear) {
    return leftYear - rightYear;
  }

  return left.title.localeCompare(right.title, "pt-BR");
}

function getPrimaryPathLabel(mediaType: string) {
  switch (mediaType) {
    case "show":
      return "Pasta da serie";
    case "music_artist":
      return "Pasta do artista";
    case "music_album":
      return "Pasta do album";
    default:
      return "Caminho";
  }
}

function getSecondaryMetaLabel(mediaType: string) {
  switch (mediaType) {
    case "show":
      return "Episódios";
    case "music_artist":
      return "Álbuns";
    case "music_album":
      return "Faixas";
    default:
      return "Formato";
  }
}

function getSecondaryMetaValue(
  item: HomeMediaItemDto,
  totalEpisodeCount: number,
  artistAlbumCount: number,
  albumTrackCount: number
) {
  switch (item.mediaType) {
    case "show":
      return String(totalEpisodeCount);
    case "music_artist":
      return String(artistAlbumCount);
    case "music_album":
      return String(albumTrackCount);
    default:
      return getMediaFormat(item.mediaPath);
  }
}

function getDefaultBackToPath(item: HomeMediaItemDto | null) {
  if (!item) {
    return "/";
  }

  if (item.mediaType === "music_artist") {
    return "/music";
  }

  if (item.mediaType === "music_album") {
    const artistPath = getParentDirectoryPath(item.mediaPath);
    return artistPath ? `/library/${encodeURIComponent(artistPath)}` : "/music";
  }

  return "/";
}

function getArtistNameFromAlbumPath(mediaPath: string) {
  const artistPath = getParentDirectoryPath(mediaPath);

  if (!artistPath) {
    return null;
  }

  return artistPath.split(/[\\/]/).filter(Boolean).pop() ?? null;
}

function getParentDirectoryPath(mediaPath: string) {
  const normalized = mediaPath.replace(/\\/g, "/").replace(/\/+$/, "");
  const lastSlashIndex = normalized.lastIndexOf("/");

  if (lastSlashIndex <= 0) {
    return null;
  }

  return normalized.slice(0, lastSlashIndex).replace(/\//g, "\\");
}

function asMessage(error: unknown, fallback: string) {
  if (error instanceof Error && error.message) {
    return error.message;
  }

  if (typeof error === "string" && error) {
    return error;
  }

  return fallback;
}