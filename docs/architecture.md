# NetCrico Architecture

## Goals

- Keep the first iteration modular and easy to extend.
- Treat the player window as a first-class product surface.
- Keep filesystem, database, ffprobe, mpv, and metadata integrations in the Tauri backend.
- Keep the React frontend focused on presentation, navigation, and UI state.

## Frontend structure

- `src/app`: app bootstrap, router, providers, and theme.
- `src/pages`: main-window pages such as home, library, and settings.
- `src/player-app`: dedicated React surface for the player window.
- `src/features`: domain-oriented UI slices with their own components, services, and stores.
- `src/components`: reusable UI primitives and layout pieces.
- `src/services/tauri`: the only place where frontend code talks directly to Tauri commands and events.
- `src/stores`: app-wide Zustand stores that are not tied to a single feature.
- `src/types`: shared frontend contracts, domain shapes, and view models.

## Backend structure

- `src-tauri/src/app`: app composition, global state, and Tauri commands.
- `src-tauri/src/contracts`: request and response types crossing the frontend/backend boundary.
- `src-tauri/src/modules/library`: library querying, persistence orchestration, and item mapping.
- `src-tauri/src/modules/scanner`: recursive scan jobs, file detection, and change tracking.
- `src-tauri/src/modules/media`: ffprobe integration and media stream parsing.
- `src-tauri/src/modules/metadata`: TMDB provider abstractions and enrichment flows.
- `src-tauri/src/modules/player`: player session lifecycle, mpv IPC, subtitles, and playback progress.
- `src-tauri/src/modules/filesystem`: path handling, media roots, and local file access rules.
- `src-tauri/src/infrastructure`: SQLite, process launching, config, and low-level integrations.
- `src-tauri/src/support`: shared utility types like IDs, errors, and time helpers.

## Recommended responsibility split

Frontend React:

- page composition and navigation
- UI state and transient session state
- playback controls and overlays
- event subscription and DTO-to-view-model mapping

Backend Rust:

- scanner jobs
- filesystem access
- ffprobe execution
- SQLite persistence
- mpv process control
- metadata provider integrations
- player progress persistence

## Why a dedicated player window

The player remains visually integrated with the app while avoiding the fragility of using the WebView video stack as the main playback engine. The dedicated player window keeps the UX strong and still allows the backend to control a native video engine.
