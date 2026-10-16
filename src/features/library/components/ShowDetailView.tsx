import { RefreshDialog } from "@/features/library/components/RefreshDialog";
import { getDisplayTitle, getOriginalTitle } from "@/features/library/utils/titles";
import { ContinueWatchingEntry } from "@/features/player/services/continueWatching";
import { fetchEpisodeSubtitles } from "@/services/tauri/commands/library";
import { EXTRAS_SEASON, HomeMediaItemDto, ShowDetailDto, ShowEpisodeDto } from "@/types/contracts/library";
import BookmarkBorderRoundedIcon from "@mui/icons-material/BookmarkBorderRounded";
import AutoFixHighRoundedIcon from "@mui/icons-material/AutoFixHighRounded";
import BookmarkRoundedIcon from "@mui/icons-material/BookmarkRounded";
import ClosedCaptionRoundedIcon from "@mui/icons-material/ClosedCaptionRounded";
import DownloadRoundedIcon from "@mui/icons-material/DownloadRounded";
import FolderOpenRoundedIcon from "@mui/icons-material/FolderOpenRounded";
import PlayArrowRoundedIcon from "@mui/icons-material/PlayArrowRounded";
import {
  Alert,
  Box,
  Button,
  ButtonBase,
  CircularProgress,
  IconButton,
  Stack,
  Tooltip,
  Typography
} from "@mui/material";
import { motion } from "framer-motion";
import { useEffect, useMemo, useRef, useState, type MouseEvent, type ReactNode, type SyntheticEvent } from "react";

type ShowDetailViewProps = {
  item: HomeMediaItemDto;
  posterUrl: string | null;
  detail: ShowDetailDto | null;
  isLoading: boolean;
  errorMessage: string | null;
  continueEntry: ContinueWatchingEntry | null;
  selectedSeasonNumber: number | null;
  isInterested: boolean | null;
  onSelectSeason: (seasonNumber: number) => void;
  onPlayEpisode: (episode: ShowEpisodeDto) => void;
  onToggleInterest: () => void;
  onOpenFolder: () => void;
  onPosterError: () => void;
  onRefreshed: () => void;
  onBack: () => void;
};

export function ShowDetailView({
  item,
  posterUrl,
  detail,
  isLoading,
  errorMessage,
  continueEntry,
  selectedSeasonNumber,
  isInterested,
  onSelectSeason,
  onPlayEpisode,
  onToggleInterest,
  onOpenFolder,
  onPosterError,
  onRefreshed,
  onBack
}: ShowDetailViewProps) {
  // Which tab "Atualizar" opens on; null is closed.
  const [refreshTab, setRefreshTab] = useState<"metadata" | "subtitles" | null>(null);
  const [isOverviewExpanded, setIsOverviewExpanded] = useState(false);
  const [isWidePoster, setIsWidePoster] = useState(true);
  const seasons = detail?.seasons ?? [];
  const selectedSeason = seasons.find((season) => season.seasonNumber === selectedSeasonNumber) ?? seasons[0] ?? null;
  const regularEpisodes = useMemo(
    () => seasons.filter((season) => season.seasonNumber > 0).flatMap((season) => season.episodes),
    [seasons]
  );
  const episodeCount = seasons.reduce((total, season) => total + season.episodes.length, 0);
  const regularSeasonCount = seasons.filter((season) => season.seasonNumber > 0).length;

  // The hero button resumes the episode in progress, otherwise starts from the top.
  const continueEpisode = continueEntry
    ? seasons.flatMap((season) => season.episodes).find((episode) => episode.id === continueEntry.mediaId) ?? null
    : null;
  const heroEpisode = continueEpisode ?? regularEpisodes[0] ?? seasons[0]?.episodes[0] ?? null;
  const heroProgress = continueEpisode && continueEntry ? progressPercent(continueEntry) : 0;

  const displayTitle = getDisplayTitle(item);
  // Only worth showing when it differs from the title above it.
  const originalTitle = getOriginalTitle(item) !== displayTitle ? getOriginalTitle(item) : null;
  const metaParts = [
    item.year !== null ? String(item.year) : null,
    item.genre,
    item.country,
    regularSeasonCount > 0 ? `${regularSeasonCount} temporada${regularSeasonCount === 1 ? "" : "s"}` : null,
    episodeCount > 0 ? `${episodeCount} episódio${episodeCount === 1 ? "" : "s"}` : null
  ].filter(Boolean);

  const handlePosterLoad = (event: SyntheticEvent<HTMLImageElement>) => {
    const { naturalWidth, naturalHeight } = event.currentTarget;
    setIsWidePoster(naturalHeight === 0 || naturalWidth / naturalHeight >= 1.15);
  };

  return (
    <Box
      sx={{
        position: "relative",
        minHeight: "calc(100vh - 88px)",
        mx: -3,
        my: -3,
        overflow: "hidden",
        backgroundColor: "var(--app-bg)"
      }}
    >
      {/* Hero */}
      <Box sx={{ position: "relative", minHeight: { xs: 420, md: "min(64vh, 640px)" }, display: "flex" }}>
        {posterUrl ? (
          <Box
            component="img"
            src={posterUrl}
            alt=""
            onLoad={handlePosterLoad}
            onError={onPosterError}
            sx={{
              position: "absolute",
              top: 0,
              right: 0,
              height: "100%",
              width: { xs: "100%", md: isWidePoster ? "74%" : "auto" },
              objectFit: "cover",
              objectPosition: "center 20%",
              opacity: { xs: 0.45, md: 1 },
              WebkitMaskImage: "linear-gradient(to right, transparent 0%, #000 38%), linear-gradient(to top, transparent 0%, #000 34%)",
              WebkitMaskComposite: "source-in",
              maskImage: "linear-gradient(to right, transparent 0%, #000 38%), linear-gradient(to top, transparent 0%, #000 34%)",
              maskComposite: "intersect",
              animation: "netcricoHeroIn 900ms ease-out both",
              "@keyframes netcricoHeroIn": {
                from: { opacity: 0, transform: "scale(1.04)" },
                to: { transform: "scale(1)" }
              }
            }}
          />
        ) : (
          <Box
            sx={{
              position: "absolute",
              inset: 0,
              background: "radial-gradient(circle at 80% 20%, rgba(237, 174, 73, 0.16), transparent 50%), radial-gradient(circle at 60% 80%, rgba(var(--app-accent-rgb), 0.18), transparent 55%)"
            }}
          />
        )}

        <Stack
          spacing={2.25}
          sx={{
            position: "relative",
            zIndex: 1,
            justifyContent: "space-between",
            width: "100%",
            px: { xs: 2.5, md: 6 },
            pt: { xs: 2, md: 3 },
            pb: { xs: 3, md: 5 }
          }}
        >
          <Button
            variant="text"
            onClick={onBack}
            sx={{ alignSelf: "flex-start", minWidth: 0, px: 0, color: "text.secondary", ":hover": { color: "common.white", background: "none" } }}
          >
            ← Voltar
          </Button>

          <Stack spacing={2} sx={{ maxWidth: 620 }}>
            <Typography
              component="h1"
              sx={{
                fontSize: { xs: "2.6rem", md: "clamp(3rem, 5.4vw, 4.6rem)" },
                fontWeight: 800,
                lineHeight: 0.95,
                letterSpacing: "-0.03em",
                color: "common.white",
                textShadow: "0 4px 32px rgba(0,0,0,0.45)"
              }}
            >
              {displayTitle}
            </Typography>

            {originalTitle ? (
              <Typography sx={{ color: "text.secondary", fontStyle: "italic", mt: "-4px !important" }}>{originalTitle}</Typography>
            ) : null}

            <Stack direction="row" alignItems="center" useFlexGap flexWrap="wrap" sx={{ columnGap: 1.25, rowGap: 0.5, color: "rgba(255,255,255,0.7)", fontSize: "0.95rem" }}>
              {item.imdbRating ? (
                <Box component="span" sx={{ color: "secondary.main", fontWeight: 700 }}>
                  ★ {item.imdbRating}
                </Box>
              ) : null}
              {metaParts.map((part, index) => (
                <Box component="span" key={part} sx={{ display: "inline-flex", alignItems: "center", gap: 1.25 }}>
                  {index > 0 || item.imdbRating ? <Box component="span" sx={{ opacity: 0.35 }}>·</Box> : null}
                  {part}
                </Box>
              ))}
            </Stack>

            {item.overview?.trim() ? (
              <Typography
                onClick={() => setIsOverviewExpanded((expanded) => !expanded)}
                title={isOverviewExpanded ? undefined : "Ler mais"}
                sx={{
                  color: "rgba(255,255,255,0.72)",
                  lineHeight: 1.6,
                  cursor: "pointer",
                  display: "-webkit-box",
                  WebkitBoxOrient: "vertical",
                  WebkitLineClamp: isOverviewExpanded ? "unset" : 3,
                  overflow: "hidden"
                }}
              >
                {item.overview}
              </Typography>
            ) : null}

            {item.director || item.actors ? (
              <Stack spacing={0.4} sx={{ fontSize: "0.85rem", color: "text.secondary" }}>
                {item.director ? <CreditLine label="Criação" value={item.director} /> : null}
                {item.actors ? <CreditLine label="Elenco" value={item.actors} /> : null}
              </Stack>
            ) : null}

            <Stack direction="row" spacing={1.25} alignItems="center" sx={{ pt: 1 }}>
              <Button
                disabled={!heroEpisode}
                onClick={() => heroEpisode && onPlayEpisode(heroEpisode)}
                startIcon={<PlayArrowRoundedIcon sx={{ fontSize: "1.6rem !important" }} />}
                sx={{
                  position: "relative",
                  overflow: "hidden",
                  pl: 2.25,
                  pr: 3,
                  py: 1.1,
                  borderRadius: 999,
                  fontWeight: 800,
                  fontSize: "1rem",
                  textTransform: "none",
                  color: "#16120a",
                  backgroundColor: "secondary.main",
                  boxShadow: "0 10px 32px rgba(237, 174, 73, 0.28)",
                  transition: "transform 160ms ease, box-shadow 160ms ease",
                  ":hover": { backgroundColor: "secondary.light", transform: "translateY(-1px) scale(1.02)", boxShadow: "0 14px 40px rgba(237, 174, 73, 0.36)" },
                  ":active": { transform: "scale(0.98)" }
                }}
              >
                {continueEpisode ? "Continuar" : "Assistir"}
                {heroEpisode ? (
                  <Box component="span" sx={{ ml: 1, fontWeight: 600, opacity: 0.7 }}>
                    {formatEpisodeCode(heroEpisode)}
                  </Box>
                ) : null}
                {heroProgress > 0 ? (
                  <Box sx={{ position: "absolute", left: 0, bottom: 0, height: 3, width: `${heroProgress}%`, backgroundColor: "rgba(22, 18, 10, 0.55)" }} />
                ) : null}
              </Button>

              {isInterested !== null ? (
                <Tooltip title={isInterested ? "Remover de Novos" : "Marcar como novo"}>
                  <HeroIconButton onClick={onToggleInterest} isActive={isInterested}>
                    {isInterested ? <BookmarkRoundedIcon /> : <BookmarkBorderRoundedIcon />}
                  </HeroIconButton>
                </Tooltip>
              ) : null}

              <Tooltip title="Atualizar metadata, imagens e legendas">
                <HeroIconButton onClick={() => setRefreshTab("metadata")} spinOnHover>
                  <AutoFixHighRoundedIcon />
                </HeroIconButton>
              </Tooltip>

              <Tooltip title="Baixar legendas">
                <HeroIconButton onClick={() => setRefreshTab("subtitles")}>
                  <ClosedCaptionRoundedIcon />
                </HeroIconButton>
              </Tooltip>

              <Tooltip title={item.mediaPath}>
                <HeroIconButton onClick={onOpenFolder}>
                  <FolderOpenRoundedIcon />
                </HeroIconButton>
              </Tooltip>
            </Stack>
          </Stack>
        </Stack>
      </Box>

      <RefreshDialog
        open={refreshTab !== null}
        initialTab={refreshTab ?? "metadata"}
        item={item}
        seasons={seasons.map((season) => season.seasonNumber)}
        initialSeason={selectedSeason?.seasonNumber ?? null}
        onClose={() => setRefreshTab(null)}
        onApplied={onRefreshed}
      />

      {/* Seasons and episodes */}
      <Stack spacing={2.5} useFlexGap sx={{ position: "relative", zIndex: 1, px: { xs: 2.5, md: 6 }, pb: 6 }}>
        {errorMessage ? <Alert severity="warning">{errorMessage}</Alert> : null}

        {isLoading ? (
          <Stack direction="row" alignItems="center" spacing={1.5}>
            <CircularProgress color="secondary" size={18} />
            <Typography color="text.secondary">Carregando episódios...</Typography>
          </Stack>
        ) : null}

        {!isLoading && seasons.length ? (
          <>
            <SeasonRail
              seasons={seasons.map((season) => season.seasonNumber)}
              selectedSeasonNumber={selectedSeason?.seasonNumber ?? null}
              continueSeasonNumber={continueEpisode?.seasonNumber ?? null}
              onSelect={onSelectSeason}
            />

            {selectedSeason ? (
              <Stack spacing={1.5}>
                <Stack direction="row" alignItems="baseline" spacing={1.5}>
                  <Typography sx={{ fontSize: "1.35rem", fontWeight: 800, color: "common.white", letterSpacing: "-0.01em" }}>
                    {formatSeasonLabel(selectedSeason.seasonNumber)}
                  </Typography>
                  <Typography color="text.secondary" sx={{ fontSize: "0.9rem" }}>
                    {selectedSeason.episodes.length} {selectedSeason.episodes.length === 1 ? "episódio" : "episódios"}
                  </Typography>
                </Stack>

                <Box
                  key={selectedSeason.seasonNumber}
                  sx={{ display: "grid", gap: 1, gridTemplateColumns: { xs: "1fr", md: "repeat(auto-fill, minmax(380px, 1fr))" } }}
                >
                  {selectedSeason.episodes.map((episode, index) => (
                    <EpisodeCard
                      key={episode.id}
                      episode={episode}
                      index={index}
                      continueEntry={continueEpisode?.id === episode.id ? continueEntry : null}
                      showId={item.id}
                      onPlay={() => onPlayEpisode(episode)}
                      onSubtitlesDownloaded={onRefreshed}
                    />
                  ))}
                </Box>
              </Stack>
            ) : null}
          </>
        ) : null}

        {!isLoading && !errorMessage && !seasons.length ? (
          <Typography color="text.secondary">Nenhum episódio encontrado para esta série.</Typography>
        ) : null}
      </Stack>
    </Box>
  );
}

function CreditLine({ label, value }: { label: string; value: string }) {
  return (
    <Typography sx={{ fontSize: "inherit", color: "inherit", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
      <Box component="span" sx={{ color: "rgba(255,255,255,0.38)", mr: 1 }}>
        {label}
      </Box>
      {value}
    </Typography>
  );
}

type HeroIconButtonProps = {
  children: ReactNode;
  onClick: (event: MouseEvent<HTMLElement>) => void;
  disabled?: boolean;
  isActive?: boolean;
  spinOnHover?: boolean;
};

function HeroIconButton({ children, onClick, disabled, isActive, spinOnHover }: HeroIconButtonProps) {
  return (
    <IconButton
      disabled={disabled}
      onClick={onClick}
      sx={{
        width: 46,
        height: 46,
        color: isActive ? "secondary.main" : "rgba(255,255,255,0.86)",
        border: "1px solid",
        borderColor: isActive ? "rgba(237, 174, 73, 0.5)" : "rgba(255,255,255,0.18)",
        backgroundColor: "rgba(var(--app-bg-rgb), 0.45)",
        backdropFilter: "blur(8px)",
        transition: "transform 160ms ease, background-color 160ms ease, border-color 160ms ease",
        ":hover": {
          backgroundColor: "rgba(255,255,255,0.1)",
          borderColor: "rgba(255,255,255,0.32)",
          transform: "translateY(-1px)"
        },
        "& svg": { transition: "transform 420ms cubic-bezier(.3,1.6,.5,1)" },
        ...(spinOnHover ? { ":hover svg": { transform: "rotate(200deg)" } } : {})
      }}
    >
      {children}
    </IconButton>
  );
}

type SeasonRailProps = {
  seasons: number[];
  selectedSeasonNumber: number | null;
  continueSeasonNumber: number | null;
  onSelect: (seasonNumber: number) => void;
};

function SeasonRail({ seasons, selectedSeasonNumber, continueSeasonNumber, onSelect }: SeasonRailProps) {
  const railRef = useRef<HTMLDivElement | null>(null);

  // Keep the selected season visible when a long rail opens on a late season.
  useEffect(() => {
    railRef.current
      ?.querySelector<HTMLElement>("[data-selected='true']")
      ?.scrollIntoView({ block: "nearest", inline: "center", behavior: "smooth" });
  }, [selectedSeasonNumber]);

  return (
    <Box
      ref={railRef}
      sx={{
        display: "flex",
        gap: 0.75,
        overflowX: "auto",
        py: 0.5,
        mx: { xs: -2.5, md: -6 },
        px: { xs: 2.5, md: 6 },
        scrollbarWidth: "none",
        "&::-webkit-scrollbar": { display: "none" },
        WebkitMaskImage: "linear-gradient(to right, transparent 0, #000 32px, #000 calc(100% - 32px), transparent 100%)",
        maskImage: "linear-gradient(to right, transparent 0, #000 32px, #000 calc(100% - 32px), transparent 100%)"
      }}
    >
      {seasons.map((seasonNumber, index) => {
        const isSelected = seasonNumber === selectedSeasonNumber;
        const isNamed = seasonNumber <= 0;
        const startsNamedGroup = isNamed && index > 0 && seasons[index - 1] > 0;

        return (
          <Box key={seasonNumber} sx={{ display: "flex", alignItems: "center", flexShrink: 0 }}>
            {startsNamedGroup ? <Box sx={{ width: "1px", height: 22, mx: 1, backgroundColor: "rgba(255,255,255,0.14)" }} /> : null}
            <ButtonBase
              data-selected={isSelected}
              onClick={() => onSelect(seasonNumber)}
              title={formatSeasonLabel(seasonNumber)}
              sx={{
                position: "relative",
                minWidth: 42,
                height: 42,
                px: isNamed ? 2 : 1.25,
                borderRadius: 999,
                fontSize: "0.95rem",
                fontWeight: isSelected ? 800 : 600,
                color: isSelected ? "#16120a" : "rgba(255,255,255,0.7)",
                backgroundColor: isSelected ? "secondary.main" : "rgba(255,255,255,0.05)",
                transition: "background-color 160ms ease, color 160ms ease, transform 160ms ease",
                ":hover": {
                  backgroundColor: isSelected ? "secondary.main" : "rgba(255,255,255,0.12)",
                  color: isSelected ? "#16120a" : "common.white",
                  transform: "translateY(-1px)"
                }
              }}
            >
              {isNamed ? formatSeasonLabel(seasonNumber) : seasonNumber}
              {seasonNumber === continueSeasonNumber ? (
                <Box
                  sx={{
                    position: "absolute",
                    top: 3,
                    right: 3,
                    width: 7,
                    height: 7,
                    borderRadius: "50%",
                    backgroundColor: isSelected ? "#16120a" : "secondary.main"
                  }}
                />
              ) : null}
            </ButtonBase>
          </Box>
        );
      })}
    </Box>
  );
}

type EpisodeCardProps = {
  episode: ShowEpisodeDto;
  index: number;
  continueEntry: ContinueWatchingEntry | null;
  showId: string;
  onPlay: () => void;
  onSubtitlesDownloaded: () => void;
};

function EpisodeCard({ episode, index, continueEntry, showId, onPlay, onSubtitlesDownloaded }: EpisodeCardProps) {
  const [subtitleState, setSubtitleState] = useState<{ status: "idle" | "loading" | "error"; message?: string }>({ status: "idle" });

  // One click: OpenSubtitles' best match for each configured language.
  async function handleFetchSubtitles() {
    setSubtitleState({ status: "loading" });
    try {
      const result = await fetchEpisodeSubtitles(showId, episode.id);
      if (result.downloaded > 0) {
        setSubtitleState({ status: "idle" });
        onSubtitlesDownloaded();
      } else {
        setSubtitleState({ status: "error", message: result.errors.join("\n") || "Nada baixado." });
      }
    } catch (error) {
      setSubtitleState({ status: "error", message: typeof error === "string" ? error : "Não foi possível baixar." });
    }
  }

  const progress = continueEntry ? progressPercent(continueEntry) : 0;
  const minutesLeft = continueEntry ? Math.max(1, Math.round((continueEntry.durationSec - continueEntry.progressSec) / 60)) : null;
  const marker = episode.seasonNumber === EXTRAS_SEASON
    ? "+"
    : episode.episodeNumber !== null
      ? String(episode.episodeNumber).padStart(2, "0")
      : "•";

  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.28, delay: Math.min(index, 14) * 0.025, ease: "easeOut" }}
      style={{ position: "relative" }}
    >
      <ButtonBase
        onClick={onPlay}
        sx={{
          width: "100%",
          justifyContent: "flex-start",
          textAlign: "left",
          gap: 2,
          pl: 2,
          // Room for the download button that sits over the card.
          pr: episode.subtitlePath || episode.seasonNumber === EXTRAS_SEASON ? 2 : 6,
          py: 1.5,
          borderRadius: 3,
          border: "1px solid",
          borderColor: continueEntry ? "rgba(237, 174, 73, 0.45)" : "rgba(255,255,255,0.06)",
          backgroundColor: continueEntry ? "rgba(237, 174, 73, 0.08)" : "rgba(255,255,255,0.025)",
          transition: "background-color 160ms ease, border-color 160ms ease, transform 160ms ease",
          "& .episode-number": { transition: "opacity 160ms ease, transform 200ms ease" },
          "& .episode-play": { opacity: 0, transform: "scale(0.6)", transition: "opacity 160ms ease, transform 220ms cubic-bezier(.3,1.6,.5,1)" },
          ":hover, &.Mui-focusVisible": {
            backgroundColor: continueEntry ? "rgba(237, 174, 73, 0.13)" : "rgba(255,255,255,0.07)",
            borderColor: continueEntry ? "rgba(237, 174, 73, 0.7)" : "rgba(255,255,255,0.16)",
            transform: "translateY(-2px)",
            "& .episode-number": { opacity: 0, transform: "scale(0.7)" },
            "& .episode-play": { opacity: 1, transform: "scale(1)" }
          }
        }}
      >
        <Box sx={{ position: "relative", width: 52, height: 44, flexShrink: 0, display: "grid", placeItems: "center" }}>
          <Typography
            className="episode-number"
            sx={{
              fontSize: "1.9rem",
              fontWeight: 800,
              lineHeight: 1,
              letterSpacing: "-0.04em",
              fontVariantNumeric: "tabular-nums",
              color: continueEntry ? "secondary.main" : "rgba(255,255,255,0.22)"
            }}
          >
            {marker}
          </Typography>
          <Box
            className="episode-play"
            sx={{
              position: "absolute",
              width: 40,
              height: 40,
              borderRadius: "50%",
              display: "grid",
              placeItems: "center",
              color: "#16120a",
              backgroundColor: "secondary.main"
            }}
          >
            <PlayArrowRoundedIcon />
          </Box>
        </Box>

        <Stack spacing={0.6} sx={{ minWidth: 0, flex: 1 }}>
          <Typography
            sx={{
              fontWeight: 700,
              fontSize: "0.98rem",
              lineHeight: 1.3,
              color: "rgba(255,255,255,0.92)",
              display: "-webkit-box",
              WebkitBoxOrient: "vertical",
              WebkitLineClamp: 2,
              overflow: "hidden"
            }}
          >
            {episode.title}
          </Typography>

          {continueEntry || episode.subtitlePath ? (
            <Stack direction="row" alignItems="center" spacing={1.25}>
              {continueEntry ? (
                <>
                  <Box sx={{ width: 96, height: 4, borderRadius: 999, overflow: "hidden", backgroundColor: "rgba(255,255,255,0.12)" }}>
                    <Box sx={{ width: `${progress}%`, height: "100%", backgroundColor: "secondary.main" }} />
                  </Box>
                  <Typography sx={{ fontSize: "0.78rem", color: "secondary.main", fontWeight: 600 }}>
                    faltam {minutesLeft} min
                  </Typography>
                </>
              ) : null}
              {episode.subtitlePath ? (
                <Box
                  component="span"
                  title="Com legenda"
                  sx={{
                    px: 0.6,
                    borderRadius: 0.75,
                    border: "1px solid rgba(255,255,255,0.28)",
                    fontSize: "0.62rem",
                    fontWeight: 800,
                    letterSpacing: 0.5,
                    lineHeight: "15px",
                    color: "rgba(255,255,255,0.6)"
                  }}
                >
                  CC
                </Box>
              ) : null}
            </Stack>
          ) : null}
        </Stack>
      </ButtonBase>

      {/* Outside the card's button: a button can't hold another one. */}
      {!episode.subtitlePath && episode.seasonNumber !== EXTRAS_SEASON ? (
        <Tooltip title={subtitleState.status === "error" ? subtitleState.message ?? "" : "Baixar legenda"}>
          <span style={{ position: "absolute", right: 10, top: "50%", transform: "translateY(-50%)" }}>
            <IconButton
              size="small"
              disabled={subtitleState.status === "loading"}
              onClick={() => void handleFetchSubtitles()}
              sx={{
                color: subtitleState.status === "error" ? "warning.main" : "rgba(255,255,255,0.45)",
                ":hover": { color: "secondary.main", backgroundColor: "rgba(255,255,255,0.06)" }
              }}
            >
              {subtitleState.status === "loading" ? <CircularProgress size={16} color="inherit" /> : <DownloadRoundedIcon fontSize="small" />}
            </IconButton>
          </span>
        </Tooltip>
      ) : null}
    </motion.div>
  );
}

function progressPercent(entry: ContinueWatchingEntry) {
  return entry.durationSec > 0 ? Math.min(100, (entry.progressSec / entry.durationSec) * 100) : 0;
}

function formatEpisodeCode(episode: ShowEpisodeDto) {
  if (episode.seasonNumber === EXTRAS_SEASON) {
    return "Extra";
  }

  const episodePart = episode.episodeNumber !== null ? `E${String(episode.episodeNumber).padStart(2, "0")}` : "";
  return episode.seasonNumber === 0 ? `Especial ${episodePart}`.trim() : `T${episode.seasonNumber} ${episodePart}`.trim();
}

function formatSeasonLabel(seasonNumber: number) {
  if (seasonNumber === EXTRAS_SEASON) {
    return "Extras";
  }

  return seasonNumber === 0 ? "Especiais" : `Temporada ${seasonNumber}`;
}
