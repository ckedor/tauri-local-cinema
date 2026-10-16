//! Missing subtitles for one season, from OpenSubtitles (REST API v1). Search
//! is by file hash (exact release) plus the show's TMDb id and episode;
//! downloads land next to the video as `<video name>.<lang>.<ext>` (library
//! contract). Free accounts have a small daily download quota, hence a
//! search the user picks from before anything is downloaded.

use super::{emit_step, file_stem, http_client, load_item};
use crate::app::state::AppState;
use crate::modules::library::LibraryModule;
use super::sync::{can_read_embedded, extract_track, sync_files, text_tracks, track_for_language, SyncSummary, SYNCABLE_EXTENSIONS};
use crate::modules::scanner::{has_extension, list_video_subtitles, SUBTITLE_EXTENSIONS};
use crate::modules::settings::{SettingsModule, SubtitleSettings};
use reqwest::blocking::{Client, RequestBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::thread;
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

const API_BASE: &str = "https://api.opensubtitles.com/api/v1";
const USER_AGENT: &str = "NetCrico v0.1.0";
// OpenSubtitles asks clients to stay well under its per-second limit.
const SEARCH_PAUSE: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleOffer {
	pub episode_id: String,
	/// None for a movie, concert, stand-up or documentary.
	pub season_number: Option<i32>,
	pub episode_number: Option<i32>,
	pub episode_title: String,
	pub language: String,
	pub release: String,
	pub file_id: i64,
	/// Made for this exact file (hash match), so it is in sync.
	pub hash_match: bool,
	pub download_count: i64,
	pub target_path: String,
	/// "embedded" (taken from the video, no download) or "opensubtitles".
	pub source: String,
	pub embedded_track: Option<i64>
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonSubtitlesDto {
	pub languages: Vec<String>,
	pub offers: Vec<SubtitleOffer>,
	/// Episodes that already have every language.
	pub covered: usize,
	pub notes: Vec<String>
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleDownload {
	pub file_id: i64,
	pub target_path: String,
	/// The episode the file belongs to (its video is the sync reference).
	pub episode_id: String,
	/// Set for an offer taken from the video: extracted, not downloaded.
	#[serde(default)]
	pub embedded_track: Option<i64>
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleDownloadResultDto {
	pub downloaded: usize,
	/// Taken from the video's own tracks: no download, no quota.
	pub extracted: usize,
	/// Downloads left today, as OpenSubtitles reports it.
	pub remaining: Option<i64>,
	pub errors: Vec<String>,
	pub sync: SyncSummary
}

/// Missing subtitles of one season, the whole show (`season_number` None) or
/// one episode.
pub fn search(
	state: &AppState,
	app: Option<AppHandle>,
	media_id: &str,
	season_number: Option<i32>,
	episode_id: Option<&str>
) -> Result<SeasonSubtitlesDto, String> {
	let settings = SettingsModule::subtitle_settings(state)?;
	// Both are needed only for languages the videos don't already carry.
	let api_key = settings.api_key.clone();
	let layout = load_item(state, media_id)?;
	let tmdb_id = layout.item.tmdb_id.clone();
	let languages = parse_languages(&settings.languages);
	let can_read_embedded = can_read_embedded();
	let client = http_client()?;
	let mut notes = Vec::new();

	let episodes = layout
		.videos(state)?
		.into_iter()
		.filter(|episode| season_number.is_none_or(|season| episode.season_number == Some(season)))
		.filter(|episode| episode_id.is_none_or(|id| episode.id == id))
		.collect::<Vec<_>>();

	let mut offers = Vec::new();
	let mut covered = 0;
	for (index, episode) in episodes.iter().enumerate() {
		let video = Path::new(&episode.media_path);
		let missing = languages.iter().filter(|language| !has_subtitle_in(video, language)).cloned().collect::<Vec<_>>();
		if missing.is_empty() {
			covered += 1;
			continue;
		}

		emit_step(&app, &format!("Procurando legendas {}/{}: {}", index + 1, episodes.len(), episode.title));

		// A language the video already carries needs no download: its own
		// track is exact, costs no quota and is in sync by definition.
		let tracks = if can_read_embedded { text_tracks(video).unwrap_or_default() } else { Vec::new() };
		let mut to_download = Vec::new();
		for language in &missing {
			match track_for_language(&tracks, language) {
				Some(track) => offers.push(SubtitleOffer {
					episode_id: episode.id.clone(),
					season_number: episode.season_number,
					episode_number: episode.episode_number,
					episode_title: episode.title.clone(),
					language: language.clone(),
					release: if track.title.is_empty() { "faixa do video".to_string() } else { track.title.clone() },
					file_id: 0,
					hash_match: true,
					download_count: 0,
					target_path: video.with_file_name(format!("{}.{}.srt", file_stem(&episode.media_path), language)).to_string_lossy().to_string(),
					source: "embedded".to_string(),
					embedded_track: Some(track.index)
				}),
				None => to_download.push(language.clone())
			}
		}
		if to_download.is_empty() {
			continue;
		}
		let (Some(api_key), Some(tmdb_id)) = (api_key.as_deref(), tmdb_id.as_deref()) else {
			let note = "Configure a OpenSubtitles API key nas configuracoes e rode Atualizar (TMDb ID) para baixar o que o video nao traz.".to_string();
			if !notes.contains(&note) {
				notes.push(note);
			}
			continue;
		};

		let mut query = vec![("languages", to_download.iter().map(|language| language.to_lowercase()).collect::<Vec<_>>().join(","))];
		match episode.season_number {
			// An episode is looked up through its show; a movie by its own id.
			Some(season) => {
				query.push(("parent_tmdb_id", tmdb_id.to_string()));
				query.push(("season_number", season.to_string()));
				if let Some(number) = episode.episode_number {
					query.push(("episode_number", number.to_string()));
				}
			}
			None => query.push(("tmdb_id", tmdb_id.to_string()))
		}
		if let Ok(hash) = movie_hash(video) {
			query.push(("moviehash", hash));
		}
		// The API redirects unless parameters are sorted.
		query.sort();

		let response = match request(client.get(format!("{}/subtitles", API_BASE)), api_key, None).query(&query).send().and_then(|response| response.error_for_status()) {
			Ok(response) => response.json::<Value>().unwrap_or(Value::Null),
			Err(error) => {
				notes.push(format!("{}: busca falhou ({})", episode.title, error));
				thread::sleep(SEARCH_PAUSE);
				continue;
			}
		};
		thread::sleep(SEARCH_PAUSE);

		for language in &to_download {
			// Exact-file match first, then human-made, then the most downloaded.
			let best = response["data"]
				.as_array()
				.into_iter()
				.flatten()
				.map(|entry| &entry["attributes"])
				.filter(|attributes| attributes["language"].as_str().is_some_and(|value| value.eq_ignore_ascii_case(language)))
				.filter(|attributes| attributes["files"][0]["file_id"].is_i64())
				.max_by_key(|attributes| {
					let machine = attributes["machine_translated"].as_bool().unwrap_or(false) || attributes["ai_translated"].as_bool().unwrap_or(false);
					(attributes["moviehash_match"].as_bool().unwrap_or(false), !machine, attributes["download_count"].as_i64().unwrap_or_default())
				});
			let Some(attributes) = best else { continue };

			let extension = attributes["files"][0]["file_name"]
				.as_str()
				.and_then(|name| Path::new(name).extension().and_then(|value| value.to_str()))
				.map(str::to_ascii_lowercase)
				.filter(|value| SUBTITLE_EXTENSIONS.contains(&value.as_str()))
				.unwrap_or_else(|| "srt".to_string());
			let target = video.with_file_name(format!("{}.{}.{}", file_stem(&episode.media_path), language, extension));

			offers.push(SubtitleOffer {
				episode_id: episode.id.clone(),
				season_number: episode.season_number,
				episode_number: episode.episode_number,
				episode_title: episode.title.clone(),
				language: language.clone(),
				release: attributes["release"].as_str().unwrap_or_default().to_string(),
				file_id: attributes["files"][0]["file_id"].as_i64().unwrap_or_default(),
				hash_match: attributes["moviehash_match"].as_bool().unwrap_or(false),
				download_count: attributes["download_count"].as_i64().unwrap_or_default(),
				target_path: target.to_string_lossy().to_string(),
				source: "opensubtitles".to_string(),
				embedded_track: None
			});
		}
	}

	emit_step(&app, "Busca de legendas concluida.");
	Ok(SeasonSubtitlesDto { languages, offers, covered, notes })
}

pub fn download(state: &AppState, app: Option<AppHandle>, media_id: &str, downloads: Vec<SubtitleDownload>) -> Result<SubtitleDownloadResultDto, String> {
	let settings = SettingsModule::subtitle_settings(state)?;
	let layout = load_item(state, media_id)?;
	let client = http_client()?;
	// Taking a track out of the video needs no account.
	let api_key = match downloads.iter().any(|item| item.embedded_track.is_none()) {
		true => Some(settings.api_key.clone().ok_or_else(|| "Configure a OpenSubtitles API key nas configuracoes.".to_string())?),
		false => None
	};
	let token = match api_key.as_deref() {
		Some(api_key) => login(&client, api_key, &settings)?,
		None => None
	};

	let mut downloaded = 0;
	let mut extracted = 0;
	let mut remaining = None;
	let mut errors = Vec::new();
	// Files written, per episode: each video is a sync reference once.
	let mut written = BTreeMap::<String, Vec<PathBuf>>::new();
	for (index, item) in downloads.iter().enumerate() {
		let target = PathBuf::from(&item.target_path);
		if !target.starts_with(&layout.folder) || !has_extension(&target, SUBTITLE_EXTENSIONS) {
			errors.push(format!("Destino invalido: {}", item.target_path));
			continue;
		}
		if target.exists() {
			errors.push(format!("Ja existe: {}", item.target_path));
			continue;
		}

		if let Some(track) = item.embedded_track {
			emit_step(&app, &format!("Extraindo legenda do video {}/{}...", index + 1, downloads.len()));
			let video = LibraryModule::get_media_item(state, &item.episode_id)?.map(|episode| PathBuf::from(episode.media_path));
			match video.ok_or_else(|| "episodio nao encontrado".to_string()).and_then(|video| extract_track(&video, track, &target)) {
				Ok(()) => extracted += 1,
				Err(error) => errors.push(format!("{}: {}", target.file_name().and_then(|name| name.to_str()).unwrap_or_default(), error))
			}
			continue;
		}

		let Some(api_key) = api_key.as_deref() else { continue };
		emit_step(&app, &format!("Baixando legenda {}/{}...", index + 1, downloads.len()));
		let link = request(client.post(format!("{}/download", API_BASE)), api_key, token.as_deref())
			.json(&json!({ "file_id": item.file_id }))
			.send()
			.map_err(|error| error.to_string())
			.and_then(|response| {
				let status = response.status();
				let body = response.json::<Value>().unwrap_or(Value::Null);
				if let Some(left) = body["remaining"].as_i64() {
					remaining = Some(left);
				}
				match body["link"].as_str() {
					Some(link) if status.is_success() => Ok(link.to_string()),
					_ => Err(body["message"].as_str().map(str::to_string).unwrap_or_else(|| format!("HTTP {}", status.as_u16())))
				}
			});
		let result = link.and_then(|link| {
			client
				.get(link)
				.header("User-Agent", USER_AGENT)
				.send()
				.and_then(|response| response.error_for_status())
				.and_then(|response| response.bytes())
				.map_err(|error| error.to_string())
		});

		match result.and_then(|bytes| fs::write(&target, &bytes).map_err(|error| error.to_string())) {
			Ok(()) => {
				downloaded += 1;
				written.entry(item.episode_id.clone()).or_default().push(target.clone());
			}
			Err(error) => {
				let quota = error.to_lowercase().contains("quota") || remaining == Some(0);
				errors.push(format!("{}: {}", target.file_name().and_then(|name| name.to_str()).unwrap_or_default(), error));
				if quota {
					errors.push("Cota diaria do OpenSubtitles esgotada.".to_string());
					break;
				}
			}
		}
	}

	// Releases are cut differently, so a download is often seconds off: align
	// it with the subtitle embedded in the video.
	let mut sync = SyncSummary::default();
	for (index, (episode_id, files)) in written.iter().enumerate() {
		emit_step(&app, &format!("Sincronizando legendas {}/{}...", index + 1, written.len()));
		match LibraryModule::get_media_item(state, episode_id)? {
			Some(episode) => sync_files(Path::new(&episode.media_path), files, None, &mut sync),
			None => sync.notes.push(format!("Episodio {} nao encontrado; legendas nao sincronizadas.", episode_id))
		}
	}

	if downloaded + extracted > 0 {
		emit_step(&app, "Reindexando...");
		layout.reindex(state, app.clone())?;
	}
	emit_step(&app, "Pronto.");

	Ok(SubtitleDownloadResultDto { downloaded, extracted, remaining, errors, sync })
}

/// Aligns the subtitle files already next to a season's (or the whole show's)
/// videos. Originals that change are kept in `_quarantine`.
pub fn sync_existing(state: &AppState, app: Option<AppHandle>, media_id: &str, season_number: Option<i32>) -> Result<SyncSummary, String> {
	let layout = load_item(state, media_id)?;
	let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|elapsed| elapsed.as_secs()).unwrap_or_default();
	let backup_root = layout.quarantine(&format!("sync-{}", stamp))?;

	let episodes = layout
		.videos(state)?
		.into_iter()
		.filter(|episode| season_number.is_none_or(|season| episode.season_number == Some(season)))
		.collect::<Vec<_>>();

	let mut summary = SyncSummary::default();
	for (index, episode) in episodes.iter().enumerate() {
		let video = Path::new(&episode.media_path);
		let files = list_video_subtitles(video)
			.into_iter()
			.map(|(path, _)| path)
			.filter(|path| has_extension(path, SYNCABLE_EXTENSIONS))
			.collect::<Vec<_>>();
		if files.is_empty() {
			continue;
		}

		emit_step(&app, &format!("Sincronizando {}/{}: {}", index + 1, episodes.len(), episode.title));
		let backup = video.parent().and_then(|folder| folder.strip_prefix(&layout.folder).ok()).map(|relative| backup_root.join(relative));
		sync_files(video, &files, backup.as_deref(), &mut summary);
	}

	emit_step(&app, "Pronto.");
	Ok(summary)
}

/// Without an account the download counts against the API key's anonymous quota.
fn login(client: &Client, api_key: &str, settings: &SubtitleSettings) -> Result<Option<String>, String> {
	let (Some(username), Some(password)) = (settings.username.as_deref(), settings.password.as_deref()) else {
		return Ok(None);
	};
	let body = request(client.post(format!("{}/login", API_BASE)), api_key, None)
		.json(&json!({ "username": username, "password": password }))
		.send()
		.and_then(|response| response.error_for_status())
		.and_then(|response| response.json::<Value>())
		.map_err(|error| format!("Login no OpenSubtitles falhou: {}", error))?;
	Ok(body["token"].as_str().map(str::to_string))
}

fn request(builder: RequestBuilder, api_key: &str, token: Option<&str>) -> RequestBuilder {
	let builder = builder.header("Api-Key", api_key).header("User-Agent", USER_AGENT).header("Accept", "application/json");
	match token {
		Some(token) => builder.bearer_auth(token),
		None => builder
	}
}

/// "pt-BR, en" -> ["pt-BR", "en"].
fn parse_languages(raw: &str) -> Vec<String> {
	raw.split(',').map(str::trim).filter(|language| !language.is_empty()).map(str::to_string).collect()
}

/// A subtitle tagged with this language next to the video ("pt-BR" also
/// accepts "pt-br"; "en" accepts "en.sdh").
fn has_subtitle_in(video: &Path, language: &str) -> bool {
	let stem = file_stem(&video.to_string_lossy());
	let prefix = format!("{}.{}", stem, language).to_lowercase();
	fs::read_dir(video.parent().unwrap_or(Path::new("")))
		.map(|entries| {
			entries.filter_map(Result::ok).map(|entry| entry.path()).any(|path| {
				let subtitle_stem = path.file_stem().and_then(|value| value.to_str()).unwrap_or_default().to_lowercase();
				has_extension(&path, SUBTITLE_EXTENSIONS) && (subtitle_stem == prefix || subtitle_stem.starts_with(&format!("{}.", prefix)))
			})
		})
		.unwrap_or(false)
}

/// OpenSubtitles hash: file size plus the 64-bit little-endian word sums of
/// the first and last 64 KiB.
fn movie_hash(path: &Path) -> Result<String, String> {
	const CHUNK: u64 = 65536;
	let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
	let size = file.metadata().map_err(|error| error.to_string())?.len();
	if size < CHUNK * 2 {
		return Err("arquivo pequeno demais".to_string());
	}

	let mut hash = size;
	let mut buffer = vec![0u8; CHUNK as usize];
	for offset in [0, size - CHUNK] {
		file.seek(SeekFrom::Start(offset)).map_err(|error| error.to_string())?;
		file.read_exact(&mut buffer).map_err(|error| error.to_string())?;
		for word in buffer.chunks_exact(8) {
			hash = hash.wrapping_add(u64::from_le_bytes(word.try_into().unwrap_or_default()));
		}
	}
	Ok(format!("{:016x}", hash))
}
