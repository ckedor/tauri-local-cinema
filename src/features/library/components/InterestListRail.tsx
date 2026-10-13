import { MediaPosterCard } from "@/features/library/components/MediaPosterCard";
import { useInterestListStore } from "@/features/library/store/interestList.store";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import { Box, Button, Stack, Typography } from "@mui/material";
import { useNavigate } from "react-router-dom";

const MAX_PREVIEW_ITEMS = 10;

export function InterestListRail() {
  const navigate = useNavigate();
  const entries = useInterestListStore((state) => state.entries);
  const removeEntry = useInterestListStore((state) => state.removeEntry);

  if (!entries.length) {
    return null;
  }

  return (
    <Stack spacing={1.5}>
      <Stack
        direction={{ xs: "column", sm: "row" }}
        spacing={1.5}
        alignItems={{ xs: "flex-start", sm: "center" }}
        justifyContent="space-between"
      >
        <Stack spacing={0.25}>
          <Typography variant="h5">Lista de interesse</Typography>
          <Typography color="text.secondary">
            {entries.length} {entries.length === 1 ? "titulo marcado" : "titulos marcados"} para ver depois.
          </Typography>
        </Stack>

        <Button variant="outlined" onClick={() => navigate("/interests")}>
          Abrir lista
        </Button>
      </Stack>

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
          {entries.slice(0, MAX_PREVIEW_ITEMS).map((entry) => (
            <Box key={entry.id} sx={{ width: { xs: 156, sm: 172, md: 184 }, flexShrink: 0 }}>
              <MediaPosterCard
                item={entry}
                onClick={() =>
                  navigate(`/library/${encodeURIComponent(entry.id)}`, {
                    state: { backToPath: "/" }
                  })
                }
                topAction={{
                  label: "Remover da lista de interesse",
                  icon: <CloseRoundedIcon fontSize="small" />,
                  onClick: () => removeEntry(entry.id),
                  tone: "danger"
                }}
              />
            </Box>
          ))}
        </Stack>
      </Box>
    </Stack>
  );
}