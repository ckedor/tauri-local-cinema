import { SectionCard } from "@/components/ui/SectionCard";
import { HomeMediaItemDto } from "@/types/contracts/library";
import { Box, Stack, Typography } from "@mui/material";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useState } from "react";

type MusicSquareCardProps = {
  item: HomeMediaItemDto;
  onClick?: () => void;
};

export function MusicSquareCard({ item, onClick }: MusicSquareCardProps) {
  const [hasPosterError, setHasPosterError] = useState(false);
  const posterUrl = item.posterPath && !hasPosterError ? convertFileSrc(item.posterPath) : null;
  const metaLabel = getMusicCardMeta(item);
  const isInteractive = Boolean(onClick);

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
        overflow: "hidden",
        cursor: isInteractive ? "pointer" : "default",
        transition: "transform 180ms ease, border-color 180ms ease",
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
          aspectRatio: "1 / 1",
          background: posterUrl
            ? "#111418"
            : "linear-gradient(180deg, rgba(237, 174, 73, 0.16) 0%, rgba(209, 73, 91, 0.3) 100%)",
          display: "flex",
          alignItems: "flex-end",
          p: 1.15
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

        <Box
          sx={{
            position: "absolute",
            inset: 0,
            background:
              "linear-gradient(180deg, rgba(10, 12, 16, 0.05) 0%, rgba(10, 12, 16, 0.18) 40%, rgba(10, 12, 16, 0.9) 100%)"
          }}
        />

        <Stack spacing={0.2} sx={{ position: "relative", zIndex: 1, width: "100%" }}>
          {metaLabel ? (
            <Typography variant="caption" sx={{ color: "rgba(255,255,255,0.72)" }}>
              {metaLabel}
            </Typography>
          ) : null}
          <Typography
            variant="body2"
            sx={{
              color: "common.white",
              fontWeight: 700,
              lineHeight: 1.08,
              textShadow: "0 2px 12px rgba(0, 0, 0, 0.5)",
              display: "-webkit-box",
              overflow: "hidden",
              WebkitLineClamp: 2,
              WebkitBoxOrient: "vertical"
            }}
          >
            {item.title}
          </Typography>
        </Stack>
      </Box>
    </SectionCard>
  );
}

function getMusicCardMeta(item: HomeMediaItemDto) {
  if (item.mediaType === "music_artist") {
    return null;
  }

  if (item.mediaType === "music_album") {
    return item.year ? String(item.year) : null;
  }

  return null;
}