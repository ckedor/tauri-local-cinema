import { invokeCommand } from "@/services/tauri/client/invoke";
import {
    HomeMediaItemDto,
    InitialSetupStatusDto,
    MusicAlbumDetailDto,
    MusicArtistDetailDto,
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

export async function startInitialScan() {
  return invokeCommand<number>("start_initial_scan");
}

export async function rescanLibrary() {
  return invokeCommand<number>("rescan_library");
}

export async function resetLibraryDatabaseAndRescan() {
  return invokeCommand<number>("reset_library_database_and_rescan");
}

export async function listHomeMedia() {
  return invokeCommand<HomeMediaItemDto[]>("list_home_media");
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

export async function getMusicAlbumDetail(albumId: string) {
  return invokeCommand<MusicAlbumDetailDto>("get_music_album_detail", { albumId });
}