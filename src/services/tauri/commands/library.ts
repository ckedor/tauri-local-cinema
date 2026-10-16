import { invokeCommand } from "@/services/tauri/client/invoke";
import {
  DirectorSummaryDto,
    HomeMediaItemDto,
    ImportIdentityDto,
    ImportPlanDto,
    InitialSetupStatusDto,
    MetadataApiSettingsDto,
  MovieDirectorDetailDto,
    MusicAlbumDetailDto,
    MusicArtistDetailDto,
    NewItemEntryDto,
    RefreshPlanDto,
    RefreshResultDto,
    RefreshSelectionDto,
    SeasonSubtitlesDto,
    ShowDetailDto,
    SubtitleDownloadResultDto,
    SubtitleSettingsDto,
    SubtitleSyncSummaryDto
} from "@/types/contracts/library";

export async function getInitialSetupStatus() {
  return invokeCommand<InitialSetupStatusDto>("get_initial_setup_status");
}

export async function pickLibraryRoot() {
  return invokeCommand<string | null>("pick_library_root");
}

export async function saveLibraryRoot(path: string) {
  return invokeCommand<void>("save_library_root", { path });
}

export async function saveLibraryRoots(paths: string[]) {
  return invokeCommand<void>("save_library_roots", { paths });
}

export async function getMetadataApiSettings() {
  return invokeCommand<MetadataApiSettingsDto>("get_metadata_api_settings");
}

export async function saveMetadataApiSettings(tmdbApiKey: string, anthropicApiKey: string) {
  return invokeCommand<void>("save_metadata_api_settings", {
    tmdbApiKey,
    anthropicApiKey
  });
}

export async function pickImportFiles() {
  return invokeCommand<string[]>("pick_import_files");
}

export async function importPreview(sourcePaths: string[], category: string | null) {
  return invokeCommand<ImportPlanDto>("import_preview", { sourcePaths, category });
}

export async function importRebuildPlan(identity: ImportIdentityDto) {
  return invokeCommand<ImportPlanDto>("import_rebuild_plan", { identity });
}

export async function importApply(identity: ImportIdentityDto) {
  return invokeCommand<string>("import_apply", { identity });
}

export async function startInitialScan() {
  return invokeCommand<number>("start_initial_scan");
}

export async function rescanLibrary() {
  return invokeCommand<number>("rescan_library");
}

export async function resetLibraryDatabaseAndRescan() {
  return invokeCommand<number>("reset_library_database_and_rescan");
}

export async function openMediaInExplorer(mediaPath: string) {
  return invokeCommand<void>("open_media_in_explorer", { mediaPath });
}

export async function listHomeMedia() {
  return invokeCommand<HomeMediaItemDto[]>("list_home_media");
}

export async function listPortableNewItems() {
  return invokeCommand<NewItemEntryDto[]>("list_portable_new_items");
}

export async function upsertPortableNewItem(mediaPath: string, mediaType: string, addedAt: number) {
  return invokeCommand<void>("upsert_portable_new_item", {
    mediaPath,
    mediaType,
    addedAt
  });
}

export async function removePortableNewItem(mediaPath: string) {
  return invokeCommand<void>("remove_portable_new_item", { mediaPath });
}

export async function getMediaItem(mediaId: string) {
  return invokeCommand<HomeMediaItemDto | null>("get_media_item", { mediaId });
}

export async function getShowDetail(showId: string) {
  return invokeCommand<ShowDetailDto>("get_show_detail", { showId });
}

export async function getMusicArtistDetail(artistId: string) {
  return invokeCommand<MusicArtistDetailDto>("get_music_artist_detail", { artistId });
}

export async function listMusicAlbums() {
  return invokeCommand<HomeMediaItemDto[]>("list_music_albums");
}

export async function listMovieDirectors() {
  return invokeCommand<DirectorSummaryDto[]>("list_movie_directors");
}

export async function getMovieDirectorDetail(directorId: string) {
  return invokeCommand<MovieDirectorDetailDto | null>("get_movie_director_detail", { directorId });
}

export async function getMusicAlbumDetail(albumId: string) {
  return invokeCommand<MusicAlbumDetailDto>("get_music_album_detail", { albumId });
}
export async function refreshPreview(mediaId: string, tmdbId: string | null) {
  return invokeCommand<RefreshPlanDto>("refresh_preview", { mediaId, tmdbId });
}

export async function refreshApply(selection: RefreshSelectionDto) {
  return invokeCommand<RefreshResultDto>("refresh_apply", { selection });
}

/** `seasonNumber` null searches every season. */
export async function searchSeasonSubtitles(mediaId: string, seasonNumber: number | null) {
  return invokeCommand<SeasonSubtitlesDto>("subtitles_search", { mediaId, seasonNumber });
}

export async function downloadSubtitles(mediaId: string, downloads: { fileId: number; targetPath: string; episodeId: string; embeddedTrack: number | null }[]) {
  return invokeCommand<SubtitleDownloadResultDto>("subtitles_download", { mediaId, downloads });
}

/** Aligns the subtitle files already downloaded; `seasonNumber` null covers every season. */
export async function syncExistingSubtitles(mediaId: string, seasonNumber: number | null) {
  return invokeCommand<SubtitleSyncSummaryDto>("subtitles_sync_existing", { mediaId, seasonNumber });
}

export async function fetchEpisodeSubtitles(mediaId: string, episodeId: string) {
  return invokeCommand<SubtitleDownloadResultDto>("subtitles_fetch_episode", { mediaId, episodeId });
}

export async function getSubtitleSettings() {
  return invokeCommand<SubtitleSettingsDto>("get_subtitle_settings");
}

export async function saveSubtitleSettings(settings: SubtitleSettingsDto) {
  return invokeCommand<void>("save_subtitle_settings", { settings });
}
