export type InitialSetupStatusDto = {
  libraryRootPath: string | null;
  libraryRootPaths: string[];
  hasLibraryRoot: boolean;
  mediaCount: number;
};

export type MetadataApiSettingsDto = {
  tmdbApiKey: string | null;
  anthropicApiKey: string | null;
};

/** Season number of a show's Extras folder; season 0 is Specials (library contract). */
export const EXTRAS_SEASON = -1;

export type ImportCategory = "movie" | "show" | "documentary" | "standup" | "concert";

export type ImportIdentityDto = {
  sourcePaths: string[];
  category: ImportCategory;
  title: string;
  year: number | null;
  tmdbId: string | null;
  existingShowId: string | null;
  defaultSeason: number | null;
  libraryRoot: string | null;
  /** Artwork picked in the dry run; null keeps TMDb's default poster. */
  posterUrl?: string | null;
  coverUrl?: string | null;
};

export type ImportFileOpDto = {
  source: string;
  target: string;
  sizeBytes: number;
  action: "copy" | "skip" | "conflict";
};

export type ImportPlanDto = {
  identity: ImportIdentityDto;
  tmdbTitle: string | null;
  posterUrl: string | null;
  overview: string | null;
  existingShow: { id: string; title: string; mediaPath: string; seasons: number[] } | null;
  posters: RefreshImageOptionDto[];
  backdrops: RefreshImageOptionDto[];
  libraryRoots: string[];
  targetFolder: string;
  files: ImportFileOpDto[];
  totalBytes: number;
  notes: string[];
  identifiedBy: "claude" | "heuristic" | "manual";
};

export type HomeMediaItemDto = {
  id: string;
  title: string;
  mediaType: string;
  year: number | null;
  posterPath: string | null;
  coverPath: string | null;
  overview: string | null;
  englishTitle: string | null;
  originalTitle: string | null;
  genre: string | null;
  director: string | null;
  actors: string | null;
  country: string | null;
  imdbRating: string | null;
  mediaPath: string;
  subtitlePath: string | null;
};

export type NewItemEntryDto = HomeMediaItemDto & {
  addedAt: number;
};

export type ShowEpisodeDto = {
  id: string;
  seasonNumber: number;
  episodeNumber: number | null;
  title: string;
  mediaPath: string;
  subtitlePath: string | null;
};

export type ShowSeasonDto = {
  seasonNumber: number;
  episodes: ShowEpisodeDto[];
};

export type ShowDetailDto = {
  showId: string;
  seasons: ShowSeasonDto[];
};

export type MusicArtistDetailDto = {
  artistId: string;
  albums: HomeMediaItemDto[];
};

export type MusicTrackDto = {
  id: string;
  trackNumber: number | null;
  title: string;
  mediaPath: string;
  posterPath: string | null;
};

export type MusicAlbumDetailDto = {
  albumId: string;
  tracks: MusicTrackDto[];
};

export type DirectorSummaryDto = {
  id: string;
  name: string;
  imagePath: string | null;
  movieCount: number;
};

export type MovieDirectorDetailDto = {
  directorId: string;
  name: string;
  imagePath: string | null;
  movies: HomeMediaItemDto[];
};
export type RefreshFieldChangeDto = {
  key: string;
  label: string;
  current: string | null;
  proposed: string | null;
};

export type RefreshImageOptionDto = {
  previewUrl: string;
  url: string;
  width: number;
  height: number;
  language: string | null;
};

export type FileMoveDto = { from: string; to: string };

export type EpisodeRenameDto = {
  seasonNumber: number;
  episodeNumber: number;
  from: string;
  to: string;
  companions: FileMoveDto[];
  matchedBy: "file" | "claude";
  conflict: boolean;
};

export type RefreshPlanDto = {
  mediaId: string;
  tmdbId: string;
  tmdbTitle: string;
  tmdbYear: number | null;
  fields: RefreshFieldChangeDto[];
  posters: RefreshImageOptionDto[];
  backdrops: RefreshImageOptionDto[];
  currentPoster: string | null;
  currentCover: string | null;
  renames: EpisodeRenameDto[];
  notes: string[];
};

export type RefreshSelectionDto = {
  mediaId: string;
  fields: { key: string; value: string | null }[];
  posterUrl: string | null;
  coverUrl: string | null;
  moves: FileMoveDto[];
};

export type RefreshResultDto = {
  moved: FileMoveDto[];
  undoLog: string | null;
};

export type SubtitleSettingsDto = {
  apiKey: string | null;
  username: string | null;
  password: string | null;
  languages: string;
};

export type SubtitleOfferDto = {
  episodeId: string;
  /** null for a movie, concert, stand-up or documentary. */
  seasonNumber: number | null;
  episodeNumber: number | null;
  episodeTitle: string;
  language: string;
  release: string;
  fileId: number;
  hashMatch: boolean;
  downloadCount: number;
  targetPath: string;
  /** "embedded": taken from the video itself (no download, no quota). */
  source: "embedded" | "opensubtitles";
  embeddedTrack: number | null;
};

export type SeasonSubtitlesDto = {
  languages: string[];
  offers: SubtitleOfferDto[];
  covered: number;
  notes: string[];
};

export type SubtitleSyncSummaryDto = {
  synced: number;
  alreadyInSync: number;
  /** Left alone: the video has no embedded text subtitle to align with. */
  noReference: number;
  notes: string[];
};

export type SubtitleDownloadResultDto = {
  downloaded: number;
  extracted: number;
  remaining: number | null;
  errors: string[];
  sync: SubtitleSyncSummaryDto;
};
