import { invokeCommand } from "@/services/tauri/client/invoke";
import {
  DirectorSummaryDto,
    HomeMediaItemDto,
    InitialSetupStatusDto,
    MetadataApiSettingsDto,
  MovieDirectorDetailDto,
    MusicAlbumDetailDto,
    MusicArtistDetailDto,
    NewItemEntryDto,
    ShowDetailDto
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

export async function saveMetadataApiSettings(tmdbApiKey: string, omdbApiKey: string) {
  return invokeCommand<void>("save_metadata_api_settings", {
    tmdbApiKey,
    omdbApiKey
  });
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