import { AppShell } from "@/components/layout/AppShell";
import { EmbeddedPlayerPage } from "@/features/player/components/EmbeddedPlayerPage";
import { HomePage } from "@/pages/home/HomePage";
import { InterestListPage } from "@/pages/interests/InterestListPage";
import {
    ConcertsPage,
    DocumentaryPage,
    MoviesPage,
    MusicPage,
    StandupPage,
    TVShowsPage
} from "@/pages/library/CategoryPages";
import { DirectorDetailPage } from "@/pages/library/DirectorDetailPage";
import { LibraryDetailPage } from "@/pages/library/LibraryDetailPage";
import { SettingsPage } from "@/pages/settings/SettingsPage";
import { Navigate, createHashRouter } from "react-router-dom";

export const router = createHashRouter([
  {
    path: "/",
    element: <AppShell />,
    children: [
      { index: true, element: <HomePage /> },
      { path: "library", element: <Navigate to="/" replace /> },
      { path: "library/:mediaId", element: <LibraryDetailPage /> },
      { path: "movies", element: <MoviesPage /> },
      { path: "movies/directors/:directorId", element: <DirectorDetailPage /> },
      { path: "tv-shows", element: <TVShowsPage /> },
      { path: "novos", element: <InterestListPage /> },
      { path: "interests", element: <Navigate to="/novos" replace /> },
      { path: "concerts", element: <ConcertsPage /> },
      { path: "music", element: <MusicPage /> },
      { path: "standup", element: <StandupPage /> },
      { path: "documentary", element: <DocumentaryPage /> },
      { path: "settings", element: <SettingsPage /> },
      { path: "player", element: <EmbeddedPlayerPage /> }
    ]
  }
]);
