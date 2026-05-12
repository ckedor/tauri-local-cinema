import {
    ContinueWatchingEntry,
    ContinueWatchingMediaType,
    toPlayerSessionFromContinueWatching
} from "@/features/player/services/continueWatching";
import { launchPlayerSession } from "@/features/player/services/launchPlayer";
import { useContinueWatchingStore } from "@/features/player/store/continueWatching.store";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import PlayArrowRoundedIcon from "@mui/icons-material/PlayArrowRounded";
import { Box, IconButton, LinearProgress, Stack, Typography } from "@mui/material";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useMemo } from "react";

type ContinueWatchingRailProps = {
  mediaType?: ContinueWatchingMediaType;
  mediaTypes?: ContinueWatchingMediaType[];
  title?: string;
};

export function ContinueWatchingRail({ mediaType, mediaTypes, title = "Continue assistindo" }: ContinueWatchingRailProps) {
  const entries = useContinueWatchingStore((state) => state.entries);
  const removeEntry = useContinueWatchingStore((state) => state.removeEntry);
  const activeMediaTypes = useMemo(() => {
    if (mediaTypes?.length) {
      return mediaTypes;
    }

    return mediaType ? [mediaType] : null;
  }, [mediaType, mediaTypes]);
  const filteredEntries = useMemo(
    () => activeMediaTypes ? entries.filter((entry) => activeMediaTypes.includes(entry.mediaType)) : entries,
    [activeMediaTypes, entries]
  );

  if (!filteredEntries.length) {
    return null;
  }

  return (
    <Stack spacing={1.5}>
      <Typography variant="h5">{title}</Typography>

      <Box
        sx={{
          overflowX: "auto",
          pb: 0.5,
          "&::-webkit-scrollbar": {
            height: 9
          },
          "&::-webkit-scrollbar-thumb": {
            backgroundColor: "rgba(255,255,255,0.12)",
            borderRadius: 999
          }
        }}
      >
        <Stack direction="row" spacing={1.5} sx={{ minWidth: "max-content" }}>
          {filteredEntries.map((entry) => (
            <ContinueWatchingCard
              key={entry.entryId}
              entry={entry}
              onDismiss={() => removeEntry(entry.entryId)}
              onResume={() => void launchPlayerSession(toPlayerSessionFromContinueWatching(entry))}
            />
          ))}
        </Stack>
      </Box>
    </Stack>
  );
}

type ContinueWatchingCardProps = {
  entry: ContinueWatchingEntry;
  onResume: () => void;
  onDismiss: () => void;
};

function ContinueWatchingCard({ entry, onResume, onDismiss }: ContinueWatchingCardProps) {
  const posterUrl = entry.posterPath ? convertFileSrc(entry.posterPath) : null;
  const progressPercent = entry.durationSec > 0 ? Math.min(100, (entry.progressSec / entry.durationSec) * 100) : 0;
  const supportingLabel = entry.mediaType === "show_episode"
    ? [entry.parentTitle, formatEpisodeMeta(entry)].filter(Boolean).join(" • ")
    : formatMediaTypeLabel(entry.mediaType);
  const title = entry.mediaType === "show_episode"
    ? entry.episodeTitle ?? entry.mediaTitle
    : entry.mediaTitle;
  const progressLabel = entry.durationSec > 0
    ? `${formatPlaybackTime(entry.progressSec)} / ${formatPlaybackTime(entry.durationSec)}`
    : `Parou em ${formatPlaybackTime(entry.progressSec)}`;

  return (
    <Box
      role="button"
      tabIndex={0}
      onClick={onResume}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onResume();
        }
      }}
      sx={{
        position: "relative",
        width: { xs: 268, md: 312 },
        minHeight: 196,
        flexShrink: 0,
        overflow: "hidden",
        borderRadius: 3,
        border: "1px solid rgba(255,255,255,0.08)",
        cursor: "pointer",
        background: "linear-gradient(180deg, rgba(237, 174, 73, 0.18) 0%, rgba(209, 73, 91, 0.36) 100%)",
        transition: "transform 160ms ease, border-color 160ms ease",
        ":hover": {
          transform: "translateY(-2px)",
          borderColor: "rgba(209, 73, 91, 0.45)"
        },
        ":focus-visible": {
          outline: "2px solid rgba(237, 174, 73, 0.88)",
          outlineOffset: 3
        }
      }}
    >
      {posterUrl ? (
        <Box
          component="img"
          src={posterUrl}
          alt=""
          aria-hidden="true"
          sx={{
            position: "absolute",
            inset: 0,
            width: "100%",
            height: "100%",
            objectFit: "cover",
            filter: "saturate(0.95) contrast(1.04)"
          }}
        />
      ) : null}

      <Box
        sx={{
          position: "absolute",
          inset: 0,
          background:
            "linear-gradient(180deg, rgba(9, 12, 16, 0.16) 0%, rgba(9, 12, 16, 0.42) 34%, rgba(9, 12, 16, 0.92) 100%)"
        }}
      />

      <IconButton
        aria-label="Remover de continue assistindo"
        onClick={(event) => {
          event.stopPropagation();
          onDismiss();
        }}
        sx={{
          position: "absolute",
          top: 10,
          right: 10,
          zIndex: 2,
          backgroundColor: "rgba(9, 12, 16, 0.62)",
          color: "common.white",
          ":hover": {
            backgroundColor: "rgba(9, 12, 16, 0.86)"
          }
        }}
      >
        <CloseRoundedIcon fontSize="small" />
      </IconButton>

      <Stack
        spacing={1.1}
        sx={{
          position: "relative",
          zIndex: 1,
          justifyContent: "flex-end",
          minHeight: "100%",
          px: 1.5,
          py: 1.5
        }}
      >
        <Typography variant="caption" sx={{ color: "rgba(255,255,255,0.72)", letterSpacing: 0.3 }}>
          {supportingLabel}
        </Typography>

        <Typography variant="h6" sx={{ color: "common.white", lineHeight: 1.08 }}>
          {title}
        </Typography>

        <Stack direction="row" spacing={0.75} alignItems="center">
          <PlayArrowRoundedIcon sx={{ color: "rgba(255,255,255,0.86)" }} />
          <Typography variant="body2" sx={{ color: "rgba(255,255,255,0.84)" }}>
            {progressLabel}
          </Typography>
        </Stack>

        <LinearProgress
          color="secondary"
          value={progressPercent}
          variant="determinate"
          sx={{
            height: 7,
            borderRadius: 999,
            backgroundColor: "rgba(255,255,255,0.14)"
          }}
        />
      </Stack>
    </Box>
  );
}

function formatEpisodeMeta(entry: ContinueWatchingEntry) {
  const seasonLabel = entry.seasonNumber !== null ? `T${String(entry.seasonNumber).padStart(2, "0")}` : null;
  const episodeLabel = entry.episodeNumber !== null ? `E${String(entry.episodeNumber).padStart(2, "0")}` : null;

  return [seasonLabel, episodeLabel].filter(Boolean).join(" ");
}

function formatPlaybackTime(value: number) {
  if (!Number.isFinite(value) || value <= 0) {
    return "00:00";
  }

  const totalSeconds = Math.floor(value);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return [hours, minutes, seconds].map((part) => String(part).padStart(2, "0")).join(":");
  }

  return [minutes, seconds].map((part) => String(part).padStart(2, "0")).join(":");
}

function formatMediaTypeLabel(mediaType: ContinueWatchingMediaType) {
  if (mediaType === "movie") {
    return "Filme";
  }

  if (mediaType === "standup") {
    return "Stand-up";
  }

  if (mediaType === "documentary") {
    return "Documentario";
  }

  return "Serie";
}