import { SectionCard } from "@/components/ui/SectionCard";
import { DirectorSummaryDto } from "@/types/contracts/library";
import { Box, Stack, Typography } from "@mui/material";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

type DirectorSquareCardProps = {
  director: DirectorSummaryDto;
  onClick?: () => void;
};

export function DirectorSquareCard({ director, onClick }: DirectorSquareCardProps) {
  const [hasImageError, setHasImageError] = useState(false);

  useEffect(() => {
    setHasImageError(false);
  }, [director.imagePath]);

  const imageUrl = !hasImageError ? resolveDirectorImageUrl(director.imagePath) : null;
  const isInteractive = Boolean(onClick);
  const movieCountLabel = director.movieCount === 1 ? "1 filme" : `${director.movieCount} filmes`;

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
          background: imageUrl
            ? "#111418"
            : "linear-gradient(180deg, rgba(237, 174, 73, 0.14) 0%, rgba(209, 73, 91, 0.24) 100%)",
          display: "flex",
          alignItems: "flex-end",
          p: 1.15
        }}
      >
        {imageUrl ? (
          <Box
            component="img"
            src={imageUrl}
            alt={director.name}
            onError={() => setHasImageError(true)}
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
              "linear-gradient(180deg, rgba(10, 12, 16, 0.08) 0%, rgba(10, 12, 16, 0.22) 38%, rgba(10, 12, 16, 0.92) 100%)"
          }}
        />

        <Stack spacing={0.2} sx={{ position: "relative", zIndex: 1, width: "100%" }}>
          <Typography variant="caption" sx={{ color: "rgba(255,255,255,0.72)" }}>
            {movieCountLabel}
          </Typography>
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
            {director.name}
          </Typography>
        </Stack>
      </Box>
    </SectionCard>
  );
}

function resolveDirectorImageUrl(imagePath: string | null) {
  const normalizedPath = imagePath?.trim();

  if (!normalizedPath) {
    return null;
  }

  if (/^https?:\/\//i.test(normalizedPath)) {
    return normalizedPath;
  }

  return convertFileSrc(normalizedPath);
}