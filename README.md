# NetCrico

NetCrico is a Windows-first local media center MVP built with Tauri 2, React, TypeScript, Rust, and SQLite.

It is designed around a simple idea: keep local media playback native and reliable, keep the UI fast and pragmatic, and avoid making internet access a hard requirement. The app scans a user-selected library folder, builds a local SQLite index, and exposes dedicated experiences for movies, TV series, concerts, documentaries, stand-up, and music.

## MVP status

This repository is no longer just a scaffold. The current MVP already includes:

- Library scanning for movies, TV series, concerts, stand-up, documentaries, and music.
- Native embedded video playback through a Rust-managed libmpv surface on Windows.
- Album-first music browsing grouped by artist.
- Inline album playback with play/pause, playlist, progress, and seeking directly on the album detail page.
- Local metadata enrichment for movies and TV series via folder-level config files and companion artwork.
- Full offline behavior during scanning.
- Remote OMDb/TMDb helpers remain in the codebase, but they are not part of the active scan flow.

## Core product behavior

### Video

- Movies and series are indexed from a local folder tree.
- Video playback is handled by a backend-managed libmpv instance embedded into the main application window.
- TV series are grouped into seasons and episodes.
- The detail view can show year, IMDb rating, genre, director, and actors when available.

### Music

- Music is scanned as `Artist -> Album -> Tracks`.
- The music listing is album-first and grouped by artist.
- Album detail includes inline playback, playlist controls, and per-track seeking.
- Album art is resolved from local image files first, then from embedded audio artwork when needed.
- Album year is taken from audio metadata first and falls back to the album folder name when written as `(YYYY)`.

### Metadata

- The app works fully offline.
- Remote metadata is optional and additive.
- Local folder metadata always wins over remote metadata.
- If remote metadata fails, times out, or no API keys are configured, scanning still completes using only local data.
- Remote poster and metadata responses are cached in the Tauri app data directory when available.

## Library layout

NetCrico expects a pragmatic folder structure under one library root selected in the Settings page.

```text
LibraryRoot/
	movies/
		The Matrix (1999)/
			movie.mkv
			poster.jpg
			metadata.json

	shows/
		Breaking Bad/
			poster.jpg
			metadata.json
			Season 01/
				S01E01.mkv
				S01E02.mkv

	concerts/
		Artist Name - Live at Somewhere/
			concert.mp4
			poster.jpg

	documentary/
		Planet Earth/
			documentary.mkv

	standup/
		Comedian Name - Special/
			special.mkv

	music/
		Artist Name/
			artist.jpg
			Album Name (2024)/
				cover.jpg
				01 Intro.flac
				02 Track Name.flac
```

### Supported category roots

- `movies`
- `shows`
- `concerts`
- `standup`
- `documentary`
- `documentaries`
- `music`

## Local metadata files

For movies and TV series, NetCrico can read local metadata from any of these files in the media folder:

- `.local-cinema.json`
- `metadata.json`
- `media.json`

Relative `posterPath` values are resolved from the same folder that contains the config file.

Example:

```json
{
	"year": 1999,
	"posterPath": "poster.jpg",
	"genre": "Sci-Fi, Action",
	"director": "The Wachowskis",
	"actors": "Keanu Reeves, Laurence Fishburne, Carrie-Anne Moss",
	"imdbRating": "8.7",
	"imdbId": "tt0133093",
	"tmdbId": "603"
}
```

### Metadata precedence

The final value resolution order is intentionally conservative:

1. Local metadata file
2. Local artwork discovered from the folder structure

The current scan flow stops there. Remote values are not consulted while indexing the library.

## Optional remote metadata enrichment

Remote enrichment helpers currently target:

- Movies
- TV series

They are intentionally kept out of the active scanner flow for now. The environment variables and backend integration remain in the codebase for future reintroduction.

Supported environment variables:

- `OMDB_API_KEY`
- `TMDB_API_KEY`

Example PowerShell session:

```powershell
$env:OMDB_API_KEY = "your-omdb-key"
$env:TMDB_API_KEY = "your-tmdb-key"
npm run tauri:dev
```

If the keys are missing, invalid, or the machine is offline, the app still behaves like a purely local media center. At the moment, that is also the default scan behavior even when the keys are present.

## Architecture overview

NetCrico keeps the responsibility split strict:

### Frontend

- React 18 + Vite + TypeScript
- Material UI for the desktop shell and detail pages
- React Router with a hash router
- Zustand for app-level UI state
- `src/services/tauri` as the only direct bridge to Tauri commands and events

Frontend responsibilities:

- page composition and navigation
- search and filter state
- detail page presentation
- inline music playback UI
- playback controls for the embedded video route

### Backend

- Tauri 2 on Rust
- `rusqlite` with bundled SQLite
- `walkdir` for recursive scanning
- `lofty` for audio metadata and embedded cover extraction
- `reqwest` for optional OMDb/TMDb metadata enrichment
- `libmpv2` for native video playback

Backend responsibilities:

- filesystem access
- library scanning and normalization
- SQLite persistence
- embedded video engine lifecycle
- metadata resolution and caching
- DTO mapping for the frontend

### Data model

The MVP keeps persistence intentionally simple.

- A single `media_items` table stores top-level items and hierarchical child items.
- TV episodes are linked to their parent show.
- Music albums are linked to their parent artist.
- Music tracks are linked to their parent album.

This model keeps the scan pipeline simple while still supporting nested detail pages.

### Playback model

- Video playback uses a native libmpv surface hosted inside the main Tauri window.
- Music album playback on the detail page uses inline HTML audio controls for a faster UX.

## Repository structure

```text
src/
	app/              React bootstrap, providers, router, theme
	components/       Reusable layout and UI pieces
	features/         Domain-focused frontend slices
	pages/            Route-level pages
	services/tauri/   Typed frontend access to Tauri commands and events
	stores/           App-wide Zustand stores
	types/            Shared frontend contracts

src-tauri/src/
	app/              App composition and Tauri commands
	contracts/        Backend DTOs and player contracts
	infrastructure/   SQLite and low-level runtime integrations
	modules/library/  Library queries over the database
	modules/scanner/  Filesystem scanning and normalization
	modules/metadata/ Optional OMDb/TMDb enrichment and local metadata resolution
	modules/player/   Embedded libmpv lifecycle and control
```

Notes:

- The `drizzle/` folder is kept for schema modeling and future tooling, but the runtime database layer currently uses `rusqlite` directly in the Tauri backend.
- `docs/architecture.md` is the best high-level architecture reference for the current MVP.
- `docs/decisions/0001-player-window.md` is kept as historical context for the earlier playback direction.

## Prerequisites

### Windows runtime requirements

1. Node.js 20 or newer
2. Rust toolchain via rustup with the MSVC target
3. Microsoft C++ Build Tools or Visual Studio with Desktop development for C++
4. WebView2 Runtime
5. mpv/libmpv runtime available on the machine for embedded video playback

### Recommended developer tools

1. Git
2. VS Code
3. Rust Analyzer
4. A SQLite viewer extension

## Getting started

Install JavaScript dependencies:

```powershell
npm install
```

The Rust dependency graph is resolved automatically when you first build or run the Tauri app.

## Development workflow

### Run the desktop app

```powershell
npm run tauri:dev
```

### Run the frontend only

```powershell
npm run dev
```

This is useful for UI work, but anything that depends on Tauri commands such as scanning, settings, or native playback requires the desktop app.

### Production build

```powershell
npm run build
```

### Additional useful checks

```powershell
npm run typecheck
Set-Location src-tauri
cargo check
```

## Settings and rescanning

The Settings page allows you to:

- choose the library root folder
- save and rescan the current library
- fully reset the SQLite database and rebuild the library index

The reset option is the safest way to apply structural scan changes during development.

## Current limitations

- Embedded native video playback is currently implemented for Windows.
- Remote metadata helpers for movies and TV series are currently disabled in the active scan flow.
- API keys are read from environment variables, not from an in-app settings form yet.
- The scanner is optimized for predictable folder organization rather than highly inconsistent naming schemes.

## Why this architecture

The app deliberately keeps playback, scanning, persistence, and metadata in the Rust backend while the React frontend stays focused on navigation and presentation.

That tradeoff keeps the MVP easier to evolve:

- local files stay under native control
- playback avoids browser codec limitations
- offline behavior stays first-class
- metadata remains optional instead of becoming a hard dependency

## References

- `docs/architecture.md`
- `docs/decisions/0001-player-window.md` (historical playback ADR)
