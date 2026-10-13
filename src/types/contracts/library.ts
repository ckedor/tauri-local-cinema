export type InitialSetupStatusDto = {
  libraryRootPath: string | null;
  libraryRootPaths: string[];
  hasLibraryRoot: boolean;
  mediaCount: number;
};

export type MetadataApiSettingsDto = {
  tmdbApiKey: string | null;
  omdbApiKey: string | null;
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