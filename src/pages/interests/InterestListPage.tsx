import { SectionCard } from "@/components/ui/SectionCard";
import { MediaPosterCard } from "@/features/library/components/MediaPosterCard";
import { useInterestListStore } from "@/features/library/store/interestList.store";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import { Box, Button, Stack, Typography } from "@mui/material";
import { useNavigate } from "react-router-dom";

export function InterestListPage() {
  const navigate = useNavigate();
  const entries = useInterestListStore((state) => state.entries);
  const removeEntry = useInterestListStore((state) => state.removeEntry);

  return (
    <Stack spacing={3}>
      <Stack
        direction={{ xs: "column", md: "row" }}
        spacing={2}
        alignItems={{ xs: "flex-start", md: "center" }}
        justifyContent="space-between"
      >
        <Stack spacing={0.5}>
          <Typography variant="h4">Novos</Typography>
          <Typography color="text.secondary">
            {entries.length
              ? `${entries.length} ${entries.length === 1 ? "titulo marcado" : "titulos marcados"} como novo.`
              : "Novos scans entram aqui automaticamente, e voce tambem pode marcar um titulo manualmente."}
          </Typography>
        </Stack>

        <Button variant="outlined" onClick={() => navigate("/")}>
          Voltar para a home
        </Button>
      </Stack>

      {!entries.length ? (
        <SectionCard>
          <Stack spacing={1.5} sx={{ py: 4 }}>
            <Typography variant="h5">Nenhum titulo em Novos</Typography>
            <Typography color="text.secondary">
              Quando um scan encontrar titulos novos, eles aparecem aqui. Voce tambem pode marcar um titulo manualmente.
            </Typography>
          </Stack>
        </SectionCard>
      ) : (
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
                lg: "calc(16.666% - 14px)",
                xl: "calc(16.666% - 14px)"
              }
            }
          }}
        >
          {entries.map((entry) => (
            <Stack key={entry.id} sx={{ minWidth: 0 }}>
              <MediaPosterCard
                item={entry}
                onClick={() =>
                  navigate(`/library/${encodeURIComponent(entry.id)}`, {
                    state: { backToPath: "/novos" }
                  })
                }
                topAction={{
                  label: "Remover de Novos",
                  icon: <CloseRoundedIcon fontSize="small" />,
                  onClick: () => removeEntry(entry.id),
                  tone: "danger"
                }}
              />
            </Stack>
          ))}
        </Stack>
      )}
    </Stack>
  );
}