# ADR 0001: Dedicated player window driven by the Tauri backend

## Status

Accepted.

## Decision

The application will use a dedicated Tauri window for playback UI and a backend-controlled mpv process as the video engine.

## Rationale

- Playback is a central part of the product experience.
- A dedicated app window preserves product cohesion better than launching the raw mpv window as the user-facing experience.
- Using mpv behind the backend reduces risk around codecs, subtitle formats, and large local files.
- Full native embed inside the main React layout is intentionally postponed until the product and playback model are validated.
