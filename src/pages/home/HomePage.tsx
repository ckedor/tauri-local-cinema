import { ContinueWatchingRail } from "@/features/library/components/ContinueWatchingRail";
import { MediaCatalogPage } from "@/features/library/components/MediaCatalogPage";
import { Stack } from "@mui/material";

export function HomePage() {
  return (
    <Stack spacing={3}>
      <ContinueWatchingRail mediaTypes={["movie", "show_episode", "standup", "documentary"]} />
      <MediaCatalogPage
        mediaTypes={["movie", "show", "concert", "standup", "documentary"]}
        emptyCategoryTitle="Biblioteca vazia"
        emptyCategoryDescription="Adicione uma pasta e faca um scan para popular o catalogo."
      />
    </Stack>
  );
}
