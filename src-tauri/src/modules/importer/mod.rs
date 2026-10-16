//! Adds one dropped folder to the library without rescanning the drive:
//! identify what it is (Claude agent, or a heuristic without an Anthropic
//! key), show a dry-run plan of every file copy, then copy into the library
//! layout with formatted names, fetch TMDb metadata and index just that item.

mod agent;

use crate::app::state::AppState;
use crate::infrastructure::db::MediaItemRecord;
use crate::modules::library::LibraryModule;
use crate::modules::metadata::MetadataModule;
use crate::modules::refresh::{http_client, image_options, tmdb_get, ImageOption};
use crate::modules::scanner::{
	has_extension, is_video_file, parse_episode_number, parse_media_name, parse_season_number, ScannerModule,
	SUBTITLE_EXTENSIONS
};
use crate::modules::settings::SettingsModule;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;

pub struct ImporterModule;

const IMPORT_PROGRESS_EVENT: &str = "library://import-progress";
pub const IMPORT_CATEGORIES: &[&str] = &["movie", "show", "documentary", "standup", "concert"];

/// What the dropped folder is. The agent (or heuristic) proposes it; the user
/// can edit it in the dry run; the plan and the copy are derived from it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportIdentity {
	/// One folder, or one or more loose files.
	pub source_paths: Vec<String>,
	pub category: String,
	pub title: String,
	pub year: Option<i32>,
	pub tmdb_id: Option<String>,
	/// Set when the folder holds new episodes of a show already in the library.
	pub existing_show_id: Option<String>,
	/// Season for episode files whose name and folder don't say it.
	pub default_season: Option<u32>,
	pub library_root: Option<String>,
	/// Artwork picked in the dry run (TMDb URLs); none keeps TMDb's default poster.
	#[serde(default)]
	pub poster_url: Option<String>,
	#[serde(default)]
	pub cover_url: Option<String>
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFileOp {
	pub source: String,
	pub target: String,
	pub size_bytes: u64,
	/// "copy", "skip" (identical file already there) or "conflict".
	pub action: String
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistingShowDto {
	pub id: String,
	pub title: String,
	pub media_path: String,
	pub tmdb_id: Option<String>,
	pub seasons: Vec<i32>
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPlanDto {
	pub identity: ImportIdentity,
	pub tmdb_title: Option<String>,
	pub poster_url: Option<String>,
	pub overview: Option<String>,
	pub existing_show: Option<ExistingShowDto>,
	/// Choices for a new item's artwork (an existing show keeps its own).
	pub posters: Vec<ImageOption>,
	pub backdrops: Vec<ImageOption>,
	pub library_roots: Vec<String>,
	pub target_folder: String,
	pub files: Vec<ImportFileOp>,
	pub total_bytes: u64,
	pub notes: Vec<String>,
	pub identified_by: String
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportProgressPayload {
	/// "step" lines form the log shown while identifying; "copy" updates the bar.
	kind: &'static str,
	message: String,
	copied_bytes: u64,
	total_bytes: u64
}

impl ImporterModule {
	/// Identifies the folder and returns the dry-run plan. `category` forces the
	/// category (the user picked one); otherwise it is detected.
	pub fn preview(
		state: &AppState,
		app: Option<AppHandle>,
		source_paths: &[String],
		category: Option<String>
	) -> Result<ImportPlanDto, String> {
		let sources = source_paths.iter().map(PathBuf::from).collect::<Vec<_>>();
		if sources.is_empty() {
			return Err("Nada para adicionar.".to_string());
		}
		if let Some(missing) = sources.iter().find(|source| !source.exists()) {
			return Err(format!("{} nao existe.", missing.display()));
		}

		let category = category.filter(|value| IMPORT_CATEGORIES.contains(&value.as_str()));
		let report = |message: String| emit_step(&app, message);
		let keys = SettingsModule::metadata_api_keys(state)?;
		// With Claude in the loop it does the TMDb search itself; the guess
		// only needs the folder name.
		let heuristic = heuristic_identity(state, &sources, category.as_deref(), keys.anthropic_api_key.is_none(), &report);
		let mut notes = Vec::new();

		let (identity, identified_by) = match keys.anthropic_api_key.as_deref() {
			Some(anthropic_key) => {
				report("Perguntando ao Claude...".to_string());
				match agent::identify(state, anthropic_key, keys.tmdb_api_key.as_deref(), &sources, category.as_deref(), &heuristic, &report) {
					Ok((identity, agent_notes)) => {
						notes.extend(agent_notes);
						(identity, "claude")
					}
					Err(error) => {
						report(format!("Claude falhou: {}. Usando o palpite pelo nome.", error));
						notes.push(format!("Agente falhou ({}); usando deteccao local.", error));
						(heuristic, "heuristic")
					}
				}
			}
			None => {
				report("Sem Anthropic API key; usando o palpite pelo nome.".to_string());
				notes.push("Sem Anthropic API key: identificacao feita so pelo nome da pasta.".to_string());
				(heuristic, "heuristic")
			}
		};

		report("Montando o plano de copia...".to_string());
		let mut plan = Self::build_plan(state, identity)?;
		plan.identified_by = identified_by.to_string();
		notes.append(&mut plan.notes);
		plan.notes = notes;
		Ok(plan)
	}

	/// Derives every file copy from an identity. Deterministic and offline
	/// except for TMDb lookups (poster preview and episode names).
	pub fn build_plan(state: &AppState, mut identity: ImportIdentity) -> Result<ImportPlanDto, String> {
		let sources = identity.source_paths.iter().map(PathBuf::from).collect::<Vec<_>>();
		let library_roots = SettingsModule::library_root_paths(state)?;
		let tmdb_key = SettingsModule::metadata_api_keys(state)?.tmdb_api_key;
		let mut notes = Vec::new();

		if !IMPORT_CATEGORIES.contains(&identity.category.as_str()) {
			return Err(format!("Categoria invalida: {}", identity.category));
		}

		if identity.category != "show" {
			identity.existing_show_id = None;
		}

		let existing_show = match identity.existing_show_id.as_deref() {
			Some(show_id) => Some(existing_show_dto(state, show_id)?.ok_or_else(|| "Serie existente nao encontrada no banco.".to_string())?),
			None => None
		};

		// Existing shows stay where they are; new items go to the chosen root.
		let library_root = match &existing_show {
			Some(show) => Path::new(&show.media_path)
				.parent()
				.and_then(Path::parent)
				.map(|root| root.to_string_lossy().to_string())
				.ok_or_else(|| "Nao foi possivel resolver a raiz da serie.".to_string())?,
			None => identity
				.library_root
				.clone()
				.filter(|root| library_roots.contains(root))
				.or_else(|| library_roots.first().cloned())
				.ok_or_else(|| "Configure uma pasta da biblioteca antes de adicionar itens.".to_string())?
		};
		identity.library_root = Some(library_root.clone());
		if identity.tmdb_id.is_none() {
			identity.tmdb_id = existing_show.as_ref().and_then(|show| show.tmdb_id.clone());
		}

		let formatted_name = format_item_name(&identity.title, identity.year);
		if formatted_name.is_empty() {
			return Err("Titulo vazio.".to_string());
		}

		let target_folder = match &existing_show {
			Some(show) => PathBuf::from(&show.media_path),
			None => category_root(Path::new(&library_root), &identity.category).join(&formatted_name)
		};

		let mut tmdb_title = None;
		let mut poster_url = None;
		let mut overview = None;
		let mut posters = Vec::new();
		let mut backdrops = Vec::new();
		if let (Some(key), Some(tmdb_id)) = (tmdb_key.as_deref(), identity.tmdb_id.as_deref()) {
			match MetadataModule::tmdb_details(key, tmdb_kind(&identity.category), tmdb_id) {
				Ok(details) => {
					tmdb_title = Some(details.title);
					poster_url = details.poster_url;
					overview = details.overview;
				}
				Err(error) => notes.push(format!("TMDb {}: {}", tmdb_id, error))
			}

			if existing_show.is_none() {
				let images = http_client().and_then(|client| {
					tmdb_get(&client, key, &format!("{}/{}/images", tmdb_kind(&identity.category), tmdb_id), &[("include_image_language", "en,pt,null")])
				});
				match images {
					Ok(images) => {
						// Same order as "Atualizar": titled posters, textless backdrops.
						let wrapped = serde_json::json!({ "images": images });
						posters = image_options(&wrapped, "/images/posters", &["en", "pt"], "w342", "w780");
						backdrops = image_options(&wrapped, "/images/backdrops", &["null"], "w780", "original");
					}
					Err(error) => notes.push(format!("Imagens do TMDb: {}", error))
				}
			}
		}

		let (videos, subtitles) = collect_source_files(&sources);
		if videos.is_empty() {
			return Err("Nenhum arquivo de video encontrado.".to_string());
		}

		let mut targets = Vec::<(PathBuf, PathBuf)>::new();

		if identity.category == "show" {
			let mut episode_names = HashMap::<u32, HashMap<u32, String>>::new();

			for video in &videos {
				let stem = file_stem(video);
				let (season, episode) = episode_position(video, &sources, identity.default_season);
				let extension = extension_of(video);
				let season_folder = target_folder.join(format!("Season {:02}", season));

				let file_name = match episode {
					Some(episode) => {
						let names = episode_names.entry(season).or_insert_with(|| {
							match (tmdb_key.as_deref(), identity.tmdb_id.as_deref()) {
								(Some(key), Some(tmdb_id)) => MetadataModule::tmdb_season_episode_names(key, tmdb_id, season).into_iter().collect(),
								_ => HashMap::new()
							}
						});
						match names.get(&episode).map(|name| sanitize_file_name(name)).filter(|name| !name.is_empty()) {
							Some(name) => format!("S{:02}E{:02} - {}.{}", season, episode, name, extension),
							None => format!("S{:02}E{:02}.{}", season, episode, extension)
						}
					}
					None => {
						notes.push(format!("Sem numero de episodio em \"{}\"; nome original mantido.", stem));
						format!("{}.{}", sanitize_file_name(&stem), extension)
					}
				};

				targets.push((video.clone(), season_folder.join(file_name)));
			}

			if let Some(show) = &existing_show {
				let mut new_seasons = targets
					.iter()
					.filter_map(|(_, target)| target.parent().and_then(|folder| folder.file_name()).and_then(|name| name.to_str()).and_then(parse_season_number))
					.map(|season| season as i32)
					.filter(|season| !show.seasons.contains(season))
					.collect::<Vec<_>>();
				new_seasons.sort();
				new_seasons.dedup();
				if !new_seasons.is_empty() {
					notes.push(format!("Nova(s) temporada(s) de {}: {:?}", show.title, new_seasons));
				}
			}
		} else {
			for (index, video) in videos.iter().enumerate() {
				let name = if videos.len() == 1 {
					formatted_name.clone()
				} else {
					format!("{} - Part {}", formatted_name, index + 1)
				};
				targets.push((video.clone(), target_folder.join(format!("{}.{}", name, extension_of(video)))));
			}

			if videos.len() > 1 {
				notes.push(format!("{} videos na pasta; cada um vira um item \"Part N\".", videos.len()));
			}
		}

		// Subtitles follow the video with the same stem, or the same episode, or
		// the only video. Unmatched ones are left behind.
		let video_targets = targets.clone();
		let mut used_subtitle_targets = Vec::<PathBuf>::new();
		for subtitle in &subtitles {
			let subtitle_stem = file_stem(subtitle).to_lowercase();
			let subtitle_episode = episode_token(&subtitle_stem);
			let matched = video_targets
				.iter()
				.find(|(video, _)| subtitle_stem.starts_with(&file_stem(video).to_lowercase()))
				.or_else(|| {
					subtitle_episode.and_then(|token| {
						video_targets
							.iter()
							.find(|(video, _)| episode_token(&file_stem(video).to_lowercase()) == Some(token))
					})
				})
				.or_else(|| (video_targets.len() == 1).then(|| &video_targets[0]));

			let Some((_, video_target)) = matched else {
				notes.push(format!("Legenda sem video correspondente ignorada: {}", subtitle.display()));
				continue;
			};

			let base = video_target.with_extension("");
			let language = subtitle_language(subtitle);
			let extension = extension_of(subtitle);
			let mut candidate = PathBuf::from(format!("{}{}.{}", base.display(), language, extension));
			let mut counter = 2;
			while used_subtitle_targets.contains(&candidate) {
				candidate = PathBuf::from(format!("{}{}.{}.{}", base.display(), language, counter, extension));
				counter += 1;
			}
			used_subtitle_targets.push(candidate.clone());
			targets.push((subtitle.clone(), candidate));
		}

		let mut files = Vec::new();
		let mut total_bytes = 0;
		for (source_file, target_file) in targets {
			let size_bytes = fs::metadata(&source_file).map(|metadata| metadata.len()).unwrap_or_default();
			let action = match fs::metadata(&target_file) {
				Ok(existing) if existing.len() == size_bytes => "skip",
				Ok(_) => "conflict",
				Err(_) => "copy"
			};
			if action == "copy" {
				total_bytes += size_bytes;
			}
			if action == "conflict" {
				notes.push(format!("Ja existe um arquivo diferente em {}", target_file.display()));
			}
			files.push(ImportFileOp {
				source: source_file.to_string_lossy().to_string(),
				target: target_file.to_string_lossy().to_string(),
				size_bytes,
				action: action.to_string()
			});
		}

		if identity.tmdb_id.is_none() && existing_show.is_none() {
			notes.push("Sem TMDb ID: o item entra sem metadata remota.".to_string());
		}

		Ok(ImportPlanDto {
			identity,
			tmdb_title,
			poster_url,
			overview,
			existing_show,
			posters,
			backdrops,
			library_roots,
			target_folder: target_folder.to_string_lossy().to_string(),
			files,
			total_bytes,
			notes,
			identified_by: "manual".to_string()
		})
	}

	/// Copies the files, writes the TMDb identity, fetches metadata and indexes
	/// only this item. Returns the item's folder.
	pub fn apply(state: &AppState, app: Option<AppHandle>, identity: ImportIdentity) -> Result<String, String> {
		let plan = Self::build_plan(state, identity)?;

		if let Some(conflict) = plan.files.iter().find(|file| file.action == "conflict") {
			return Err(format!("Conflito: {} ja existe com outro tamanho.", conflict.target));
		}

		let mut copied_bytes = 0;
		for file in plan.files.iter().filter(|file| file.action == "copy") {
			let target = Path::new(&file.target);
			let file_name = target.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_string();
			if let Some(parent) = target.parent() {
				fs::create_dir_all(parent).map_err(|error| format!("Nao foi possivel criar {}: {}", parent.display(), error))?;
			}

			// Copy to a temp name first so an interrupted copy never looks like a
			// finished file (and gets skipped as "identical" next time).
			let partial = target.with_extension(format!("{}.partial", extension_of(target)));
			let mut reader = fs::File::open(&file.source).map_err(|error| format!("{}: {}", file.source, error))?;
			let mut writer = fs::File::create(&partial).map_err(|error| format!("{}: {}", partial.display(), error))?;
			let mut buffer = vec![0u8; 8 * 1024 * 1024];
			loop {
				let read = reader.read(&mut buffer).map_err(|error| format!("{}: {}", file.source, error))?;
				if read == 0 {
					break;
				}
				writer.write_all(&buffer[..read]).map_err(|error| format!("{}: {}", partial.display(), error))?;
				copied_bytes += read as u64;
				emit_progress(&app, &format!("Copiando {}", file_name), copied_bytes, plan.total_bytes);
			}
			writer.flush().map_err(|error| error.to_string())?;
			drop(writer);
			fs::rename(&partial, target).map_err(|error| format!("{}: {}", target.display(), error))?;
		}

		let identity = &plan.identity;
		let target_folder = PathBuf::from(&plan.target_folder);
		let library_root = PathBuf::from(identity.library_root.as_deref().unwrap_or_default());
		let has_new_identity = plan.existing_show.is_none() && identity.tmdb_id.is_some();

		if plan.existing_show.is_none() {
			MetadataModule::write_identity_seed(&target_folder, identity.year, identity.tmdb_id.as_deref())?;

			// Picked artwork lands before indexing, so the scan finds it and
			// doesn't fetch TMDb's default poster over it.
			for (url, name) in [(&identity.poster_url, "poster"), (&identity.cover_url, "cover")] {
				let Some(url) = url else { continue };
				emit_step(&app, format!("Baixando {}...", if name == "poster" { "poster" } else { "capa" }));
				let bytes = http_client()?
					.get(url)
					.send()
					.and_then(|response| response.error_for_status())
					.and_then(|response| response.bytes())
					.map_err(|error| format!("Download de {} falhou: {}", name, error))?;
				let extension = Path::new(url).extension().and_then(|value| value.to_str()).unwrap_or("jpg").to_ascii_lowercase();
				fs::write(target_folder.join(format!("{}.{}", name, extension)), &bytes).map_err(|error| error.to_string())?;
				MetadataModule::write_local_metadata_fields(&target_folder, &[(name, Some(serde_json::Value::String(format!("{}.{}", name, extension))))])?;
			}
		}

		emit_step(&app, "Buscando metadata e indexando...".to_string());
		ScannerModule::index_item(
			state,
			app.clone(),
			&category_root(&library_root, &identity.category),
			&target_folder,
			&identity.category,
			has_new_identity
		)?;
		emit_step(&app, "Pronto.".to_string());

		Ok(plan.target_folder)
	}
}

/// Library items the agent can compare against (no episodes, no music).
pub(crate) fn library_video_items(state: &AppState) -> Result<Vec<MediaItemRecord>, String> {
	Ok(LibraryModule::list_media(state)?
		.into_iter()
		.filter(|item| IMPORT_CATEGORIES.contains(&item.media_type.as_str()))
		.collect())
}

pub(crate) fn existing_show_dto(state: &AppState, show_id: &str) -> Result<Option<ExistingShowDto>, String> {
	let Some(show) = LibraryModule::get_media_item(state, show_id)?.filter(|item| item.media_type == "show") else {
		return Ok(None);
	};
	let mut seasons = LibraryModule::list_show_episodes(state, show_id)?
		.into_iter()
		.filter_map(|episode| episode.season_number)
		.collect::<Vec<_>>();
	seasons.sort();
	seasons.dedup();

	Ok(Some(ExistingShowDto {
		id: show.id,
		title: show.title,
		media_path: show.media_path,
		tmdb_id: show.tmdb_id,
		seasons
	}))
}

/// Folder-name guess used without an Anthropic key, and as a hint to the agent.
fn heuristic_identity(
	state: &AppState,
	sources: &[PathBuf],
	forced_category: Option<&str>,
	search_tmdb: bool,
	report: &dyn Fn(String)
) -> ImportIdentity {
	report("Lendo arquivos...".to_string());
	let (videos, subtitles) = collect_source_files(sources);
	report(format!("{} video(s) e {} legenda(s) encontrados.", videos.len(), subtitles.len()));
	// Several loose files: name them after the first video ("Dark.S02E01" -> "Dark").
	let folder_name = match (sources, videos.first()) {
		([source], _) => item_name(source),
		(_, Some(video)) => file_stem(video),
		_ => String::new()
	};
	let looks_like_show = videos.iter().any(|video| episode_token(&file_stem(video).to_lowercase()).is_some())
		|| parse_season_number(&folder_name).is_some();
	let category = forced_category
		.map(str::to_string)
		.unwrap_or_else(|| if looks_like_show { "show" } else { "movie" }.to_string());

	// "Show Name S02" / "Show Name Season 2" -> title "Show Name", season 2.
	let default_season = parse_season_number(&folder_name).filter(|season| *season > 0);
	let title_source = folder_name
		.split(|character: char| character == '.' || character == '_' || character.is_whitespace())
		.take_while(|word| {
			let lower = word.to_ascii_lowercase();
			!(lower == "season" || lower == "temporada" || episode_token(&lower).is_some() || parse_season_number(&lower).is_some())
		})
		.collect::<Vec<_>>()
		.join(" ");
	let (title, year) = parse_media_name(if title_source.trim().is_empty() { &folder_name } else { &title_source });

	let existing_show = (category == "show")
		.then(|| library_video_items(state).ok())
		.flatten()
		.and_then(|items| {
			let wanted = normalize_for_match(&title);
			items
				.into_iter()
				.find(|item| item.media_type == "show" && normalize_for_match(&item.title) == wanted)
		});
	report(format!(
		"Palpite pelo nome: {} ({}), {}.",
		title,
		year.map(|year| year.to_string()).unwrap_or("sem ano".to_string()),
		category
	));

	// A show already in the library has its TMDb id; no need to search.
	let tmdb_id = match &existing_show {
		Some(show) => {
			report(format!("Serie ja existe na biblioteca: {}.", show.title));
			show.tmdb_id.clone()
		}
		None if search_tmdb => SettingsModule::metadata_api_keys(state)
			.ok()
			.and_then(|keys| keys.tmdb_api_key)
			.and_then(|key| {
				report(format!("Buscando \"{}\" no TMDb...", title));
				MetadataModule::tmdb_search(&key, tmdb_kind(&category), &title, year)
					.map_err(|error| report(format!("TMDb falhou: {}", error)))
					.ok()
			})
			.and_then(|results| {
				results
					.iter()
					.find(|result| year.is_some() && result.year == year)
					.or(results.first())
					.map(|result| result.tmdb_id.clone())
			}),
		None => None
	};
	let existing_show_id = existing_show.map(|show| show.id);

	ImportIdentity {
		source_paths: sources.iter().map(|source| source.to_string_lossy().to_string()).collect(),
		category,
		title,
		year,
		tmdb_id,
		existing_show_id,
		default_season,
		library_root: None,
		poster_url: None,
		cover_url: None
	}
}

fn collect_source_files(sources: &[PathBuf]) -> (Vec<PathBuf>, Vec<PathBuf>) {
	let mut videos = Vec::new();
	let mut subtitles = Vec::new();

	for entry in sources.iter().flat_map(|source| WalkDir::new(source).follow_links(true)).filter_map(Result::ok) {
		let path = entry.path();
		if !entry.file_type().is_file() {
			continue;
		}
		let is_sample = file_stem(path).to_lowercase().contains("sample");
		if is_video_file(path) && !is_sample {
			videos.push(path.to_path_buf());
		} else if has_extension(path, SUBTITLE_EXTENSIONS) {
			subtitles.push(path.to_path_buf());
		}
	}

	videos.sort();
	videos.dedup();
	subtitles.sort();
	subtitles.dedup();
	(videos, subtitles)
}

/// Season comes from an SxxEyy token, then the parent folder, then the
/// identity default; episode from the file name.
fn episode_position(video: &Path, sources: &[PathBuf], default_season: Option<u32>) -> (u32, Option<u32>) {
	let stem = file_stem(video).to_lowercase();
	if let Some((season, episode)) = episode_token(&stem) {
		return (season, Some(episode));
	}

	let folder_season = video
		.parent()
		// A dropped folder only gives the season if it is named like one.
		.filter(|parent| !sources.iter().any(|source| source == parent) || parse_season_number(&item_name(parent)).is_some())
		.and_then(|parent| parent.file_name())
		.and_then(|name| name.to_str())
		.and_then(parse_season_number);

	(folder_season.or(default_season).unwrap_or(1), parse_episode_number(&stem))
}

/// Finds "s01e02" or "1x02" anywhere in a lowercase name.
fn episode_token(lower_name: &str) -> Option<(u32, u32)> {
	lower_name
		.split(|character: char| !character.is_ascii_alphanumeric())
		.find_map(|word| {
			if let Some((season, episode)) = word.strip_prefix('s').and_then(|rest| rest.split_once('e')) {
				let episode = episode.trim_end_matches(|character: char| !character.is_ascii_digit());
				return Some((season.parse().ok()?, episode.parse().ok()?));
			}
			let (season, episode) = word.split_once('x')?;
			(season.len() <= 2).then_some(())?;
			Some((season.parse().ok()?, episode.parse().ok()?))
		})
}

/// Keeps a language tag such as "movie.pt-BR.srt" -> ".pt-BR".
fn subtitle_language(subtitle: &Path) -> String {
	let stem = file_stem(subtitle);
	stem.rsplit_once('.')
		.map(|(_, tag)| tag)
		.filter(|tag| (2..=6).contains(&tag.len()) && tag.chars().all(|character| character.is_ascii_alphabetic() || character == '-'))
		.map(|tag| format!(".{}", tag))
		.unwrap_or_default()
}

pub(crate) fn category_root(library_root: &Path, category: &str) -> PathBuf {
	let folder = match category {
		"movie" => "movies",
		"show" => "shows",
		"standup" => "standup",
		"concert" => "concerts",
		_ if library_root.join("documentaries").is_dir() && !library_root.join("documentary").is_dir() => "documentaries",
		_ => "documentary"
	};
	library_root.join(folder)
}

pub(crate) fn tmdb_kind(category: &str) -> &'static str {
	if category == "show" { "tv" } else { "movie" }
}

fn format_item_name(title: &str, year: Option<i32>) -> String {
	let title = sanitize_file_name(title);
	match year {
		Some(year) if !title.is_empty() => format!("{} ({})", title, year),
		_ => title
	}
}

/// Library contract: ": " becomes " - ", and characters Windows rejects are dropped.
pub(crate) fn sanitize_file_name(raw: &str) -> String {
	raw.replace(": ", " - ")
		.chars()
		.map(|character| match character {
			':' => '-',
			'<' | '>' | '"' | '/' | '\\' | '|' | '?' | '*' => ' ',
			_ => character
		})
		.collect::<String>()
		.split_whitespace()
		.collect::<Vec<_>>()
		.join(" ")
		.trim_end_matches('.')
		.trim()
		.to_string()
}

pub(crate) fn normalize_for_match(raw: &str) -> String {
	raw.to_lowercase()
		.chars()
		.map(|character| if character.is_alphanumeric() { character } else { ' ' })
		.collect::<String>()
		.split_whitespace()
		.collect::<Vec<_>>()
		.join(" ")
}

/// Folder name, or the stem for a single dropped file ("Movie.2010.1080p" must
/// keep its dots as a folder name).
pub(crate) fn item_name(path: &Path) -> String {
	if path.is_dir() {
		path.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_string()
	} else {
		file_stem(path)
	}
}

fn file_stem(path: &Path) -> String {
	path.file_stem().and_then(|stem| stem.to_str()).unwrap_or_default().to_string()
}

fn extension_of(path: &Path) -> String {
	path.extension().and_then(|extension| extension.to_str()).unwrap_or_default().to_lowercase()
}

/// A step of the import, shown in the dialog's log and printed to the terminal
/// running the app.
fn emit_step(app: &Option<AppHandle>, message: String) {
	eprintln!("[import] {}", message);
	if let Some(app) = app {
		let _ = app.emit(
			IMPORT_PROGRESS_EVENT,
			ImportProgressPayload {
				kind: "step",
				message,
				copied_bytes: 0,
				total_bytes: 0
			}
		);
	}
}

fn emit_progress(app: &Option<AppHandle>, message: &str, copied_bytes: u64, total_bytes: u64) {
	if let Some(app) = app {
		let _ = app.emit(
			IMPORT_PROGRESS_EVENT,
			ImportProgressPayload {
				kind: "copy",
				message: message.to_string(),
				copied_bytes,
				total_bytes
			}
		);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::infrastructure::db::DatabaseInfrastructure;

	fn touch(path: &Path, bytes: usize) {
		fs::create_dir_all(path.parent().unwrap()).unwrap();
		fs::write(path, vec![0u8; bytes]).unwrap();
	}

	#[test]
	fn parses_episode_tokens() {
		assert_eq!(episode_token("show.name.s02e05.1080p"), Some((2, 5)));
		assert_eq!(episode_token("show 1x03 title"), Some((1, 3)));
		assert_eq!(episode_token("movie 2010 1080p"), None);
		assert_eq!(subtitle_language(Path::new("a.pt-BR.srt")), ".pt-BR");
		assert_eq!(subtitle_language(Path::new("a b.srt")), "");
		assert_eq!(format_item_name("Mission: Impossible", Some(1996)), "Mission - Impossible (1996)");
	}

	#[test]
	fn plans_copies_and_indexes_movie_and_new_season() {
		let base = std::env::temp_dir().join(format!("netcrico-import-test-{}", std::process::id()));
		let _ = fs::remove_dir_all(&base);
		let library = base.join("library");
		fs::create_dir_all(library.join("movies")).unwrap();
		touch(&library.join("shows/Dark (2017)/Season 01/S01E01.mkv"), 10);

		let database = DatabaseInfrastructure::new(base.join("appdata")).unwrap();
		database.initialize().unwrap();
		let state = AppState::new(database);
		SettingsModule::save_library_root_paths(&state, &[library.to_string_lossy().to_string()]).unwrap();
		ScannerModule::rescan_library(&state, None).unwrap();

		// Movie with release-name noise and a subtitle.
		let movie_source = base.join("downloads/The.Matrix.1999.1080p.BluRay.x264");
		touch(&movie_source.join("The.Matrix.1999.1080p.BluRay.x264.mkv"), 100);
		touch(&movie_source.join("The.Matrix.1999.1080p.BluRay.x264.en.srt"), 5);
		touch(&movie_source.join("sample.mkv"), 1);

		let plan = ImporterModule::preview(&state, None, &[movie_source.to_string_lossy().to_string()], None).unwrap();
		assert_eq!(plan.identity.category, "movie");
		assert_eq!(plan.identity.title, "The Matrix");
		assert_eq!(plan.identity.year, Some(1999));
		let targets = plan.files.iter().map(|file| file.target.clone()).collect::<Vec<_>>();
		let movie_folder = library.join("movies/The Matrix (1999)");
		assert_eq!(targets, vec![
			movie_folder.join("The Matrix (1999).mkv").to_string_lossy().to_string(),
			movie_folder.join("The Matrix (1999).en.srt").to_string_lossy().to_string()
		]);
		ImporterModule::apply(&state, None, plan.identity.clone()).unwrap();
		assert!(movie_folder.join("The Matrix (1999).mkv").exists());
		assert!(movie_source.join("The.Matrix.1999.1080p.BluRay.x264.mkv").exists());

		// New season of a show already in the library.
		let season_source = base.join("downloads/Dark.S02.1080p");
		touch(&season_source.join("Dark.S02E01.1080p.mkv"), 20);
		touch(&season_source.join("Dark.S02E02.1080p.mkv"), 20);

		let plan = ImporterModule::preview(&state, None, &[season_source.to_string_lossy().to_string()], None).unwrap();
		assert_eq!(plan.identity.category, "show");
		let show = plan.existing_show.clone().expect("matches the existing show");
		assert_eq!(show.seasons, vec![1]);
		assert_eq!(plan.files[0].target, library.join("shows/Dark (2017)/Season 02/S02E01.mkv").to_string_lossy());
		ImporterModule::apply(&state, None, plan.identity.clone()).unwrap();

		let items = LibraryModule::list_media(&state).unwrap();
		assert!(items.iter().any(|item| item.title == "The Matrix" && item.year == Some(1999)));
		assert_eq!(existing_show_dto(&state, &show.id).unwrap().unwrap().seasons, vec![1, 2]);

		// Loose episode files of the same show, dropped without a folder.
		let loose = base.join("downloads/loose");
		touch(&loose.join("Dark.S03E01.mkv"), 30);
		touch(&loose.join("Dark.S03E01.pt-BR.srt"), 3);
		let loose_plan = ImporterModule::preview(
			&state,
			None,
			&[loose.join("Dark.S03E01.mkv").to_string_lossy().to_string(), loose.join("Dark.S03E01.pt-BR.srt").to_string_lossy().to_string()],
			None
		)
		.unwrap();
		assert_eq!(loose_plan.identity.title, "Dark");
		assert_eq!(loose_plan.existing_show.as_ref().map(|show| show.id.clone()), Some(show.id.clone()));
		assert_eq!(loose_plan.files[1].target, library.join("shows/Dark (2017)/Season 03/S03E01.pt-BR.srt").to_string_lossy());

		// Re-planning the same folder now skips identical files.
		let plan = ImporterModule::build_plan(&state, plan.identity).unwrap();
		assert!(plan.files.iter().all(|file| file.action == "skip"));

		fs::remove_dir_all(&base).unwrap();
	}

	#[test]
	fn scans_specials_and_extras_as_separate_sections() {
		let base = std::env::temp_dir().join(format!("netcrico-contract-test-{}", std::process::id()));
		let _ = fs::remove_dir_all(&base);
		let show = base.join("library/shows/South Park (1997)");
		touch(&show.join("Season 01/S01E01 - Cartman Gets an Anal Probe.mp4"), 1);
		touch(&show.join("Season 14/S14E13 - Coon vs. Coon & Friends.mp4"), 1);
		touch(&show.join("Specials/S00E01 - The Spirit of Christmas (Jesus vs. Frosty).mp4"), 1);
		touch(&show.join("Specials/Special - The End of Obesity (2024).mkv"), 1);
		touch(&show.join("Specials/Special - Post COVID (2021).mkv"), 1);
		touch(&show.join("Extras/S14E05 - 200 (Uncensored).avi"), 1);
		touch(&show.join("Extras/S12 - Six Days to South Park.mp4"), 1);
		fs::write(
			show.join(".local-cinema.json"),
			r#"{ "specials": { "Special - Post COVID (2021).mkv": { "title": "Post COVID", "tmdbMovieId": "874299" } } }"#
		)
		.unwrap();

		let database = DatabaseInfrastructure::new(base.join("appdata")).unwrap();
		database.initialize().unwrap();
		let state = AppState::new(database);
		SettingsModule::save_library_root_paths(&state, &[base.join("library").to_string_lossy().to_string()]).unwrap();
		ScannerModule::rescan_library(&state, None).unwrap();

		let episodes = LibraryModule::list_show_episodes(&state, &show.to_string_lossy())
			.unwrap()
			.into_iter()
			.map(|episode| (episode.season_number.unwrap(), episode.episode_number, episode.title))
			.collect::<Vec<_>>();
		let has = |season: i32, episode: Option<i32>, title: &str| episodes.contains(&(season, episode, title.to_string()));

		assert!(has(1, Some(1), "Cartman Gets an Anal Probe"), "{:?}", episodes);
		assert!(has(14, Some(13), "Coon vs. Coon & Friends"), "{:?}", episodes);
		assert!(has(0, Some(1), "The Spirit of Christmas (Jesus vs. Frosty)"), "{:?}", episodes);
		assert!(has(0, None, "Post COVID"), "{:?}", episodes);
		assert!(has(0, None, "The End of Obesity"), "{:?}", episodes);
		assert!(has(-1, None, "S14E05 - 200 (Uncensored)"), "{:?}", episodes);
		assert!(has(-1, None, "S12 - Six Days to South Park"), "{:?}", episodes);
		let specials = episodes.iter().filter(|episode| episode.0 == 0).map(|episode| episode.2.as_str()).collect::<Vec<_>>();
		assert_eq!(specials, vec!["The Spirit of Christmas (Jesus vs. Frosty)", "Post COVID", "The End of Obesity"]);

		fs::remove_dir_all(&base).unwrap();
	}

	#[test]
	fn rescan_after_contract_rename_keeps_ids() {
		let base = std::env::temp_dir().join(format!("netcrico-rename-test-{}", std::process::id()));
		let _ = fs::remove_dir_all(&base);
		let library = base.join("library");
		let old_movie = library.join("movies/Indiana Jones - Last Crusade (1989)");
		touch(&old_movie.join("E a ultima cruzada CD 1.avi"), 1);
		fs::write(old_movie.join(".local-cinema.json"), r#"{ "tmdbId": "89" }"#).unwrap();
		let old_show = library.join("shows/Sons of Anarchy");
		touch(&old_show.join("Sons.of.Anarchy.S01.1080p-RARBG/Sons.of.Anarchy.S01E01.1080p-RARBG.mp4"), 1);
		fs::write(old_show.join(".local-cinema.json"), r#"{ "tmdbId": "1409" }"#).unwrap();

		let database = DatabaseInfrastructure::new(base.join("appdata")).unwrap();
		database.initialize().unwrap();
		let state = AppState::new(database);
		SettingsModule::save_library_root_paths(&state, &[library.to_string_lossy().to_string()]).unwrap();
		ScannerModule::rescan_library(&state, None).unwrap();
		let ids_before = LibraryModule::list_media(&state).unwrap().into_iter().map(|item| (item.media_type, item.id)).collect::<Vec<_>>();
		let episode_before = LibraryModule::list_media(&state)
			.unwrap()
			.into_iter()
			.find(|item| item.media_type == "show")
			.map(|show| LibraryModule::list_show_episodes(&state, &show.id).unwrap()[0].id.clone())
			.unwrap();

		// Rename both to the library contract.
		let new_movie = library.join("movies/Indiana Jones and the Last Crusade (1989)");
		fs::rename(&old_movie, &new_movie).unwrap();
		fs::rename(new_movie.join("E a ultima cruzada CD 1.avi"), new_movie.join("Indiana Jones and the Last Crusade (1989) - Part 1.avi")).unwrap();
		let new_show = library.join("shows/Sons of Anarchy (2008)");
		fs::rename(&old_show, &new_show).unwrap();
		fs::create_dir_all(new_show.join("Season 01")).unwrap();
		fs::rename(
			new_show.join("Sons.of.Anarchy.S01.1080p-RARBG/Sons.of.Anarchy.S01E01.1080p-RARBG.mp4"),
			new_show.join("Season 01/S01E01 - Pilot.mp4")
		)
		.unwrap();
		ScannerModule::rescan_library(&state, None).unwrap();

		let items = LibraryModule::list_media(&state).unwrap();
		let ids_after = items.iter().map(|item| (item.media_type.clone(), item.id.clone())).collect::<Vec<_>>();
		assert_eq!(ids_before.len(), ids_after.len(), "{:?}", ids_after);
		for pair in &ids_before {
			assert!(ids_after.contains(pair), "{:?} lost; after: {:?}", pair, ids_after);
		}
		let movie = items.iter().find(|item| item.media_type == "movie").unwrap();
		assert_eq!(movie.title, "Indiana Jones and the Last Crusade");
		assert!(movie.media_path.ends_with("Indiana Jones and the Last Crusade (1989) - Part 1.avi"));
		let show = items.iter().find(|item| item.media_type == "show").unwrap();
		let episodes = LibraryModule::list_show_episodes(&state, &show.id).unwrap();
		assert_eq!(episodes.len(), 1);
		assert_eq!(episodes[0].id, episode_before);
		assert_eq!(episodes[0].title, "Pilot");

		fs::remove_dir_all(&base).unwrap();
	}
}
