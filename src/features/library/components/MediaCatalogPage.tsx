import { LibraryRootsField } from "@/components/ui/LibraryRootsField";
import { MediaPosterCard } from "@/features/library/components/MediaPosterCard";
import { useCatalogFiltersStore } from "@/features/library/store/catalogFilters.store";
import { formatMediaType } from "@/features/library/utils/media";
import {
    getInitialSetupStatus,
    listHomeMedia,
    pickLibraryRoot,
  rescanLibrary,
  saveLibraryRoots
} from "@/services/tauri/commands/library";
import { HomeMediaItemDto, InitialSetupStatusDto } from "@/types/contracts/library";
import { appendLibraryRootPath, normalizeLibraryRootPaths } from "@/utils/libraryRoots";
import {
    Alert,
    Box,
    Button,
    CircularProgress,
    Stack,
    Typography
} from "@mui/material";
import { useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";

import { SectionCard } from "@/components/ui/SectionCard";

const INITIAL_VISIBLE_ITEMS = 60;
const VISIBLE_ITEMS_STEP = 40;

type MediaCatalogPageProps = {
  mediaTypes?: string[];
  emptyCategoryTitle?: string;
  emptyCategoryDescription?: string;
};

export function MediaCatalogPage({
  mediaTypes,
  emptyCategoryTitle = "Nenhuma midia encontrada",
  emptyCategoryDescription = "Nenhum titulo disponivel neste recorte da biblioteca."
}: MediaCatalogPageProps) {
  const navigate = useNavigate();
  const location = useLocation();
  const [setupStatus, setSetupStatus] = useState<InitialSetupStatusDto | null>(null);
  const [mediaItems, setMediaItems] = useState<HomeMediaItemDto[]>([]);
  const [selectedPaths, setSelectedPaths] = useState<string[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isScanning, setIsScanning] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [visibleCount, setVisibleCount] = useState(INITIAL_VISIBLE_ITEMS);
  const loadMoreRef = useRef<HTMLDivElement | null>(null);
  const searchQuery = useCatalogFiltersStore((state) => state.searchQuery);
  const selectedDecade = useCatalogFiltersStore((state) => state.selectedDecade);
  const deferredSearchQuery = useDeferredValue(searchQuery);

  useEffect(() => {
    void refreshCatalog();
  }, []);

  useEffect(() => {
    setVisibleCount(INITIAL_VISIBLE_ITEMS);
  }, [deferredSearchQuery, selectedDecade, mediaTypes?.join("|")]);

  async function refreshCatalog() {
    setIsLoading(true);

    try {
      const [nextStatus, nextItems] = await Promise.all([
        getInitialSetupStatus(),
        listHomeMedia().catch(() => [] as HomeMediaItemDto[])
      ]);

      setSetupStatus(nextStatus);
  setSelectedPaths(nextStatus.libraryRootPaths);
      setErrorMessage(null);
      setMediaItems(nextStatus.hasLibraryRoot ? nextItems : []);
      setVisibleCount(INITIAL_VISIBLE_ITEMS);
    } catch (error) {
      setErrorMessage(asMessage(error, "Nao foi possivel carregar a biblioteca."));
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
      setErrorMessage("Escolha ao menos uma pasta antes de reescanear tudo.");
      return;
    }

    setIsScanning(true);

    try {
      await saveLibraryRoots(nextPaths);
      await rescanLibrary();
      await refreshCatalog();
    } catch (error) {
      setErrorMessage(asMessage(error, "Nao foi possivel reescanear a biblioteca."));
    } finally {
      setIsScanning(false);
    }
  }

  const isFirstRun = !setupStatus?.hasLibraryRoot;

  const categoryItems = useMemo(() => {
    if (!mediaTypes?.length) {
      return mediaItems;
    }

    return mediaItems.filter((item) => mediaTypes.includes(item.mediaType));
  }, [mediaItems, mediaTypes]);

  const filteredMediaItems = useMemo(() => {
    const normalizedQuery = deferredSearchQuery.trim().toLocaleLowerCase();

    return [...categoryItems]
      .filter((item) => {
        if (selectedDecade !== null) {
          if (item.year === null || item.year < selectedDecade || item.year >= selectedDecade + 10) {
            return false;
          }
        }

        if (!normalizedQuery) {
          return true;
        }

        const yearLabel = item.year ? String(item.year) : "";
        return [item.title, formatMediaType(item.mediaType), yearLabel].some((value) =>
          value.toLocaleLowerCase().includes(normalizedQuery)
        );
      })
      .sort(compareMediaItems);
  }, [categoryItems, deferredSearchQuery, selectedDecade]);

  useEffect(() => {
    if (visibleCount >= filteredMediaItems.length) {
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

        setVisibleCount((count) => Math.min(count + VISIBLE_ITEMS_STEP, filteredMediaItems.length));
      },
      {
        rootMargin: "0px 0px 420px 0px"
      }
    );

    observer.observe(target);

    return () => {
      observer.disconnect();
    };
  }, [filteredMediaItems.length, visibleCount]);

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
                {isScanning ? "Reescaneando biblioteca..." : "Salvar pastas e reescanear tudo"}
              </Button>
            </Stack>
          </Stack>
        </SectionCard>
      ) : null}

      {!isFirstRun && categoryItems.length === 0 && !isScanning ? (
        <SectionCard>
          <Stack spacing={1.5} sx={{ py: 4 }}>
            <Typography variant="h5">{emptyCategoryTitle}</Typography>
            <Typography color="text.secondary">{emptyCategoryDescription}</Typography>
          </Stack>
        </SectionCard>
      ) : null}

      {!isFirstRun && categoryItems.length > 0 && filteredMediaItems.length === 0 ? (
        <SectionCard>
          <Stack spacing={1.5} sx={{ py: 4 }}>
            <Typography variant="h5">Nenhum resultado</Typography>
            <Typography color="text.secondary">Ajuste a busca para encontrar outros titulos.</Typography>
          </Stack>
        </SectionCard>
      ) : null}

      {filteredMediaItems.length > 0 ? (
        <>
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
            {filteredMediaItems.slice(0, visibleCount).map((item) => (
              <Stack key={item.id} sx={{ minWidth: 0 }}>
                <MediaPosterCard
                  item={item}
                  onClick={() =>
                    navigate(`/library/${encodeURIComponent(item.id)}`, {
                      state: { item, backToPath: location.pathname }
                    })
                  }
                />
              </Stack>
            ))}
          </Stack>

          {visibleCount < filteredMediaItems.length ? (
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

function compareMediaItems(left: HomeMediaItemDto, right: HomeMediaItemDto) {
  const leftYear = left.year ?? Number.NEGATIVE_INFINITY;
  const rightYear = right.year ?? Number.NEGATIVE_INFINITY;

  if (leftYear !== rightYear) {
    return rightYear - leftYear;
  }

  return left.title.localeCompare(right.title, "pt-BR");
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