import { SectionCard } from "@/components/ui/SectionCard";
import { formatMediaType } from "@/features/library/utils/media";
import { HomeMediaItemDto } from "@/types/contracts/library";
import { Box, Chip, Stack, Typography } from "@mui/material";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useState } from "react";

type MediaPosterCardProps = {
  item: HomeMediaItemDto;
  onClick?: () => void;
};

export function MediaPosterCard({ item, onClick }: MediaPosterCardProps) {
  const [hasPosterError, setHasPosterError] = useState(false);
  const posterUrl = item.posterPath && !hasPosterError ? convertFileSrc(item.posterPath) : null;
  const isInteractive = Boolean(onClick);
  const metaLabel = getCardMetaLabel(item);

  return (
    <SectionCard
      onClick={onClick}
      onKeyDown={(event) => {
        if (!onClick) {
          return;
        }

        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onClick();
        }
      }}
      role={isInteractive ? "button" : undefined}
      tabIndex={isInteractive ? 0 : undefined}
      sx={{
        p: 0,
        height: "100%",
        overflow: "hidden",
        transition: "transform 180ms ease, border-color 180ms ease",
        cursor: isInteractive ? "pointer" : "default",
        ":hover": {
          transform: "translateY(-3px)",
          borderColor: "rgba(209, 73, 91, 0.5)"
        },
        ":focus-visible": {
          outline: "2px solid rgba(237, 174, 73, 0.9)",
          outlineOffset: 3
        }
      }}
    >
      <Box
        sx={{
          position: "relative",
          aspectRatio: "4 / 5",
          minHeight: 180,
          background: posterUrl
            ? "#111418"
            : "linear-gradient(180deg, rgba(237, 174, 73, 0.16) 0%, rgba(209, 73, 91, 0.3) 100%)",
          display: "flex",
          alignItems: "flex-end",
          p: 1.1
        }}
      >
        {posterUrl ? (
          <Box
            component="img"
            src={posterUrl}
            alt={item.title}
            onError={() => setHasPosterError(true)}
            sx={{
              position: "absolute",
              inset: 0,
              width: "100%",
              height: "100%",
              objectFit: "cover"
            }}
          />
        ) : null}

        {posterUrl ? (
          <Box
            sx={{
              position: "absolute",
              inset: 0,
              background:
                "linear-gradient(180deg, rgba(10, 12, 16, 0.12) 0%, rgba(10, 12, 16, 0.28) 38%, rgba(10, 12, 16, 0.92) 100%)"
            }}
          />
        ) : null}

        <Stack
          spacing={0}
          sx={{
            position: "relative",
            zIndex: 1,
            width: "100%"
          }}
        >
          <Typography
            variant="body2"
            sx={{
              fontWeight: 700,
              color: "common.white",
              textShadow: "0 2px 12px rgba(0, 0, 0, 0.5)",
              display: "-webkit-box",
              overflow: "hidden",
              WebkitLineClamp: 2,
              WebkitBoxOrient: "vertical",
              lineHeight: 1.08,
              minHeight: 0,
            }}
          >
            {item.title}
          </Typography>

          <Stack direction="row" justifyContent="space-between" alignItems="flex-end" spacing={1}>
            <Typography
              variant="caption"
              sx={{
                color: "rgba(255, 255, 255, 0.82)",
                textShadow: "0 1px 6px rgba(0, 0, 0, 0.45)",
                lineHeight: 1,
                pt: 0,
                mt: 0
              }}
            >
              {metaLabel}
            </Typography>

            <Chip
              label={formatMediaType(item.mediaType)}
              size="small"
              sx={{
                backgroundColor: "rgba(209, 73, 91, 0.22)",
                backdropFilter: "blur(8px)",
                height: 22,
              }}
            />
          </Stack>
        </Stack>
      </Box>
    </SectionCard>
  );
}

function getCardMetaLabel(item: HomeMediaItemDto) {
  if (item.mediaType === "music_artist") {
    return "Artista";
  }

  if (item.mediaType === "music_album") {
    return item.year ?? "Album";
  }

  if (item.mediaType === "concert") {
    return item.year ?? "Concerto";
  }

  return item.year ?? "Ano desconhecido";
}