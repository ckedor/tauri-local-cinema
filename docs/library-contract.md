# Library contract

The on-disk layout NetCrico owns. Everything the app writes (the `+` import,
future in-app edits) produces exactly this, and the scanner only needs to
understand this. Anything else in the library is a bug to fix on disk, not a
case for the scanner to guess at.

First applied by hand to `shows/South Park (1997)` (2026-10-16).

## General rules

- **Identity comes from TMDb.** Every movie-like item and every show has a
  `tmdbId` in its `.local-cinema.json`. Titles, years and episode names are
  TMDb's (English), not the release name.
- **Names:** `<Title> (<Year>)`. In file and folder names `": "` becomes
  `" - "` (`South Park: Bigger, Longer & Uncut` ->
  `South Park - Bigger, Longer & Uncut`) and `< > " / \ | ? *` are dropped,
  so names are valid on Windows and Linux.
- **Subtitles** sit next to their video with the same name plus an optional
  language tag and qualifier: `<video name>[.<lang>[.<qualifier>]].<srt|ass|ssa|vtt|sub>`,
  e.g. `Movie (1999).en.srt`, `Movie (1999).en.sdh.srt`.
- **Artwork:** `poster.jpg` (2:3, the cards) and `cover.jpg` (landscape, the
  detail page background) in the item folder; either one stands in for the other.
- **No release leftovers:** no `.nfo`, `.sfv`, `.rar`, tracker `.txt`,
  `sample` clips, `.exe`, release-group images, DVD menu loops.
- **Nothing is deleted by a reorganization.** Removed files go to
  `<library root>/_quarantine/<item>/<original relative path>`, with an
  `undo-*.json` log of every move. The scanner ignores `_quarantine`.

## Movies, documentaries, stand-up, concerts

```
movies/<Title> (<Year>)/
  <Title> (<Year>).<ext>
  <Title> (<Year>)[.<lang>].srt
  poster.jpg
  .local-cinema.json
```

Same shape under `documentary/`, `standup/`, `concerts/`. One video per
folder; a film split in parts is `<Title> (<Year>) - Part N.<ext>` (a part may
carry a label: `- Part 2 (New York 1973)`). Bonus videos go in
`<Title> (<Year>)/Extras/<Name>.<ext>`; the scanner doesn't list them as
items.

Items TMDb doesn't have (bootlegs, TV specials) keep a clean
`<Artist> - <Title> (<Year>)` name and have no `tmdbId`. A recording in the
wrong category moves: audio-only releases go to `music/`, concert films to
`concerts/`.

## Shows

```
shows/<Title> (<Year>)/                  year = first air year
  .local-cinema.json                     tmdbId of the show (+ "specials", below)
  poster.jpg | cover.jpg
  Season 01/
    S01E01 - <TMDb episode name>.<ext>
    S01E01 - <TMDb episode name>[.<lang>].srt
  Specials/
    S00E07 - <TMDb episode name>.<ext>   TMDb season-0 numbering
    Special - <Title> (<Year>).<ext>     specials TMDb lists as movies
  Extras/
    S12 - <Name>.<ext>                   bonus material tied to a season
    S14E05 - <Episode name> (<Variant>).<ext>   alternate cut of an episode
    <Name>.<ext>                         bonus material for the whole show
```

- **`Season NN`** (two digits) holds only the episodes of that TMDb season,
  one file per episode, numbered as TMDb numbers them.
- **`Specials`** are things you watch as episodes: TMDb season-0 entries
  (`S00Exx`, TMDb's numbering, not TVDB's or the release's), and specials TMDb
  catalogs as standalone movies (e.g. the Paramount+ South Park specials).
  The latter are named `Special - <Title> (<Year>)` and listed in the show's
  `.local-cinema.json` with their TMDb movie id:

  ```json
  "specials": {
    "Special - Post COVID (2021).mkv": { "tmdbMovieId": "874299", "year": 2021, "title": "Post COVID" }
  }
  ```
- **`Extras`** is everything else: deleted scenes, commentaries, featurettes,
  panels, trailers, and alternate cuts of episodes (`(Uncensored)`,
  `(Extended)`). When the same episode exists twice, the best-quality copy is
  the episode and other cuts worth keeping go here.
- A theatrical movie of a show is a movie and lives in `movies/`.

### Exceptions

- **Chaves (1973), Chapolin (1973):** Brazilian dubbed collections. They keep
  the dubbing's own numbering and Portuguese titles (`283 - Os Hóspedes do
  Senhor Barriga.mkv`, or title only), because TMDb's Spanish episode list
  doesn't match them. The show folder still has the TMDb id.
- **DVD rips** (`VIDEO_TS` `.vob`/`.ifo`) aren't in the contract yet: they need
  remuxing to one `.mkv` (no re-encode). Until then they're untouched and the
  app can't see them.
- **Subtitle archives** that need `unrar` (solid RAR) sit next to their movie
  as `<Title> (<Year>).subs.rar` until they can be extracted.

## Music

Unchanged for now: `music/<Artist>/<Album> (<Year>)/<NN> <Track>.<ext>`,
`artist.jpg` in the artist folder, `cover.jpg` in the album folder.

## How the app reads it

- `Season NN/` -> season NN. `Specials/` -> season 0, shown as "Especiais".
  `Extras/` -> season -1 (`EXTRAS_SEASON`), shown as "Extras". On the show
  page they come after the regular seasons, in that order.
- `S01E02 - Name` -> episode 2, title `Name` verbatim. A `Special - Title (Year)`
  file takes its title from the `specials` map (falling back to `Title`) and
  sorts after the numbered specials, oldest first. An extra's title is its
  file name as-is.

## What the app does not understand yet

- The `+` import doesn't place files in `Specials/` or `Extras/`.
- The TMDb movie ids in `specials` aren't used for metadata or posters.
- Episodes in Extras (`S14E05 - 200 (Uncensored)`) aren't linked to the
  episode they are a cut of.
