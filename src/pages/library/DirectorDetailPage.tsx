import { MediaPosterCard } from "@/features/library/components/MediaPosterCard";
import { getMovieDirectorDetail } from "@/services/tauri/commands/library";
import { DirectorSummaryDto, MovieDirectorDetailDto } from "@/types/contracts/library";
import { Alert, Box, Button, CircularProgress, Stack, Typography } from "@mui/material";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import { useLocation, useNavigate, useParams } from "react-router-dom";

type DirectorDetailLocationState = {
  director?: DirectorSummaryDto;
  backToPath?: string;
};

export function DirectorDetailPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const { directorId } = useParams();
  const navigationState = location.state as DirectorDetailLocationState | null;
  const decodedDirectorId = directorId ? decodeURIComponent(directorId) : null;
  const [detail, setDetail] = useState<MovieDirectorDetailDto | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [hasImageError, setHasImageError] = useState(false);

  useEffect(() => {
    if (!decodedDirectorId) {
      setDetail(null);
      setErrorMessage("Diretor nao encontrado.");
      setIsLoading(false);
      return;
    }

    let isMounted = true;
    setIsLoading(true);

    void getMovieDirectorDetail(decodedDirectorId)
      .then((nextDetail) => {
        if (!isMounted) {
          return;
        }

        setDetail(nextDetail);
        setErrorMessage(nextDetail ? null : "Diretor nao encontrado.");
      })
      .catch((error) => {
        if (isMounted) {
          setDetail(null);
          setErrorMessage(asMessage(error, "Nao foi possivel carregar os filmes do diretor."));
        }
      })
      .finally(() => {
        if (isMounted) {
          setIsLoading(false);
        }
      });

    return () => {
      isMounted = false;
    };
  }, [decodedDirectorId]);

  useEffect(() => {
    setHasImageError(false);
  }, [detail?.imagePath]);

  const backToPath = navigationState?.backToPath ?? "/movies?tab=directors";
  const imageUrl = !hasImageError ? resolveDirectorImageUrl(detail?.imagePath ?? null) : null;
  const movieCountLabel = detail
    ? detail.movies.length === 1
      ? "1 filme"
      : `${detail.movies.length} filmes`
    : navigationState?.director
      ? navigationState.director.movieCount === 1
        ? "1 filme"
        : `${navigationState.director.movieCount} filmes`
      : null;
  const pageTitle = detail?.name ?? navigationState?.director?.name ?? "Diretor";
  const orderedMovies = useMemo(
    () => [...(detail?.movies ?? [])].sort((left, right) => (right.year ?? 0) - (left.year ?? 0) || left.title.localeCompare(right.title, "pt-BR")),
    [detail?.movies]
  );

  return (
    <Box
      sx={{
        position: "relative",
        minHeight: "calc(100vh - 88px)",
        mx: { xs: -3, md: -3 },
        my: -3,
        overflow: "hidden",
        background:
          imageUrl
            ? "rgba(11, 14, 18, 0.96)"
            : "linear-gradient(120deg, rgba(184, 85, 99, 0.18), rgba(17, 20, 24, 0.96) 52%)"
      }}
    >
      <Stack spacing={3} sx={{ minHeight: "calc(100vh - 88px)", p: { xs: 2, md: 4 } }}>
        <Button sx={{ width: "fit-content", color: "text.secondary" }} variant="text" onClick={() => navigate(backToPath)}>
          ← Voltar
        </Button>

        {errorMessage ? <Alert severity="warning">{errorMessage}</Alert> : null}

        <Stack direction={{ xs: "column", md: "row" }} spacing={{ xs: 3, md: 4 }} alignItems={{ md: "flex-start" }}>
          <Stack sx={{ width: { xs: "100%", md: "32%" }, maxWidth: { md: 420 }, flexShrink: 0 }}>
            <Box
              sx={{
                position: "relative",
                width: "100%",
                aspectRatio: "1 / 1",
                overflow: "hidden",
                borderRadius: 4,
                border: "1px solid rgba(255,255,255,0.08)",
                background:
                  imageUrl
                    ? "rgba(17,20,24,0.96)"
                    : "linear-gradient(180deg, rgba(237, 174, 73, 0.16) 0%, rgba(209, 73, 91, 0.3) 100%)",
                boxShadow: "0 24px 64px rgba(0,0,0,0.28)"
              }}
            >
              {imageUrl ? (
                <Box component="img" src={imageUrl} alt={pageTitle} onError={() => setHasImageError(true)} sx={{ width: "100%", height: "100%", objectFit: "cover" }} />
              ) : null}
            </Box>
          </Stack>

          <Stack spacing={3} sx={{ flex: 1, minWidth: 0 }}>
            <Stack spacing={1}>
              <Typography variant="overline" sx={{ color: "text.secondary", letterSpacing: 1.6 }}>
                Diretor
              </Typography>
              <Typography
                variant="h3"
                sx={{
                  fontSize: { xs: "2rem", md: "2.6rem" },
                  lineHeight: 1,
                  maxWidth: 760
                }}
              >
                {pageTitle}
              </Typography>
              {movieCountLabel ? <Typography color="text.secondary">{movieCountLabel}</Typography> : null}
            </Stack>

            {isLoading ? (
              <Stack direction="row" alignItems="center" spacing={1.5}>
                <CircularProgress color="secondary" size={20} />
                <Typography color="text.secondary">Carregando filmes...</Typography>
              </Stack>
            ) : null}

            {!isLoading && !errorMessage && orderedMovies.length === 0 ? (
              <Typography color="text.secondary">Nenhum filme encontrado para este diretor.</Typography>
            ) : null}

            {orderedMovies.length > 0 ? (
              <Stack
                direction="row"
                flexWrap="wrap"
                gap={2}
                sx={{
                  alignItems: "stretch",
                  "> *": {
                    width: {
                      xs: "calc(50% - 8px)",
                      sm: "calc(33.333% - 11px)",
                      md: "calc(25% - 12px)",
                      lg: "calc(20% - 13px)",
                      xl: "calc(16.666% - 14px)"
                    }
                  }
                }}
              >
                {orderedMovies.map((movie) => (
                  <Stack key={movie.id} sx={{ minWidth: 0 }}>
                    <MediaPosterCard
                      item={movie}
                      onClick={() =>
                        navigate(`/library/${encodeURIComponent(movie.id)}`, {
                          state: { item: movie, backToPath: `${location.pathname}${location.search}` }
                        })
                      }
                    />
                  </Stack>
                ))}
              </Stack>
            ) : null}
          </Stack>
        </Stack>
      </Stack>
    </Box>
  );
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