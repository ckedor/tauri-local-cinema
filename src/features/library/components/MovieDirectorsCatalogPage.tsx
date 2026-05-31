import { LibraryRootsField } from "@/components/ui/LibraryRootsField";
import { SectionCard } from "@/components/ui/SectionCard";
import { DirectorSquareCard } from "@/features/library/components/DirectorSquareCard";
import { runLibraryScanAndMarkNewItems } from "@/features/library/services/newItemsSync";
import { useCatalogFiltersStore } from "@/features/library/store/catalogFilters.store";
import {
  getInitialSetupStatus,
  listMovieDirectors,
  pickLibraryRoot,
  rescanLibrary,
  saveLibraryRoots
} from "@/services/tauri/commands/library";
import { DirectorSummaryDto, InitialSetupStatusDto } from "@/types/contracts/library";
import { appendLibraryRootPath, normalizeLibraryRootPaths } from "@/utils/libraryRoots";
import { Alert, Box, Button, CircularProgress, Stack, Typography } from "@mui/material";
import { useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";

const INITIAL_VISIBLE_ITEMS = 60;
const VISIBLE_ITEMS_STEP = 40;

export function MovieDirectorsCatalogPage() {
  const navigate = useNavigate();
  const [setupStatus, setSetupStatus] = useState<InitialSetupStatusDto | null>(null);
  const [directors, setDirectors] = useState<DirectorSummaryDto[]>([]);
  const [selectedPaths, setSelectedPaths] = useState<string[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isScanning, setIsScanning] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [visibleCount, setVisibleCount] = useState(INITIAL_VISIBLE_ITEMS);
  const loadMoreRef = useRef<HTMLDivElement | null>(null);
  const searchQuery = useCatalogFiltersStore((state) => state.searchQuery);
  const deferredSearchQuery = useDeferredValue(searchQuery);

  useEffect(() => {
    void refreshCatalog();
  }, []);

  useEffect(() => {
    setVisibleCount(INITIAL_VISIBLE_ITEMS);
  }, [deferredSearchQuery]);

  async function refreshCatalog() {
    setIsLoading(true);

    try {
      const [nextStatus, nextDirectors] = await Promise.all([
        getInitialSetupStatus(),
        listMovieDirectors().catch(() => [] as DirectorSummaryDto[])
      ]);

      setSetupStatus(nextStatus);
      setSelectedPaths(nextStatus.libraryRootPaths);
      setDirectors(nextStatus.hasLibraryRoot ? nextDirectors : []);
      setErrorMessage(null);
      setVisibleCount(INITIAL_VISIBLE_ITEMS);
    } catch (error) {
      setErrorMessage(asMessage(error, "Nao foi possivel carregar os diretores."));
    } finally {
      setIsLoading(false);
    }
  }

  async function handlePickFolder() {
    try {
      const path = await pickLibraryRoot();

      if (path) {
        setSelectedPaths((currentPaths) => appendLibraryRootPath(currentPaths, path));
        setErrorMessage(null);
      }
    } catch (error) {
      setErrorMessage(asMessage(error, "Nao foi possivel abrir o seletor de pasta."));
    }
  }

  function handleRemoveFolder(pathIndex: number) {
    setSelectedPaths((currentPaths) => currentPaths.filter((_, index) => index !== pathIndex));
  }

  async function handleSaveAndScan() {
    const nextPaths = normalizeLibraryRootPaths(selectedPaths);
    if (nextPaths.length === 0) {
      setErrorMessage("Escolha ao menos uma pasta antes de reescanear a biblioteca.");
      return;
    }

    setIsScanning(true);

    try {
      await saveLibraryRoots(nextPaths);
      await runLibraryScanAndMarkNewItems(() => rescanLibrary());
      await refreshCatalog();
    } catch (error) {
      setErrorMessage(asMessage(error, "Nao foi possivel reescanear a biblioteca."));
    } finally {
      setIsScanning(false);
    }
  }

  const isFirstRun = !setupStatus?.hasLibraryRoot;

  const filteredDirectors = useMemo(() => {
    const normalizedQuery = deferredSearchQuery.trim().toLocaleLowerCase();

    return [...directors]
      .filter((director) => {
        if (!normalizedQuery) {
          return true;
        }

        return director.name.toLocaleLowerCase().includes(normalizedQuery);
      })
      .sort((left, right) => left.name.localeCompare(right.name, "pt-BR"));
  }, [deferredSearchQuery, directors]);

  useEffect(() => {
    if (visibleCount >= filteredDirectors.length) {
      return;
    }

    const target = loadMoreRef.current;

    if (!target) {
      return;
    }

    const observer = new IntersectionObserver(
      (entries) => {
        if (!entries.some((entry) => entry.isIntersecting)) {
          return;
        }

        setVisibleCount((count) => Math.min(count + VISIBLE_ITEMS_STEP, filteredDirectors.length));
      },
      {
        rootMargin: "0px 0px 420px 0px"
      }
    );

    observer.observe(target);

    return () => {
      observer.disconnect();
    };
  }, [filteredDirectors.length, visibleCount]);

  if (isLoading) {
    return (
      <Stack alignItems="center" justifyContent="center" sx={{ minHeight: "60vh" }}>
        <CircularProgress color="secondary" />
      </Stack>
    );
  }

  return (
    <Stack spacing={3}>
      {errorMessage ? <Alert severity="error">{errorMessage}</Alert> : null}

      {isFirstRun ? (
        <SectionCard
          sx={{
            background:
              "radial-gradient(circle at top left, rgba(209, 73, 91, 0.22), rgba(17, 20, 24, 0.96) 54%), rgba(27, 31, 36, 0.88)"
          }}
        >
          <Stack spacing={3}>
            <Typography variant="h5">Configure sua biblioteca</Typography>

            <LibraryRootsField
              disabled={isScanning}
              onAddPath={handlePickFolder}
              onRemovePath={handleRemoveFolder}
              paths={selectedPaths}
            />

            <Stack direction={{ xs: "column", sm: "row" }} spacing={2}>
              <Button
                disabled={selectedPaths.length === 0 || isScanning}
                onClick={handleSaveAndScan}
                size="large"
                variant="contained"
              >
                {isScanning ? "Reescaneando biblioteca..." : "Salvar pastas e reescanear biblioteca"}
              </Button>
            </Stack>
          </Stack>
        </SectionCard>
      ) : null}

      {!isFirstRun && directors.length === 0 ? (
        <SectionCard>
          <Stack spacing={1.5} sx={{ py: 4 }}>
            <Typography variant="h5">Nenhum diretor encontrado</Typography>
            <Typography color="text.secondary">
              Reescaneie a biblioteca com a chave do TMDb para montar os cards de diretores.
            </Typography>
          </Stack>
        </SectionCard>
      ) : null}

      {!isFirstRun && directors.length > 0 && filteredDirectors.length === 0 ? (
        <SectionCard>
          <Stack spacing={1.5} sx={{ py: 4 }}>
            <Typography variant="h5">Nenhum resultado</Typography>
            <Typography color="text.secondary">Ajuste a busca para encontrar outros diretores.</Typography>
          </Stack>
        </SectionCard>
      ) : null}

      {filteredDirectors.length > 0 ? (
        <>
          <Box
            sx={{
              display: "grid",
              gap: { xs: 1.25, md: 1.5 },
              gridTemplateColumns: "repeat(auto-fill, minmax(min(100%, 170px), 1fr))"
            }}
          >
            {filteredDirectors.slice(0, visibleCount).map((director) => (
              <DirectorSquareCard
                key={director.id}
                director={director}
                onClick={() =>
                  navigate(`/movies/directors/${encodeURIComponent(director.id)}`, {
                    state: { director, backToPath: "/movies?tab=directors" }
                  })
                }
              />
            ))}
          </Box>

          {visibleCount < filteredDirectors.length ? (
            <Stack alignItems="center" sx={{ py: 1.5 }}>
              <Box ref={loadMoreRef} sx={{ width: 1, display: "flex", justifyContent: "center", py: 1.5 }}>
                <CircularProgress color="secondary" size={24} />
              </Box>
            </Stack>
          ) : null}
        </>
      ) : null}
    </Stack>
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