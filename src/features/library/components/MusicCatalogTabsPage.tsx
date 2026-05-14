import { LibraryRootsField } from "@/components/ui/LibraryRootsField";
import { SectionCard } from "@/components/ui/SectionCard";
import { MusicSquareCard } from "@/features/library/components/MusicSquareCard";
import { useCatalogFiltersStore } from "@/features/library/store/catalogFilters.store";
import {
    getInitialSetupStatus,
    listMusicAlbums,
    pickLibraryRoot,
  rescanLibrary,
  saveLibraryRoots
} from "@/services/tauri/commands/library";
import { HomeMediaItemDto, InitialSetupStatusDto } from "@/types/contracts/library";
import { appendLibraryRootPath, normalizeLibraryRootPaths } from "@/utils/libraryRoots";
import { Alert, Box, Button, CircularProgress, Stack, Typography } from "@mui/material";
import { useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";

const INITIAL_VISIBLE_ITEMS = 60;
const VISIBLE_ITEMS_STEP = 40;

type MusicAlbumGroup = {
  artistName: string;
  albums: HomeMediaItemDto[];
};

export function MusicCatalogTabsPage() {
  const navigate = useNavigate();
  const [setupStatus, setSetupStatus] = useState<InitialSetupStatusDto | null>(null);
  const [albums, setAlbums] = useState<HomeMediaItemDto[]>([]);
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
  }, [deferredSearchQuery, selectedDecade]);

  async function refreshCatalog() {
    setIsLoading(true);

    try {
      const [nextStatus, nextAlbums] = await Promise.all([
        getInitialSetupStatus(),
        listMusicAlbums().catch(() => [] as HomeMediaItemDto[])
      ]);

      setSetupStatus(nextStatus);
  setSelectedPaths(nextStatus.libraryRootPaths);
      setErrorMessage(null);
      setAlbums(nextStatus.hasLibraryRoot ? nextAlbums : []);
      setVisibleCount(INITIAL_VISIBLE_ITEMS);
    } catch (error) {
      setErrorMessage(asMessage(error, "Nao foi possivel carregar a biblioteca musical."));
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
      setErrorMessage(asMessage(error, "Nao foi possivel reescanear a biblioteca musical."));
    } finally {
      setIsScanning(false);
    }
  }

  const isFirstRun = !setupStatus?.hasLibraryRoot;

  const groupedAlbums = useMemo<MusicAlbumGroup[]>(() => {
    const normalizedQuery = deferredSearchQuery.trim().toLocaleLowerCase();
    const groups = new Map<string, HomeMediaItemDto[]>();

    for (const album of albums) {
      if (selectedDecade !== null) {
        if (album.year === null || album.year < selectedDecade || album.year >= selectedDecade + 10) {
          continue;
        }
      }

      const artistName = getArtistNameFromAlbumPath(album.mediaPath);
      const yearLabel = album.year ? String(album.year) : "";
      const matchesArtist = normalizedQuery ? artistName.toLocaleLowerCase().includes(normalizedQuery) : true;
      const matchesAlbum = normalizedQuery
        ? [album.title, yearLabel].some((value) => value.toLocaleLowerCase().includes(normalizedQuery))
        : true;

      if (!matchesArtist && !matchesAlbum) {
        continue;
      }

      const artistAlbums = groups.get(artistName) ?? [];
      artistAlbums.push(album);
      groups.set(artistName, artistAlbums);
    }

    return Array.from(groups.entries())
      .map(([artistName, artistAlbums]) => ({
        artistName,
        albums: [...artistAlbums].sort(compareAlbums)
      }))
      .sort((left, right) => left.artistName.localeCompare(right.artistName, "pt-BR"));
  }, [albums, deferredSearchQuery, selectedDecade]);

  const totalAlbumCount = useMemo(
    () => groupedAlbums.reduce((total, group) => total + group.albums.length, 0),
    [groupedAlbums]
  );

  const visibleAlbumGroups = useMemo(() => {
    let remaining = visibleCount;

    return groupedAlbums.reduce<MusicAlbumGroup[]>((groups, group) => {
      if (remaining <= 0) {
        return groups;
      }

      const visibleAlbums = group.albums.slice(0, remaining);

      if (!visibleAlbums.length) {
        return groups;
      }

      groups.push({
        artistName: group.artistName,
        albums: visibleAlbums
      });

      remaining -= visibleAlbums.length;
      return groups;
    }, []);
  }, [groupedAlbums, visibleCount]);

  useEffect(() => {
    if (visibleCount >= totalAlbumCount) {
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

        setVisibleCount((count) => Math.min(count + VISIBLE_ITEMS_STEP, totalAlbumCount));
      },
      {
        rootMargin: "0px 0px 420px 0px"
      }
    );

    observer.observe(target);

    return () => {
      observer.disconnect();
    };
  }, [totalAlbumCount, visibleCount]);

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

      {!isFirstRun && totalAlbumCount === 0 ? (
        <SectionCard>
          <Stack spacing={1.5} sx={{ py: 4 }}>
            <Typography variant="h5">Nenhum album encontrado</Typography>
            <Typography color="text.secondary">
              Adicione albuns em music/Artista/Album (Ano) e reescaneie a biblioteca.
            </Typography>
          </Stack>
        </SectionCard>
      ) : null}

      {visibleAlbumGroups.length > 0 ? (
        <>
          <Stack spacing={3.25}>
            {visibleAlbumGroups.map((group) => (
              <Stack key={group.artistName} spacing={1.25}>
                <Typography variant="h5" sx={{ fontSize: { xs: "1.1rem", md: "1.25rem" } }}>
                  {group.artistName}
                </Typography>

                <Box
                  sx={{
                    display: "grid",
                    gap: { xs: 1.25, md: 1.5 },
                    gridTemplateColumns: "repeat(auto-fill, minmax(min(100%, 150px), 1fr))"
                  }}
                >
                  {group.albums.map((item) => (
                    <MusicSquareCard
                      key={item.id}
                      item={item}
                      onClick={() =>
                        navigate(`/library/${encodeURIComponent(item.id)}`, {
                          state: { item, backToPath: "/music" }
                        })
                      }
                    />
                  ))}
                </Box>
              </Stack>
            ))}
          </Stack>

          {visibleCount < totalAlbumCount ? (
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

function compareAlbums(left: HomeMediaItemDto, right: HomeMediaItemDto) {
  const leftYear = left.year ?? Number.NEGATIVE_INFINITY;
  const rightYear = right.year ?? Number.NEGATIVE_INFINITY;

  if (leftYear !== rightYear) {
    return rightYear - leftYear;
  }

  return left.title.localeCompare(right.title, "pt-BR");
}

function getArtistNameFromAlbumPath(mediaPath: string) {
  const normalizedPath = mediaPath.replace(/\\/g, "/").replace(/\/+$/, "");
  const pathSegments = normalizedPath.split("/").filter(Boolean);
  const artistSegment = pathSegments[pathSegments.length - 2] ?? "Artista desconhecido";

  return artistSegment
    .replace(/[._]+/g, " ")
    .split(/\s+/)
    .filter(Boolean)
    .join(" ");
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