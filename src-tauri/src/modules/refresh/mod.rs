//! "Atualizar" on a show: a dry run of what TMDb would change (metadata in
//! pt-BR, poster and cover choices, episode file names), applied only after
//! the user confirms. Claude places episode files whose names don't say
//! which TMDb episode they are (mostly Specials). Subtitles are a separate,
//! per-season step (`subtitles`), since OpenSubtitles has a daily quota.

mod agent;
pub mod subtitles;
pub mod sync;

use crate::app::state::AppState;
use crate::infrastructure::db::MediaItemRecord;
use crate::modules::importer::sanitize_file_name;
use crate::modules::library::LibraryModule;
use crate::modules::metadata::{format_rating_value, normalize_country_label, normalize_genre_label, MetadataModule};
use crate::modules::scanner::{has_extension, ScannerModule, EXTRAS_SEASON, SUBTITLE_EXTENSIONS};
use crate::modules::settings::SettingsModule;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

pub struct RefreshModule;

const REFRESH_PROGRESS_EVENT: &str = "library://refresh-progress";
const IMAGE_OPTIONS: usize = 12;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldChange {
	/// Key in `.local-cinema.json`.
	pub key: String,
	pub label: String,
	pub current: Option<String>,
	pub proposed: Option<String>
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageOption {
	pub preview_url: String,
	pub url: String,
	pub width: u32,
	pub height: u32,
	pub language: Option<String>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMove {
	pub from: String,
	pub to: String
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeRename {
	pub season_number: i32,
	pub episode_number: u32,
	pub from: String,
	pub to: String,
	/// Subtitles that follow the video to its new name.
	pub companions: Vec<FileMove>,
	/// "file" (number taken from the file name) or "claude".
	pub matched_by: String,
	/// Target already taken (on disk or by another rename); not applied.
	pub conflict: bool
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshPlanDto {
	pub media_id: String,
	pub tmdb_id: String,
	pub tmdb_title: String,
	pub tmdb_year: Option<i32>,
	pub fields: Vec<FieldChange>,
	pub posters: Vec<ImageOption>,
	pub backdrops: Vec<ImageOption>,
	pub current_poster: Option<String>,
	pub current_cover: Option<String>,
	pub renames: Vec<EpisodeRename>,
	pub notes: Vec<String>
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldValue {
	pub key: String,
	pub value: Option<String>
}

/// What the user kept from the plan.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshSelection {
	pub media_id: String,
	pub fields: Vec<FieldValue>,
	pub poster_url: Option<String>,
	pub cover_url: Option<String>,
	/// Videos and their subtitles, as listed in the plan.
	pub moves: Vec<FileMove>
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshResultDto {
	/// Applied renames, so the UI can follow ids (file paths) it keeps, like
	/// continue watching.
	pub moved: Vec<FileMove>,
	pub undo_log: Option<String>
}

#[derive(Clone, Serialize)]
struct RefreshProgressPayload {
	message: String
}

impl RefreshModule {
	/// Builds the dry run. Nothing is written. `tmdb_id` overrides the show's
	/// own id (the user is fixing a wrong match).
	pub fn preview(state: &AppState, app: Option<AppHandle>, media_id: &str, tmdb_id: Option<String>) -> Result<RefreshPlanDto, String> {
		let report = |message: &str| emit_step(&app, message);
		let layout = load_item(state, media_id)?;
		let item = &layout.item;
		let is_show = layout.is_show();
		let kind = layout.tmdb_kind();
		let keys = SettingsModule::metadata_api_keys(state)?;
		let tmdb_key = keys.tmdb_api_key.ok_or_else(|| "Configure a TMDb API key nas configuracoes.".to_string())?;
		let client = http_client()?;
		let mut notes = Vec::new();

		let tmdb_id = match tmdb_id.map(|id| id.trim().to_string()).filter(|id| !id.is_empty()).or(item.tmdb_id.clone()) {
			Some(id) => id,
			None => {
				report("Procurando no TMDb...");
				let candidate = MetadataModule::tmdb_search(&tmdb_key, kind, &item.title, item.year)?
					.into_iter()
					.next()
					.ok_or_else(|| format!("TMDb nao encontrou \"{}\". Informe o TMDb ID.", item.title))?;
				notes.push(format!("Sem TMDb ID salvo; escolhido o primeiro resultado da busca ({}). Confira.", candidate.title));
				candidate.tmdb_id
			}
		};

		report("Buscando metadata no TMDb...");
		let english = tmdb_get(
			&client,
			&tmdb_key,
			&format!("{}/{}", kind, tmdb_id),
			&[
				("language", "en-US"),
				("append_to_response", if is_show { "aggregate_credits,external_ids,images" } else { "credits,external_ids,images" }),
				("include_image_language", "en,pt,null")
			]
		)?;
		let portuguese = tmdb_get(&client, &tmdb_key, &format!("{}/{}", kind, tmdb_id), &[("language", "pt-BR")]).unwrap_or(Value::Null);

		// TMDb names the same things differently for TV and movies.
		let (title_key, original_key, date_key) = match is_show {
			true => ("name", "original_name", "first_air_date"),
			false => ("title", "original_title", "release_date")
		};
		let tmdb_title = str_at(&english, &format!("/{}", title_key)).unwrap_or_default();
		let tmdb_year = str_at(&english, &format!("/{}", date_key)).and_then(|date| date.get(..4).and_then(|year| year.parse().ok()));
		// Up to `take` distinct names from `names`, mapped and comma-joined.
		let joined = |names: Vec<&str>, take: usize, mapper: fn(&str) -> String| {
			let mut seen = Vec::<String>::new();
			for name in names {
				let name = mapper(name);
				if !name.is_empty() && !seen.contains(&name) && seen.len() < take {
					seen.push(name);
				}
			}
			(!seen.is_empty()).then(|| seen.join(", "))
		};
		let names_at = |pointer: &str, field: &str| -> Vec<&str> {
			english.pointer(pointer).and_then(Value::as_array).into_iter().flatten().filter_map(|entry| entry[field].as_str()).collect()
		};

		let (director_label, directors) = match is_show {
			true => ("Criacao", names_at("/created_by", "name")),
			false => (
				"Direcao",
				english
					.pointer("/credits/crew")
					.and_then(Value::as_array)
					.into_iter()
					.flatten()
					.filter(|member| member["job"] == "Director")
					.filter_map(|member| member["name"].as_str())
					.collect()
			)
		};
		let cast = names_at(if is_show { "/aggregate_credits/cast" } else { "/credits/cast" }, "name");
		// A movie has production countries; a show only the countries it originates in.
		let country_names = match (is_show, names_at("/production_countries", "name")) {
			(false, names) if !names.is_empty() => names,
			_ => english.pointer("/origin_country").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect()
		};

		// Titles stay TMDb's English (library contract); the synopsis is pt-BR
		// when TMDb has one.
		let proposed = [
			("englishTitle", "Titulo", Some(tmdb_title.clone()).filter(|title| !title.is_empty()), item.english_title.clone()),
			("originalTitle", "Titulo original", str_at(&english, &format!("/{}", original_key)), item.original_title.clone()),
			("year", "Ano", tmdb_year.map(|year: i32| year.to_string()), item.year.map(|year| year.to_string())),
			("overview", "Sinopse", str_at(&portuguese, "/overview").or_else(|| str_at(&english, "/overview")), item.overview.clone()),
			("genre", "Genero", joined(names_at("/genres", "name"), 4, normalize_genre_label), item.genre.clone()),
			("director", director_label, joined(directors, 4, trimmed), item.director.clone()),
			("actors", "Elenco", joined(cast, 6, trimmed), item.actors.clone()),
			("country", "Pais", joined(country_names, 4, normalize_country_label), item.country.clone()),
			("rating", "Nota", english.pointer("/vote_average").and_then(Value::as_f64).and_then(format_rating_value), item.imdb_rating.clone()),
			("imdbId", "IMDb ID", str_at(&english, "/external_ids/imdb_id"), item.imdb_id.clone()),
			("tmdbId", "TMDb ID", Some(tmdb_id.clone()), item.tmdb_id.clone())
		];
		let fields = proposed
			.into_iter()
			.filter(|(_, _, proposed, current)| proposed.is_some() && proposed != current)
			.map(|(key, label, proposed, current)| FieldChange { key: key.to_string(), label: label.to_string(), current, proposed })
			.collect();

		// Posters carry the title, so English/Portuguese ones first; the cover
		// sits behind the page's own title, so textless backdrops first.
		let posters = image_options(&english, "/images/posters", &["en", "pt"], "w342", "w780");
		let backdrops = image_options(&english, "/images/backdrops", &["null"], "w780", "original");

		// Only shows have episodes to name; a movie's file name is the user's.
		let renames = if is_show {
			report("Comparando nomes dos episodios com o TMDb...");
			plan_renames(state, &layout, &tmdb_key, &tmdb_id, keys.anthropic_api_key.as_deref(), &mut notes, &report)?
		} else {
			Vec::new()
		};

		Ok(RefreshPlanDto {
			media_id: item.id.clone(),
			tmdb_id,
			tmdb_title,
			tmdb_year,
			fields,
			posters,
			backdrops,
			current_poster: item.poster_path.clone(),
			current_cover: item.cover_path.clone(),
			renames,
			notes
		})
	}

	/// Renames files, swaps artwork (old files go to `_quarantine`), writes the
	/// metadata file and re-indexes the show.
	pub fn apply(state: &AppState, app: Option<AppHandle>, selection: RefreshSelection) -> Result<RefreshResultDto, String> {
		let report = |message: &str| emit_step(&app, message);
		let layout = load_item(state, &selection.media_id)?;
		let item_folder = layout.folder.clone();
		let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|elapsed| elapsed.as_secs()).unwrap_or_default();
		// Library contract: nothing is deleted, replaced files go here with an undo log.
		let quarantine = layout.quarantine(&format!("refresh-{}", stamp))?;

		// Every move stays inside the item's folder and keeps its directory.
		for file_move in &selection.moves {
			let (from, to) = (Path::new(&file_move.from), Path::new(&file_move.to));
			if !from.starts_with(&item_folder) || from.parent() != to.parent() {
				return Err(format!("Renomeacao fora da pasta do item: {}", file_move.from));
			}
			if !from.exists() {
				return Err(format!("{} nao existe mais. Gere o plano de novo.", file_move.from));
			}
			let case_only = file_move.from.to_lowercase() == file_move.to.to_lowercase();
			if to.exists() && !case_only {
				return Err(format!("{} ja existe.", file_move.to));
			}
		}

		let mut moved = Vec::new();
		let mut undo = Vec::new();
		let result = (|| -> Result<(), String> {
			if !selection.moves.is_empty() {
				report(&format!("Renomeando {} arquivos...", selection.moves.len()));
			}
			for file_move in &selection.moves {
				fs::rename(&file_move.from, &file_move.to).map_err(|error| format!("{}: {}", file_move.from, error))?;
				moved.push(file_move.clone());
				undo.push(file_move.clone());
			}

			let client = http_client()?;
			let mut metadata_fields = selection
				.fields
				.iter()
				.map(|field| {
					let value = field.value.clone().filter(|value| !value.trim().is_empty()).map(|value| match field.key.as_str() {
						"year" => value.parse::<i64>().map(Value::from).unwrap_or(Value::String(value)),
						_ => Value::String(value)
					});
					(field.key.as_str(), value)
				})
				.collect::<Vec<_>>();

			for (url, name) in [(&selection.poster_url, "poster"), (&selection.cover_url, "cover")] {
				let Some(url) = url else { continue };
				report(&format!("Baixando {}...", if name == "poster" { "poster" } else { "capa" }));
				let bytes = client
					.get(url)
					.send()
					.and_then(|response| response.error_for_status())
					.and_then(|response| response.bytes())
					.map_err(|error| format!("Download de {} falhou: {}", name, error))?;
				let extension = Path::new(url).extension().and_then(|value| value.to_str()).unwrap_or("jpg").to_ascii_lowercase();
				let target = item_folder.join(format!("{}.{}", name, extension));
				let partial = item_folder.join(format!("{}.{}.partial", name, extension));
				fs::write(&partial, &bytes).map_err(|error| format!("{}: {}", partial.display(), error))?;

				// Any poster.* / cover.* would compete with the new file in the scanner.
				for existing in fs::read_dir(&item_folder).map_err(|error| error.to_string())?.filter_map(Result::ok).map(|entry| entry.path()) {
					let stem = existing.file_stem().and_then(|value| value.to_str()).unwrap_or_default();
					let is_image = existing.extension().and_then(|value| value.to_str()).is_some_and(|value| IMAGE_EXTENSIONS.contains(&value.to_ascii_lowercase().as_str()));
					if stem.eq_ignore_ascii_case(name) && is_image {
						fs::create_dir_all(&quarantine).map_err(|error| error.to_string())?;
						let parked = quarantine.join(existing.file_name().unwrap_or_default());
						fs::rename(&existing, &parked).map_err(|error| format!("{}: {}", existing.display(), error))?;
						undo.push(FileMove { from: existing.to_string_lossy().to_string(), to: parked.to_string_lossy().to_string() });
					}
				}
				fs::rename(&partial, &target).map_err(|error| format!("{}: {}", target.display(), error))?;
				metadata_fields.push((name, Some(Value::String(format!("{}.{}", name, extension)))));
			}

			if !metadata_fields.is_empty() {
				report("Gravando metadata...");
				MetadataModule::write_local_metadata_fields(&item_folder, &metadata_fields)?;
			}
			Ok(())
		})();

		let undo_log = if undo.is_empty() {
			None
		} else {
			fs::create_dir_all(&quarantine).map_err(|error| error.to_string())?;
			let path = quarantine.join(format!("undo-{}.json", stamp));
			let log = json!({ "item": layout.item.media_path, "moves": undo });
			fs::write(&path, serde_json::to_string_pretty(&log).unwrap_or_default()).map_err(|error| error.to_string())?;
			Some(path.to_string_lossy().to_string())
		};
		result?;

		report("Reindexando...");
		if layout.is_show() {
			if !moved.is_empty() {
				state.database.delete_show_episodes(&layout.item.id)?;
			}
		} else {
			// A changed director must not stay linked next to the new one.
			state.database.clear_director_links(&layout.item.id)?;
		}
		layout.reindex(state, app.clone())?;
		report("Pronto.");

		Ok(RefreshResultDto { moved, undo_log })
	}
}

/// Episode files whose name isn't `SxxEyy - <TMDb name>`. The number comes
/// from the scanner (file name); Claude places the ones it can't trust.
fn plan_renames(
	state: &AppState,
	layout: &ItemLayout,
	tmdb_key: &str,
	tmdb_id: &str,
	anthropic_key: Option<&str>,
	notes: &mut Vec<String>,
	report: &dyn Fn(&str)
) -> Result<Vec<EpisodeRename>, String> {
	let episodes = LibraryModule::list_show_episodes(state, &layout.item.id)?
		.into_iter()
		.filter(|episode| episode.season_number.is_some_and(|season| season != EXTRAS_SEASON))
		.collect::<Vec<_>>();

	let seasons = episodes.iter().filter_map(|episode| episode.season_number).collect::<BTreeSet<_>>();
	let mut names = HashMap::<i32, HashMap<u32, String>>::new();
	for season in seasons {
		names.insert(season, MetadataModule::tmdb_season_episode_names(tmdb_key, tmdb_id, season as u32).into_iter().collect());
	}

	// Specials numbered by a release (not TMDb) and episodes with no number.
	let is_contract_named = |stem: &str| stem.starts_with("Special - ") || is_episode_token_named(stem);
	let uncertain = episodes
		.iter()
		.filter(|episode| {
			let stem = file_stem(&episode.media_path);
			!is_contract_named(&stem) && (episode.season_number == Some(0) || episode.episode_number.is_none())
		})
		.collect::<Vec<_>>();

	let mut claude_numbers = HashMap::<String, u32>::new();
	if !uncertain.is_empty() {
		match anthropic_key {
			Some(key) => {
				report(&format!("Perguntando ao Claude sobre {} arquivo(s) sem numero confiavel...", uncertain.len()));
				let files = uncertain
					.iter()
					.map(|episode| (episode.media_path.clone(), episode.season_number.unwrap_or_default()))
					.collect::<Vec<_>>();
				match agent::match_episodes(key, &layout.item.title, &files, &names) {
					Ok(matches) => claude_numbers = matches,
					Err(error) => notes.push(format!("Claude falhou ({}); esses arquivos ficaram como estao.", error))
				}
			}
			None => notes.push(format!(
				"{} arquivo(s) de Especiais ou sem numero ficaram como estao: configure a Anthropic API key para o Claude identifica-los.",
				uncertain.len()
			))
		}
	}
	let uncertain_paths = uncertain.iter().map(|episode| episode.media_path.clone()).collect::<HashSet<_>>();

	let mut renames = Vec::new();
	let mut taken = HashSet::<String>::new();
	let mut missing_names = 0;
	for episode in &episodes {
		let season = episode.season_number.unwrap_or_default();
		let (number, matched_by) = if uncertain_paths.contains(&episode.media_path) {
			match claude_numbers.get(&episode.media_path) {
				Some(number) => (*number, "claude"),
				None => continue
			}
		} else {
			match episode.episode_number {
				Some(number) if number > 0 => (number as u32, "file"),
				_ => continue
			}
		};
		let Some(name) = names.get(&season).and_then(|season_names| season_names.get(&number)) else {
			missing_names += 1;
			continue;
		};

		let video = Path::new(&episode.media_path);
		let old_stem = file_stem(&episode.media_path);
		let new_stem = format!("S{:02}E{:02} - {}", season, number, sanitize_file_name(name));
		if old_stem == new_stem {
			continue;
		}
		let folder = video.parent().unwrap_or(Path::new(""));
		let extension = video.extension().and_then(|value| value.to_str()).unwrap_or_default();
		let to = folder.join(format!("{}.{}", new_stem, extension)).to_string_lossy().to_string();

		// "<old stem>.en.srt" -> "<new stem>.en.srt".
		let companions = fs::read_dir(folder)
			.map(|entries| {
				entries
					.filter_map(Result::ok)
					.map(|entry| entry.path())
					.filter(|path| has_extension(path, SUBTITLE_EXTENSIONS))
					.filter_map(|path| {
						let file_name = path.file_name()?.to_str()?.to_string();
						let rest = file_name.strip_prefix(&old_stem)?;
						rest.starts_with('.').then(|| FileMove {
							from: path.to_string_lossy().to_string(),
							to: folder.join(format!("{}{}", new_stem, rest)).to_string_lossy().to_string()
						})
					})
					.collect::<Vec<_>>()
			})
			.unwrap_or_default();

		let targets = std::iter::once(to.clone()).chain(companions.iter().map(|companion| companion.to.clone())).collect::<Vec<_>>();
		let conflict = targets.iter().any(|target| {
			taken.contains(&target.to_lowercase()) || (Path::new(target).exists() && target.to_lowercase() != episode.media_path.to_lowercase())
		});
		if !conflict {
			taken.extend(targets.iter().map(|target| target.to_lowercase()));
		}

		renames.push(EpisodeRename {
			season_number: season,
			episode_number: number,
			from: episode.media_path.clone(),
			to,
			companions,
			matched_by: matched_by.to_string(),
			conflict
		});
	}

	if missing_names > 0 {
		notes.push(format!("{} episodio(s) sem nome no TMDb para o numero do arquivo; ficaram como estao.", missing_names));
	}
	let conflicts = renames.iter().filter(|rename| rename.conflict).count();
	if conflicts > 0 {
		notes.push(format!("{} renomeacao(oes) em conflito (destino ja existe ou repetido); nao serao aplicadas.", conflicts));
	}
	renames.sort_by(|left, right| (sort_season(left.season_number), left.episode_number).cmp(&(sort_season(right.season_number), right.episode_number)));
	Ok(renames)
}

/// "S01E02 - Name": already in the library contract's shape.
fn is_episode_token_named(stem: &str) -> bool {
	let bytes = stem.as_bytes();
	bytes.len() > 9
		&& (bytes[0] == b'S' || bytes[0] == b's')
		&& bytes[1..3].iter().all(u8::is_ascii_digit)
		&& (bytes[3] == b'E' || bytes[3] == b'e')
		&& bytes[4..6].iter().all(u8::is_ascii_digit)
		&& stem[6..].trim_start_matches(|character: char| character.is_ascii_digit()).starts_with(" - ")
}

// Specials after the regular seasons, as on the show page.
fn sort_season(season: i32) -> i32 {
	if season == 0 { i32::MAX } else { season }
}

pub(crate) fn image_options(details: &Value, pointer: &str, preferred_languages: &[&str], preview_size: &str, full_size: &str) -> Vec<ImageOption> {
	let mut images = details.pointer(pointer).and_then(Value::as_array).cloned().unwrap_or_default();
	let language_rank = |image: &Value| {
		let language = image["iso_639_1"].as_str().unwrap_or("null");
		preferred_languages.iter().position(|preferred| *preferred == language).unwrap_or(preferred_languages.len())
	};
	images.sort_by(|left, right| {
		language_rank(left)
			.cmp(&language_rank(right))
			.then(right["vote_average"].as_f64().unwrap_or_default().total_cmp(&left["vote_average"].as_f64().unwrap_or_default()))
			.then(right["width"].as_u64().cmp(&left["width"].as_u64()))
	});

	images
		.iter()
		.filter_map(|image| {
			let file_path = image["file_path"].as_str()?;
			Some(ImageOption {
				preview_url: format!("https://image.tmdb.org/t/p/{}{}", preview_size, file_path),
				url: format!("https://image.tmdb.org/t/p/{}{}", full_size, file_path),
				width: image["width"].as_u64().unwrap_or_default() as u32,
				height: image["height"].as_u64().unwrap_or_default() as u32,
				language: image["iso_639_1"].as_str().map(str::to_string)
			})
		})
		.take(IMAGE_OPTIONS)
		.collect()
}

/// Category folders of the library contract.
const CATEGORY_FOLDERS: &[&str] = &["movies", "documentary", "documentaries", "standup", "concerts"];

/// Where an item lives. A show is a folder under `shows`; a movie, concert,
/// stand-up or documentary is a video file in a folder under its category folder.
pub(crate) struct ItemLayout {
	pub item: MediaItemRecord,
	/// The show folder, or the folder holding the video (its metadata and artwork).
	pub folder: PathBuf,
	/// `shows`, `movies`, ...: what the scanner walks from.
	pub category_root: PathBuf
}

/// A video of an item: one per episode of a show, the file itself otherwise.
pub(crate) struct VideoEntry {
	pub id: String,
	pub media_path: String,
	pub title: String,
	pub season_number: Option<i32>,
	pub episode_number: Option<i32>
}

impl ItemLayout {
	pub fn is_show(&self) -> bool {
		self.item.media_type == "show"
	}

	/// TMDb's name for the item's kind.
	pub fn tmdb_kind(&self) -> &'static str {
		if self.is_show() { "tv" } else { "movie" }
	}

	/// The videos subtitles apply to (a show's extras excluded).
	pub fn videos(&self, state: &AppState) -> Result<Vec<VideoEntry>, String> {
		if !self.is_show() {
			return Ok(vec![VideoEntry {
				id: self.item.id.clone(),
				media_path: self.item.media_path.clone(),
				title: self.item.title.clone(),
				season_number: None,
				episode_number: None
			}]);
		}

		Ok(LibraryModule::list_show_episodes(state, &self.item.id)?
			.into_iter()
			.filter(|episode| episode.season_number.is_some_and(|season| season != EXTRAS_SEASON))
			.map(|episode| VideoEntry {
				id: episode.id,
				media_path: episode.media_path,
				title: episode.title,
				season_number: episode.season_number,
				episode_number: episode.episode_number
			})
			.collect())
	}

	/// Library contract: nothing is deleted, replaced files go to
	/// `<library>/_quarantine/<item folder>/<label>`.
	pub fn quarantine(&self, label: &str) -> Result<PathBuf, String> {
		let library_root = self.category_root.parent().ok_or_else(|| "A pasta da categoria esta fora da biblioteca.".to_string())?;
		Ok(library_root.join("_quarantine").join(self.folder.file_name().unwrap_or_default()).join(label))
	}

	/// Re-reads just this item into the database.
	pub fn reindex(&self, state: &AppState, app: Option<AppHandle>) -> Result<(), String> {
		ScannerModule::index_item(state, app, &self.category_root, &self.folder, &self.item.media_type, false)
	}
}

pub(crate) fn load_item(state: &AppState, media_id: &str) -> Result<ItemLayout, String> {
	let item = LibraryModule::get_media_item(state, media_id)?
		.filter(|item| matches!(item.media_type.as_str(), "show" | "movie" | "documentary" | "standup" | "concert"))
		.ok_or_else(|| "Item nao encontrado ou sem suporte a Atualizar.".to_string())?;
	let media_path = PathBuf::from(&item.media_path);

	if item.media_type == "show" {
		let category_root = media_path.parent().ok_or_else(|| "Pasta da serie sem pasta pai.".to_string())?.to_path_buf();
		return Ok(ItemLayout { folder: media_path, category_root, item });
	}

	let folder = media_path.parent().ok_or_else(|| "Video sem pasta.".to_string())?.to_path_buf();
	// A video loose in the category folder has no folder of its own to hold artwork.
	let category_root = folder
		.ancestors()
		.skip(1)
		.find(|ancestor| {
			ancestor.file_name().and_then(|name| name.to_str()).is_some_and(|name| CATEGORY_FOLDERS.iter().any(|category| name.eq_ignore_ascii_case(category)))
		})
		.map(Path::to_path_buf)
		.ok_or_else(|| "O video precisa estar em uma pasta propria dentro de movies, documentary, standup ou concerts.".to_string())?;
	Ok(ItemLayout { item, folder, category_root })
}

pub(crate) fn http_client() -> Result<Client, String> {
	Client::builder().timeout(Duration::from_secs(30)).build().map_err(|error| error.to_string())
}

pub(crate) fn tmdb_get(client: &Client, api_key: &str, path: &str, query: &[(&str, &str)]) -> Result<Value, String> {
	client
		.get(format!("https://api.themoviedb.org/3/{}", path))
		.query(&[("api_key", api_key)])
		.query(query)
		.send()
		.and_then(|response| response.error_for_status())
		.and_then(|response| response.json::<Value>())
		.map_err(|error| format!("TMDb: {}", error))
}

fn trimmed(raw: &str) -> String {
	raw.trim().to_string()
}

fn str_at(value: &Value, pointer: &str) -> Option<String> {
	value.pointer(pointer).and_then(Value::as_str).map(str::trim).filter(|text| !text.is_empty()).map(str::to_string)
}

pub(crate) fn file_stem(path: &str) -> String {
	Path::new(path).file_stem().and_then(|stem| stem.to_str()).unwrap_or_default().to_string()
}

pub(crate) fn emit_step(app: &Option<AppHandle>, message: &str) {
	eprintln!("[refresh] {}", message);
	if let Some(app) = app {
		let _ = app.emit(REFRESH_PROGRESS_EVENT, RefreshProgressPayload { message: message.to_string() });
	}
}

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "jfif", "png", "webp", "gif", "bmp", "avif"];
