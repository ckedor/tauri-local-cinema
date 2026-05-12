import { ContinueWatchingRail } from "@/features/library/components/ContinueWatchingRail";
import { MediaCatalogPage } from "@/features/library/components/MediaCatalogPage";
import { MusicCatalogTabsPage } from "@/features/library/components/MusicCatalogTabsPage";
import { Stack } from "@mui/material";

export function MoviesPage() {
  return (
    <Stack spacing={3}>
      <ContinueWatchingRail mediaType="movie" />
      <MediaCatalogPage
        mediaTypes={["movie"]}
        emptyCategoryTitle="Nenhum filme encontrado"
        emptyCategoryDescription="Adicione filmes na pasta movies e reescaneie a biblioteca."
      />
    </Stack>
  );
}

export function TVShowsPage() {
  return (
    <Stack spacing={3}>
      <ContinueWatchingRail mediaType="show_episode" />
      <MediaCatalogPage
        mediaTypes={["show"]}
        emptyCategoryTitle="Nenhuma serie encontrada"
        emptyCategoryDescription="Adicione conteudo na pasta shows e reescaneie a biblioteca."
      />
    </Stack>
  );
}

export function ConcertsPage() {
  return (
    <MediaCatalogPage
      mediaTypes={["concert"]}
      emptyCategoryTitle="Nenhum concerto encontrado"
      emptyCategoryDescription="Adicione videos na pasta concerts e reescaneie a biblioteca."
    />
  );
}

export function MusicPage() {
  return <MusicCatalogTabsPage />;
}

export function StandupPage() {
  return (
    <MediaCatalogPage
      mediaTypes={["standup"]}
      emptyCategoryTitle="Nenhum standup encontrado"
      emptyCategoryDescription="Adicione conteudo na pasta standup e reescaneie a biblioteca."
    />
  );
}

export function DocumentaryPage() {
  return (
    <MediaCatalogPage
      mediaTypes={["documentary"]}
      emptyCategoryTitle="Nenhum documentario encontrado"
      emptyCategoryDescription="Adicione conteudo na pasta documentary e reescaneie a biblioteca."
    />
  );
}