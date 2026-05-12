import { useCatalogFiltersStore } from "@/features/library/store/catalogFilters.store";
import FilterListRoundedIcon from "@mui/icons-material/FilterListRounded";
import SettingsRoundedIcon from "@mui/icons-material/SettingsRounded";
import {
    AppBar,
    Box,
    Button,
    ButtonBase,
    Checkbox,
    Divider,
    FormControlLabel,
    IconButton,
    Popover,
    Stack,
    TextField,
    Toolbar,
    Tooltip,
    Typography
} from "@mui/material";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { Outlet, useLocation, useNavigate } from "react-router-dom";

const currentDecade = Math.floor(new Date().getFullYear() / 10) * 10;
const decadeOptions = Array.from(
  { length: Math.floor((currentDecade - 1930) / 10) + 1 },
  (_, index) => 1930 + index * 10
);

const genreOptions = [
  "Acao",
  "Animacao",
  "Aventura",
  "Comedia",
  "Crime",
  "Documentario",
  "Drama",
  "Fantasia",
  "Ficcao cientifica",
  "Misterio",
  "Musical",
  "Romance",
  "Suspense",
  "Terror"
];

type NavigationItem = {
  label: string;
  path: string;
  matches?: string[];
};

const navigationItems: NavigationItem[] = [
  { label: "Filmes", path: "/movies" },
  { label: "Series", path: "/tv-shows" },
  { label: "Shows", path: "/concerts" },
  { label: "Musica", path: "/music" },
  { label: "Stand-up", path: "/standup" },
  { label: "Documentarios", path: "/documentary" }
];

export function AppShell() {
  const location = useLocation();
  const navigate = useNavigate();
  const [filtersAnchor, setFiltersAnchor] = useState<HTMLElement | null>(null);
  const searchQuery = useCatalogFiltersStore((state) => state.searchQuery);
  const selectedDecade = useCatalogFiltersStore((state) => state.selectedDecade);
  const setSearchQuery = useCatalogFiltersStore((state) => state.setSearchQuery);
  const setSelectedDecade = useCatalogFiltersStore((state) => state.setSelectedDecade);
  const clearFilters = useCatalogFiltersStore((state) => state.clearFilters);

  const isFiltersOpen = Boolean(filtersAnchor);
  const isSettingsSelected = location.pathname === "/settings" || location.pathname.startsWith("/settings/");
  const isListingRoute = matchesListingRoute(location.pathname);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if ((event.key !== "F11" && event.code !== "F11") || event.repeat) {
        return;
      }

      event.preventDefault();

      void toggleAppFullscreen();
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  async function toggleAppFullscreen() {
    try {
      const currentWindow = getCurrentWindow();
      const nextFullscreen = !(await currentWindow.isFullscreen());
      await currentWindow.setFullscreen(nextFullscreen);
    } catch {
      // Ignore when running outside the Tauri desktop shell.
    }
  }

  return (
    <Box
      sx={{
        minHeight: "100vh",
        background: isListingRoute
          ? "radial-gradient(circle at top, rgba(var(--app-accent-rgb), 0.18), transparent 35%), linear-gradient(180deg, rgba(var(--app-bg-rgb), 1) 0%, rgba(var(--app-bg-deep-rgb), 1) 100%)"
          : "linear-gradient(180deg, rgba(var(--app-bg-rgb), 1) 0%, rgba(var(--app-bg-deep-rgb), 1) 100%)"
      }}
    >
      <AppBar position="sticky" color="transparent" elevation={0}>
        <Toolbar
          sx={{
            gap: 1.5,
            flexWrap: "wrap",
            alignItems: "center",
            justifyContent: "space-between",
            backdropFilter: "blur(18px)",
            py: 1
          }}
        >
          <ButtonBase
            onClick={() => navigate("/")}
            sx={{
              borderRadius: 2,
              px: 0.5,
              py: 0.25,
              alignSelf: { xs: "flex-start", md: "center" }
            }}
          >
            <Typography variant="h6" sx={{ fontWeight: 700, pr: 1 }}>
              NetCrico
            </Typography>
          </ButtonBase>

          <Stack
            direction="row"
            spacing={1}
            useFlexGap
            sx={{
              flexGrow: 1,
              flexWrap: "wrap",
              alignItems: "center",
              justifyContent: "flex-end",
              minWidth: 0
            }}
          >
            <TextField
              placeholder="Buscar na biblioteca"
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
              size="small"
              InputProps={{
                startAdornment: (
                  <Box
                    component="svg"
                    viewBox="0 0 24 24"
                    aria-hidden="true"
                    sx={{ width: 18, height: 18, color: "text.secondary", mr: 1 }}
                  >
                    <Box
                      component="path"
                      d="M10.5 4a6.5 6.5 0 1 0 4.03 11.6l4.43 4.43 1.06-1.06-4.43-4.43A6.5 6.5 0 0 0 10.5 4Zm0 1.5a5 5 0 1 1 0 10 5 5 0 0 1 0-10Z"
                      fill="currentColor"
                    />
                  </Box>
                )
              }}
              sx={{
                minWidth: { xs: "100%", md: 320 },
                maxWidth: 420,
                flexShrink: 0,
                ".MuiOutlinedInput-root": {
                  borderRadius: 999,
                  backgroundColor: "rgba(27, 31, 36, 0.72)",
                  backdropFilter: "blur(12px)"
                }
              }}
            />

            {navigationItems.map((item) => {
              const selected = item.matches
                ? item.matches.some((path) =>
                    path === "/" ? location.pathname === path : location.pathname.startsWith(path)
                  )
                : location.pathname === item.path || location.pathname.startsWith(`${item.path}/`);

              return (
                <Button
                  key={item.path}
                  color={selected ? "primary" : "inherit"}
                  onClick={() => navigate(item.path)}
                  variant={selected ? "contained" : "text"}
                >
                  {item.label}
                </Button>
              );
            })}

            <Tooltip title={selectedDecade ? `Filtros ativos: ${selectedDecade}` : "Filtros"}>
              <IconButton
                aria-label="Abrir filtros"
                color={selectedDecade ? "secondary" : "inherit"}
                onClick={(event) => setFiltersAnchor(event.currentTarget)}
                sx={{
                  border: "1px solid rgba(255,255,255,0.12)",
                  backgroundColor: selectedDecade ? "rgba(209, 73, 91, 0.18)" : "transparent"
                }}
              >
                <FilterListRoundedIcon />
              </IconButton>
            </Tooltip>

            <Tooltip title="Configuracoes">
              <IconButton
                aria-label="Abrir configuracoes"
                color={isSettingsSelected ? "primary" : "inherit"}
                onClick={() => navigate("/settings")}
                sx={{
                  border: "1px solid rgba(255,255,255,0.12)",
                  backgroundColor: isSettingsSelected ? "rgba(255,255,255,0.08)" : "transparent"
                }}
              >
                <SettingsRoundedIcon />
              </IconButton>
            </Tooltip>
          </Stack>
        </Toolbar>
      </AppBar>

      <Popover
        open={isFiltersOpen}
        anchorEl={filtersAnchor}
        onClose={() => setFiltersAnchor(null)}
        anchorOrigin={{ vertical: "bottom", horizontal: "left" }}
        transformOrigin={{ vertical: "top", horizontal: "left" }}
        PaperProps={{
          sx: {
            mt: 1,
            p: 2,
            width: minWidthForFilters(),
            backgroundColor: "rgba(18, 21, 26, 0.96)",
            backdropFilter: "blur(16px)",
            border: "1px solid rgba(255,255,255,0.08)"
          }
        }}
      >
        <Stack spacing={2}>
          <Stack direction="row" justifyContent="space-between" alignItems="center">
            <Typography variant="subtitle1" sx={{ fontWeight: 700 }}>
              Filtros
            </Typography>
            <Button color="inherit" onClick={clearFilters} size="small">
              Limpar
            </Button>
          </Stack>

          <Divider sx={{ borderColor: "rgba(255,255,255,0.08)" }} />

          <Box
            sx={{
              display: "grid",
              gap: 3,
              gridTemplateColumns: { xs: "1fr", md: "minmax(0, 1fr) minmax(0, 1fr)" }
            }}
          >
            <Stack spacing={1.5}>
              <Typography variant="overline" sx={{ color: "text.secondary", letterSpacing: 1.4 }}>
                Decada
              </Typography>

              <Stack spacing={1} sx={{ maxHeight: 280, overflowY: "auto", pr: 1 }}>
                <Button
                  color={!selectedDecade ? "secondary" : "inherit"}
                  onClick={() => setSelectedDecade(null)}
                  sx={{ justifyContent: "flex-start" }}
                  variant={!selectedDecade ? "contained" : "text"}
                >
                  Todas
                </Button>

                {decadeOptions.map((decade) => (
                  <Button
                    key={decade}
                    color={selectedDecade === decade ? "secondary" : "inherit"}
                    onClick={() => setSelectedDecade(decade)}
                    sx={{ justifyContent: "flex-start" }}
                    variant={selectedDecade === decade ? "contained" : "text"}
                  >
                    {decade}
                  </Button>
                ))}
              </Stack>
            </Stack>

            <Stack spacing={1.5}>
              <Typography variant="overline" sx={{ color: "text.secondary", letterSpacing: 1.4 }}>
                Genero
              </Typography>

              <Stack spacing={0.3} sx={{ maxHeight: 280, overflowY: "auto", pr: 1 }}>
                {genreOptions.map((genre) => (
                  <FormControlLabel
                    key={genre}
                    control={<Checkbox disabled size="small" />}
                    label={genre}
                    sx={{ m: 0, color: "text.secondary" }}
                  />
                ))}
              </Stack>

              <Typography variant="caption" sx={{ color: "text.secondary" }}>
                Generos entram quando a etapa de metadados estiver ativa.
              </Typography>
            </Stack>
          </Box>
        </Stack>
      </Popover>

      <Box component="main" sx={{ p: 3 }}>
        <Outlet />
      </Box>
    </Box>
  );
}

function minWidthForFilters() {
  return { xs: "calc(100vw - 24px)", sm: 560 };
}

function matchesListingRoute(pathname: string) {
  return pathname === "/"
    || pathname.startsWith("/movies")
    || pathname.startsWith("/tv-shows")
    || pathname.startsWith("/concerts")
    || pathname.startsWith("/music")
    || pathname.startsWith("/standup")
    || pathname.startsWith("/documentary");
}
