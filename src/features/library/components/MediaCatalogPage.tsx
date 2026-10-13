import { LibraryRootsField } from "@/components/ui/LibraryRootsField";
import { MediaPosterCard } from "@/features/library/components/MediaPosterCard";
import { supportsInterestList } from "@/features/library/services/interestList";
import { runLibraryScanAndMarkNewItems } from "@/features/library/services/newItemsSync";
import { CatalogSortOrder, useCatalogFiltersStore } from "@/features/library/store/catalogFilters.store";
import { useInterestListStore } from "@/features/library/store/interestList.store";
import { formatMediaType } from "@/features/library/utils/media";
import { getDisplayTitle, getOriginalTitle } from "@/features/library/utils/titles";
import {
    getInitialSetupStatus,
    listHomeMedia,
    pickLibraryRoot,
    rescanLibrary,
    saveLibraryRoots
} from "@/services/tauri/commands/library";
import { HomeMediaItemDto, InitialSetupStatusDto } from "@/types/contracts/library";
import { appendLibraryRootPath, normalizeLibraryRootPaths } from "@/utils/libraryRoots";
import BookmarkBorderRoundedIcon from "@mui/icons-material/BookmarkBorderRounded";
import BookmarkRoundedIcon from "@mui/icons-material/BookmarkRounded";
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
  const selectedGenres = useCatalogFiltersStore((state) => state.selectedGenres);
  const selectedCountries = useCatalogFiltersStore((state) => state.selectedCountries);
  const sortOrder = useCatalogFiltersStore((state) => state.sortOrder);
  const interestEntries = useInterestListStore((state) => state.entries);
  const toggleInterestItem = useInterestListStore((state) => state.toggleItem);
  const deferredSearchQuery = useDeferredValue(searchQuery);

  useEffect(() => {
    void refreshCatalog();
  }, []);

  useEffect(() => {
    setVisibleCount(INITIAL_VISIBLE_ITEMS);
  }, [deferredSearchQuery, selectedDecade, selectedGenres, selectedCountries, sortOrder, mediaTypes?.join("|")]);

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

        if (selectedGenres.length > 0 && !matchesFacetFilter(item.genre, selectedGenres)) {
          return false;
        }

        if (selectedCountries.length > 0 && !matchesFacetFilter(item.country, selectedCountries)) {
          return false;
        }

        if (!normalizedQuery) {
          return true;
        }

        const yearLabel = item.year ? String(item.year) : "";
        return [
          item.title,
          getDisplayTitle(item),
          getOriginalTitle(item) ?? "",
          formatMediaType(item.mediaType),
          yearLabel,
          item.genre ?? "",
          item.country ?? "",
          item.director ?? "",
          item.actors ?? ""
        ].some((value) =>
          value.toLocaleLowerCase().includes(normalizedQuery)
        );
      })
        .sort((left, right) => compareMediaItems(left, right, sortOrder));
      }, [categoryItems, deferredSearchQuery, selectedCountries, selectedDecade, selectedGenres, sortOrder]);

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
				{isScanning ? "Reescaneando biblioteca..." : "Salvar pastas e reescanear biblioteca"}
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
                  topAction={supportsInterestList(item)
                    ? {
                        label: interestEntries.some((entry) => entry.id === item.id)
                          ? "Remover de Novos"
                          : "Marcar como novo",
                        icon: interestEntries.some((entry) => entry.id === item.id)
                          ? <BookmarkRoundedIcon fontSize="small" />
                          : <BookmarkBorderRoundedIcon fontSize="small" />,
                        isActive: interestEntries.some((entry) => entry.id === item.id),
                        onClick: () => toggleInterestItem(item)
                      }
                    : undefined}
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

function compareMediaItems(left: HomeMediaItemDto, right: HomeMediaItemDto, sortOrder: CatalogSortOrder) {
  if (sortOrder === "yearDesc") {
    return compareOptionalNumbers(left.year, right.year, "desc") || compareTitles(left, right);
  }

  if (sortOrder === "yearAsc") {
    return compareOptionalNumbers(left.year, right.year, "asc") || compareTitles(left, right);
  }

  if (sortOrder === "ratingDesc") {
    return compareOptionalNumbers(parseRating(left.imdbRating), parseRating(right.imdbRating), "desc") || compareTitles(left, right);
  }

  if (sortOrder === "ratingAsc") {
    return compareOptionalNumbers(parseRating(left.imdbRating), parseRating(right.imdbRating), "asc") || compareTitles(left, right);
  }

  return compareTitles(left, right);
}

function compareTitles(left: HomeMediaItemDto, right: HomeMediaItemDto) {
  return getDisplayTitle(left).localeCompare(getDisplayTitle(right), "pt-BR");
}

function compareOptionalNumbers(
  left: number | null | undefined,
  right: number | null | undefined,
  direction: "asc" | "desc"
) {
  if (left == null && right == null) {
    return 0;
  }

  if (left == null) {
    return 1;
  }

  if (right == null) {
    return -1;
  }

  if (left !== right) {
    return direction === "desc" ? right - left : left - right;
  }

  return 0;
}

function parseRating(value: string | null) {
  if (!value) {
    return null;
  }

  const parsedValue = Number.parseFloat(value.replace(",", "."));
  return Number.isFinite(parsedValue) ? parsedValue : null;
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

function matchesFacetFilter(rawValue: string | null, selectedValues: string[]) {
  if (!rawValue) {
    return false;
  }

  const values = rawValue
    .split(",")
    .map((value) => value.trim().toLocaleLowerCase())
    .filter(Boolean);

  return selectedValues.some((value) => values.includes(value.toLocaleLowerCase()));
}