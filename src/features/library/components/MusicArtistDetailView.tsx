import { MusicSquareCard } from "@/features/library/components/MusicSquareCard";
import { HomeMediaItemDto } from "@/types/contracts/library";
import { Alert, Box, Button, CircularProgress, Stack, Typography } from "@mui/material";

type MusicArtistDetailViewProps = {
  item: HomeMediaItemDto;
  posterUrl: string | null;
  albums: HomeMediaItemDto[];
  isLoading: boolean;
  errorMessage: string | null;
  backToPath: string;
  onBack: () => void;
  onOpenAlbum: (album: HomeMediaItemDto) => void;
};

export function MusicArtistDetailView({
  item,
  posterUrl,
  albums,
  isLoading,
  errorMessage,
  onBack,
  onOpenAlbum
}: MusicArtistDetailViewProps) {
  return (
    <Box
      sx={{
        position: "relative",
        minHeight: "calc(100vh - 88px)",
        mx: { xs: -3, md: -3 },
        my: -3,
        overflow: "hidden",
        background:
          posterUrl
            ? "rgba(11, 14, 18, 0.96)"
            : "linear-gradient(120deg, rgba(184, 85, 99, 0.18), rgba(17, 20, 24, 0.96) 52%)"
      }}
    >
      <Stack spacing={3} sx={{ minHeight: "calc(100vh - 88px)", p: { xs: 2, md: 4 } }}>
        <Button sx={{ width: "fit-content", color: "text.secondary" }} variant="text" onClick={onBack}>
          ← Voltar
        </Button>

        {errorMessage ? <Alert severity="warning">{errorMessage}</Alert> : null}

        <Stack direction={{ xs: "column", md: "row" }} spacing={{ xs: 3, md: 4 }} alignItems={{ md: "flex-start" }}>
          <Stack sx={{ width: { xs: "100%", md: "33%" }, maxWidth: { md: 420 }, flexShrink: 0 }}>
            <Box
              sx={{
                position: "relative",
                width: "100%",
                aspectRatio: "1 / 1",
                overflow: "hidden",
                borderRadius: 4,
                border: "1px solid rgba(255,255,255,0.08)",
                background:
                  posterUrl
                    ? "rgba(17,20,24,0.96)"
                    : "linear-gradient(180deg, rgba(237, 174, 73, 0.16) 0%, rgba(209, 73, 91, 0.3) 100%)",
                boxShadow: "0 24px 64px rgba(0,0,0,0.28)"
              }}
            >
              {posterUrl ? (
                <Box component="img" src={posterUrl} alt={item.title} sx={{ width: "100%", height: "100%", objectFit: "cover" }} />
              ) : null}
            </Box>
          </Stack>

          <Stack spacing={3} sx={{ flex: 1, minWidth: 0 }}>
            <Typography
              variant="h3"
              sx={{
                fontSize: { xs: "2rem", md: "2.6rem" },
                lineHeight: 1,
                maxWidth: 760
              }}
            >
              {item.title}
            </Typography>

            <Typography variant="overline" sx={{ color: "text.secondary", letterSpacing: 1.6 }}>
              Albuns
            </Typography>

            {isLoading ? (
              <Stack direction="row" alignItems="center" spacing={1.5}>
                <CircularProgress color="secondary" size={20} />
                <Typography color="text.secondary">Carregando albuns...</Typography>
              </Stack>
            ) : null}

            {!isLoading && !errorMessage && !albums.length ? (
              <Typography color="text.secondary">Nenhum album encontrado para este artista.</Typography>
            ) : null}

            {albums.length ? (
              <Box
                sx={{
                  display: "grid",
                  gap: 2,
                  gridTemplateColumns: {
                    xs: "repeat(2, minmax(0, 1fr))",
                    sm: "repeat(3, minmax(0, 1fr))",
                    xl: "repeat(4, minmax(0, 1fr))"
                  }
                }}
              >
                {albums.map((album) => (
                  <MusicSquareCard key={album.id} item={album} onClick={() => onOpenAlbum(album)} />
                ))}
              </Box>
            ) : null}
          </Stack>
        </Stack>
      </Stack>
    </Box>
  );
}