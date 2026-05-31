import { SectionCard } from "@/components/ui/SectionCard";
import { ContinueWatchingRail } from "@/features/library/components/ContinueWatchingRail";
import { MediaCatalogPage } from "@/features/library/components/MediaCatalogPage";
import { MovieDirectorsCatalogPage } from "@/features/library/components/MovieDirectorsCatalogPage";
import { Stack, Tab, Tabs } from "@mui/material";
import { SyntheticEvent, useMemo } from "react";
import { useSearchParams } from "react-router-dom";

type MovieTabValue = "titles" | "directors";

export function MovieCatalogTabsPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const activeTab = useMemo<MovieTabValue>(
    () => (searchParams.get("tab") === "directors" ? "directors" : "titles"),
    [searchParams]
  );

  function handleTabChange(_: SyntheticEvent, value: MovieTabValue) {
    const nextParams = new URLSearchParams(searchParams);

    if (value === "directors") {
      nextParams.set("tab", value);
    } else {
      nextParams.delete("tab");
    }

    setSearchParams(nextParams, { replace: true });
  }

  return (
    <Stack spacing={3}>
      <SectionCard sx={{ p: 0.75 }}>
        <Tabs
          value={activeTab}
          onChange={handleTabChange}
          variant="standard"
          centered
          sx={{
            minHeight: 34,
            ".MuiTabs-flexContainer": {
              justifyContent: "center",
              gap: 2.5
            },
            ".MuiTabs-indicator": {
              height: 2,
              borderRadius: 999,
              backgroundColor: "rgba(209, 73, 91, 0.95)"
            },
            ".MuiTab-root": {
              minHeight: 34,
              minWidth: 0,
              py: 0.4,
              px: 0.25,
              fontSize: "0.85rem",
              fontWeight: 600,
              letterSpacing: 0.2,
              textTransform: "none",
              color: "text.secondary"
            },
            ".MuiTab-root.Mui-selected": {
              color: "common.white"
            }
          }}
        >
          <Tab disableRipple label="Filmes" value="titles" />
          <Tab disableRipple label="Diretores" value="directors" />
        </Tabs>
      </SectionCard>

      {activeTab === "titles" ? (
        <Stack spacing={3}>
          <ContinueWatchingRail mediaType="movie" />
          <MediaCatalogPage
            mediaTypes={["movie"]}
            emptyCategoryTitle="Nenhum filme encontrado"
            emptyCategoryDescription="Adicione filmes na pasta movies e reescaneie a biblioteca."
          />
        </Stack>
      ) : (
        <MovieDirectorsCatalogPage />
      )}
    </Stack>
  );
}